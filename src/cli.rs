//! CLI definition for unbundio-vault.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "unbundio-vault",
    version,
    about = "Local-first password vault — no account, no cloud, no subscription."
)]
pub struct Cli {
    /// Vault file. Defaults to $UNBUNDIO_VAULT or ./unbundio-vault.vault
    #[arg(long, global = true)]
    pub vault: Option<PathBuf>,

    /// Read master password from this file (for scripts). Prefer env UNBUNDIO_VAULT_PASSWORD.
    #[arg(long, global = true)]
    pub password_file: Option<PathBuf>,

    /// Machine-readable JSON output where supported
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new empty vault
    Init {},
    /// Add a login (prompts for missing fields; --generate creates the password)
    Add {
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        #[arg(long)]
        favorite: bool,
        #[arg(long)]
        generate: bool,
        #[arg(long, default_value_t = 20)]
        length: usize,
        #[arg(long)]
        no_symbols: bool,
    },
    /// Show one entry (fails on ambiguous query)
    Get {
        query: String,
        /// Print the password (default masks it)
        #[arg(long)]
        show: bool,
    },
    /// List entries (titles only by default)
    List {
        #[arg(long)]
        search: Option<String>,
        #[arg(long)]
        tag: Option<String>,
    },
    /// Update fields of one entry
    Update {
        query: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long)]
        favorite: Option<bool>,
    },
    /// Delete one entry (exact id for safety when ambiguous)
    Rm { query: String },
    /// Generate a random password (no vault needed)
    Gen {
        #[arg(long, default_value_t = 20)]
        length: usize,
        #[arg(long)]
        no_uppercase: bool,
        #[arg(long)]
        no_lowercase: bool,
        #[arg(long)]
        no_digits: bool,
        #[arg(long)]
        no_symbols: bool,
        #[arg(long)]
        exclude_ambiguous: bool,
    },
    /// Export decrypted entries to CSV (warns: plaintext!) or JSON
    Export {
        #[arg(long, default_value = "csv")]
        format: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Import CSV exported from Chrome / Dashlane / 1Password
    Import {
        #[arg(long)]
        path: PathBuf,
    },
    /// Local audit: weak, reused, old passwords
    Audit {},
    /// Change the master password (re-encrypts with fresh salt)
    Passwd {},
}
