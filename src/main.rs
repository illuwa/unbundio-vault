mod cli;

use anyhow::{bail, Context, Result};
use clap::Parser;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use unbundio_vault::{
    audit, csv_io,
    generator::GenOptions,
    host,
    vault::{NewEntry, Vault},
};

use cli::{Cli, Command};

fn default_vault_path(arg: Option<PathBuf>) -> PathBuf {
    if let Some(p) = arg {
        return p;
    }
    if let Ok(env) = std::env::var("UNBUNDIO_VAULT") {
        if !env.trim().is_empty() {
            return PathBuf::from(env);
        }
    }
    PathBuf::from("unbundio-vault.vault")
}

/// Master password resolution order:
/// 1. $UNBUNDIO_VAULT_PASSWORD  2. --password-file  3. interactive prompt (no echo)
fn read_password(password_file: Option<&Path>, confirm: bool) -> Result<String> {
    if let Ok(env) = std::env::var("UNBUNDIO_VAULT_PASSWORD") {
        if !env.is_empty() {
            return Ok(env);
        }
    }
    if let Some(path) = password_file {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let pw = raw.trim_end_matches(['\n', '\r']).to_string();
        if pw.is_empty() {
            bail!("password file is empty");
        }
        return Ok(pw);
    }
    if !std::io::stdin().is_terminal() {
        bail!("no TTY: set UNBUNDIO_VAULT_PASSWORD or --password-file");
    }
    let pw = rpassword::prompt_password("Master password: ").context("read password")?;
    if confirm {
        let again =
            rpassword::prompt_password("Confirm master password: ").context("read password")?;
        if pw != again {
            bail!("passwords do not match");
        }
    }
    if pw.is_empty() {
        bail!("password must not be empty");
    }
    Ok(pw)
}

fn prompt_line(label: &str) -> Result<String> {
    print!("{label}: ");
    std::io::stdout().flush().ok();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).context("read stdin")?;
    Ok(s.trim().to_string())
}

