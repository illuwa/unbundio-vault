//! CSV import / export for migration off paid managers.
//!
//! Accepts the common shapes exported by Chrome (`name,url,username,password`),
//! Dashlane (`title,url,username,password,notes`) and 1Password-ish files.
//! Matching is by case-insensitive header name, so column order does not matter.
//! Exported CSV uses the Chrome-compatible header so it imports back anywhere.

use crate::vault::{Entry, NewEntry};
use anyhow::{Context, Result};
use std::io::{Read, Write};

const EXPORT_HEADER: &[&str] = &[
    "name", "url", "username", "password", "notes", "tags", "favorite",
];

pub fn export_csv<W: Write>(entries: &[Entry], mut w: W) -> Result<()> {
    let mut wtr = csv::Writer::from_writer(&mut w);
    wtr.write_record(EXPORT_HEADER)
        .context("write csv header")?;
    for e in entries {
        let tags = e.tags.join(";");
        wtr.write_record([
            e.title.as_str(),
            e.url.as_deref().unwrap_or(""),
            e.username.as_str(),
            e.password.as_str(),
            e.notes.as_deref().unwrap_or(""),
            tags.as_str(),
            if e.favorite { "1" } else { "0" },
        ])
        .with_context(|| format!("write row {}", e.title))?;
    }
    wtr.flush().context("flush csv")?;
    Ok(())
}

fn col(headers: &[String], row: &csv::StringRecord, names: &[&str]) -> String {
    for name in names {
        for (i, h) in headers.iter().enumerate() {
            if h.trim().to_lowercase() == *name {
                return row.get(i).unwrap_or("").to_string();
            }
        }
    }
    String::new()
}

/// Parse any supported CSV shape into drafts. Empty-password rows are skipped.
pub fn import_csv<R: Read>(r: R) -> Result<Vec<NewEntry>> {
    let mut rdr = csv::Reader::from_reader(r);
    let raw_headers = rdr.headers().context("read csv header")?.clone();
    let headers: Vec<String> = raw_headers.iter().map(str::to_string).collect();
    let mut out = Vec::new();
    for rec in rdr.records() {
        let row = rec.context("read csv row")?;
        let title = col(&headers, &row, &["name", "title", "site", "account"]);
        let url = col(&headers, &row, &["url", "website", "site url", "link"]);
        let username = col(
            &headers,
            &row,
            &["username", "login", "email", "user", "account name"],
        );
        let password = col(&headers, &row, &["password", "pass", "secret"]);
        let notes = col(&headers, &row, &["notes", "note", "comments", "memo"]);
        let tags_raw = col(&headers, &row, &["tags", "tag", "category", "group"]);
        if password.trim().is_empty() {
            continue;
        }
        let title = if title.trim().is_empty() {
            if !url.trim().is_empty() {
                url.trim().to_string()
            } else if !username.trim().is_empty() {
                username.trim().to_string()
            } else {
                "untitled".to_string()
            }
        } else {
            title.trim().to_string()
        };
        let tags = tags_raw
            .split([';', ','])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        out.push(NewEntry {
            title,
            username: username.trim().to_string(),
            password: password.trim().to_string(),
            url: if url.trim().is_empty() {
                None
            } else {
                Some(url.trim().to_string())
            },
            notes: if notes.trim().is_empty() {
                None
            } else {
                Some(notes.trim().to_string())
            },
            tags,
            favorite: false,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_shape_roundtrip() {
        let csv = "name,url,username,password\ngithub,https://github.com,alice,s3cret\n";
        let drafts = import_csv(csv.as_bytes()).expect("import");
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].title, "github");
        let entries: Vec<Entry> = drafts
            .into_iter()
            .map(|d| Entry {
                id: "x".into(),
                title: d.title,
                username: d.username,
                password: d.password,
                url: d.url,
                notes: d.notes,
                tags: vec![],
                favorite: false,
                created_at: 0,
                updated_at: 0,
            })
            .collect();
        let mut buf = Vec::new();
        export_csv(&entries, &mut buf).expect("export");
        let text = String::from_utf8(buf).expect("utf8");
        assert!(text.contains("github"));
    }
}