fn mask(pw: &str) -> String {
    if pw.len() <= 4 {
        return "****".to_string();
    }
    format!("{}…({} chars)", &pw[..2], pw.len())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let vault_path = default_vault_path(cli.vault);

    match cli.cmd {
        Command::Init {} => {
            let pw = read_password(cli.password_file.as_deref(), true)?;
            Vault::create(&vault_path, &pw)?;
            println!("created {}", vault_path.display());
        }
        Command::Gen {
            length,
            no_uppercase,
            no_lowercase,
            no_digits,
            no_symbols,
            exclude_ambiguous,
        } => {
            let opts = GenOptions {
                length,
                uppercase: !no_uppercase,
                lowercase: !no_lowercase,
                digits: !no_digits,
                symbols: !no_symbols,
                exclude_ambiguous,
            };
            let pw =
                unbundio_vault::generator::generate(&opts).map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{pw}");
        }
        Command::Add {
            title,
            username,
            password,
            url,
            notes,
            tags,
            favorite,
            generate,
            length,
            no_symbols,
        } => {
            let pw_master = read_password(cli.password_file.as_deref(), false)?;
            let mut vault = Vault::open(&vault_path, &pw_master)?;
            let title = match title {
                Some(t) if !t.trim().is_empty() => t,
                _ => prompt_line("Title (e.g. github)")?,
            };
            let username = match username {
                Some(u) if !u.is_empty() => u,
                _ => prompt_line("Username")?,
            };
            let password = if generate || password.is_none() {
                if password.is_none() && !generate && std::io::stdin().is_terminal() {
                    // Ask: generate or type?
                    let choice = prompt_line("Password (empty = generate)")?;
                    if choice.is_empty() {
                        let opts = GenOptions {
                            length,
                            symbols: !no_symbols,
                            ..Default::default()
                        };
                        unbundio_vault::generator::generate(&opts)
                            .map_err(|e| anyhow::anyhow!("{e}"))?
                    } else {
                        choice
                    }
                } else {
                    let opts = GenOptions {
                        length,
                        symbols: !no_symbols,
                        ..Default::default()
                    };
                    unbundio_vault::generator::generate(&opts)
                        .map_err(|e| anyhow::anyhow!("{e}"))?
                }
            } else {
                password.unwrap_or_default()
            };
            if title.trim().is_empty() {
                bail!("title must not be empty");
            }
            let id = {
                let e = vault.add(NewEntry {
                    title: title.trim().to_string(),
                    username,
                    password,
                    url,
                    notes,
                    tags: tags
                        .into_iter()
                        .flat_map(|t| {
                            t.split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>()
                        })
                        .collect(),
                    favorite,
                });
                e.id.clone()
            };
            vault.save(&pw_master)?;
            if cli.json {
                println!("{}", serde_json::json!({"id": id}));
            } else {
                println!("added {id}");
            }
        }
        Command::Get { query, show } => {
            let pw = read_password(cli.password_file.as_deref(), false)?;
            let vault = Vault::open(&vault_path, &pw)?;
            let hits = vault.find(&query);
            if hits.is_empty() {
                bail!("no entry matches '{query}'");
            }
            if hits.len() > 1 {
                eprintln!(
                    "{} entries match; showing first. Refine query or use id:",
                    hits.len()
                );
                for e in hits.iter().take(10) {
                    eprintln!(
                        "  {}  {}  {}",
                        &e.id[..8.min(e.id.len())],
                        e.title,
                        e.username
                    );
                }
                bail!("ambiguous query");
            }
            let e = hits[0];
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": e.id, "title": e.title, "username": e.username,
                        "password": e.password, "url": e.url, "notes": e.notes,
                        "tags": e.tags, "favorite": e.favorite,
                    })
                );
            } else {
                println!("title:    {}", e.title);
                println!("username:  {}", e.username);
                println!(
                    "password:  {}",
                    if show {
                        e.password.clone()
                    } else {
                        mask(&e.password)
                    }
                );
                println!("url:       {}", e.url.as_deref().unwrap_or("-"));
                println!("notes:     {}", e.notes.as_deref().unwrap_or("-"));
                println!(
                    "tags:      {}",
                    if e.tags.is_empty() {
                        "-".into()
                    } else {
                        e.tags.join(", ")
                    }
                );
                println!("id:        {}", e.id);
            }
        }
        Command::List { search, tag } => {
            let pw = read_password(cli.password_file.as_deref(), false)?;
            let vault = Vault::open(&vault_path, &pw)?;
            let mut entries: Vec<&unbundio_vault::Entry> = vault.data.entries.iter().collect();
            if let Some(q) = search {
                entries.retain(|e| e.matches(&q));
            }
            if let Some(t) = tag {
                let low = t.to_lowercase();
                entries.retain(|e| e.tags.iter().any(|x| x.to_lowercase() == low));
            }
            entries.sort_by_key(|a| a.title.to_lowercase());
            if cli.json {
                let arr: Vec<_> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({"id": e.id, "title": e.title, "username": e.username, "url": e.url, "tags": e.tags, "favorite": e.favorite})
                    })
                    .collect();
                println!("{}", serde_json::json!(arr));
            } else if entries.is_empty() {
                println!("(empty)");
            } else {
                for e in entries {
                    let star = if e.favorite { "*" } else { " " };
                    println!(
                        "{star} {}  {}  {}",
                        &e.id[..8.min(e.id.len())],
                        e.title,
                        e.username
                    );
                }
            }
        }
        Command::Update {
            query,
            title,
            username,
            password,
            url,
            notes,
            favorite,
        } => {
            let pw = read_password(cli.password_file.as_deref(), false)?;
            let mut vault = Vault::open(&vault_path, &pw)?;
            let hits = vault.find(&query);
            if hits.is_empty() {
                bail!("no entry matches '{query}'");
            }
            if hits.len() > 1 {
                bail!("ambiguous query: {} match; use exact id", hits.len());
            }
            let id = hits[0].id.clone();
            {
                let entry = vault
                    .data
                    .entries
                    .iter_mut()
                    .find(|e| e.id == id)
                    .context("entry vanished")?;
                if let Some(v) = title {
                    entry.title = v;
                }
                if let Some(v) = username {
                    entry.username = v;
                }
                if let Some(v) = password {
                    entry.password = v;
                }
                if let Some(v) = url {
                    entry.url = if v.trim().is_empty() { None } else { Some(v) };
                }
                if let Some(v) = notes {
                    entry.notes = if v.trim().is_empty() { None } else { Some(v) };
                }
                if let Some(v) = favorite {
                    entry.favorite = v;
                }
            }
            vault.touch(&id);
            vault.save(&pw)?;
            println!("updated {id}");
        }
        Command::Rm { query } => {
            let pw = read_password(cli.password_file.as_deref(), false)?;
            let mut vault = Vault::open(&vault_path, &pw)?;
            let n = vault.remove(&query)?;
            vault.save(&pw)?;
            println!("removed {n}");
        }
        Command::Export { format, out } => {
            let pw = read_password(cli.password_file.as_deref(), false)?;
            let vault = Vault::open(&vault_path, &pw)?;
            eprintln!(
                "warning: '{}' will contain PLAINTEXT secrets",
                out.display()
            );
            match format.to_lowercase().as_str() {
                "csv" => {
                    let f = std::fs::File::create(&out)
                        .with_context(|| format!("cannot write {}", out.display()))?;
                    csv_io::export_csv(&vault.data.entries, f)?;
                }
                "json" => {
                    let text =
                        serde_json::to_string_pretty(&vault.data.entries).context("encode json")?;
                    std::fs::write(&out, text)
                        .with_context(|| format!("cannot write {}", out.display()))?;
                }
                other => bail!("unknown format '{other}' (csv|json)"),
            }
            println!(
                "exported {} entries to {}",
                vault.data.entries.len(),
                out.display()
            );
        }
        Command::Import { path } => {
            let pw = read_password(cli.password_file.as_deref(), false)?;
            let mut vault = Vault::open(&vault_path, &pw)?;
            let f = std::fs::File::open(&path)
                .with_context(|| format!("cannot read {}", path.display()))?;
            let drafts = csv_io::import_csv(f)?;
            if drafts.is_empty() {
                bail!("nothing to import (no rows with a password?)");
            }
            let n = drafts.len();
            for d in drafts {
                vault.add(d);
            }
            vault.save(&pw)?;
            println!("imported {n} entries");
        }
        Command::Audit {} => {
            let pw = read_password(cli.password_file.as_deref(), false)?;
            let vault = Vault::open(&vault_path, &pw)?;
            let rep = audit::audit(&vault.data.entries);
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "total": rep.total, "weak": rep.weak,
                        "reused": rep.reused, "old": rep.old,
                        "without_url": rep.without_url, "favorites": rep.favorites,
                    })
                );
            } else {
                print!("{}", audit::format_human(&rep));
            }
        }
        Command::Passwd {} => {
            let old = read_password(cli.password_file.as_deref(), false)?;
            let vault = Vault::open(&vault_path, &old)?;
            if std::env::var("UNBUNDIO_VAULT_PASSWORD").is_ok() || cli.password_file.is_some() {
                bail!("refusing to change password non-interactively (unset UNBUNDIO_VAULT_PASSWORD first)");
            }
            let new1 = rpassword::prompt_password("New master password: ").context("read")?;
            let new2 =
                rpassword::prompt_password("Confirm new master password: ").context("read")?;
            if new1 != new2 {
                bail!("passwords do not match");
            }
            if new1.is_empty() {
                bail!("password must not be empty");
            }
            // Re-encrypt same data under the new password.
            let vault2 = Vault {
                path: vault.path.clone(),
                data: vault.data.clone(),
            };
            // Borrow dance: VaultData is Clone via derive.
            vault2.save(&new1)?;
            println!("master password changed");
        }
        Command::Host { .. } => {
            // Browser-spawned: password must come from env/file (never the browser).
            let pw = read_password(cli.password_file.as_deref(), false)?;
            host::run(&vault_path, &pw)?;
        }
        Command::Manifest {
            browser,
            extension_id,
            binary,
            print_paths,
        } => {
            let bin = match binary {
                Some(p) => p.to_string_lossy().into_owned(),
                None => std::env::current_exe()
                    .map(|p| p.to_string_lossy().into_owned())
                    .context("resolve current executable; pass --binary explicitly")?,
            };
            let m = host::manifest(&browser, &extension_id, &bin)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&m).context("encode")?);
            if print_paths {
                eprintln!("install to one of:");
                for p in host::manifest_paths(&browser) {
                    eprintln!("  {p}");
                }
            }
        }
    }
    Ok(())
}
