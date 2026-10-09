//! Local security audit: weak / reused / old passwords. No network, no breach API.

use crate::vault::Entry;
use chrono::{TimeZone, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct AuditReport {
    pub total: usize,
    pub weak: Vec<String>,
    pub reused: Vec<Vec<String>>,
    pub old: Vec<String>,
    pub without_url: usize,
    pub favorites: usize,
}

const OLD_DAYS: i64 = 90;
const COMMON_FRAGMENTS: &[&str] = &[
    "123", "password", "qwerty", "letmein", "welcome", "admin", "abc", "111", "000",
];

fn classes(pw: &str) -> usize {
    let mut c = 0;
    if pw.bytes().any(|b| b.is_ascii_lowercase()) {
        c += 1;
    }
    if pw.bytes().any(|b| b.is_ascii_uppercase()) {
        c += 1;
    }
    if pw.bytes().any(|b| b.is_ascii_digit()) {
        c += 1;
    }
    if pw.bytes().any(|b| !b.is_ascii_alphanumeric()) {
        c += 1;
    }
    c
}

/// Local heuristic. Deliberately conservative: flags short, simple, or
/// pattern-based passwords. This is not a breach lookup.
pub fn is_weak(pw: &str) -> bool {
    if pw.len() < 12 {
        return true;
    }
    if classes(pw) <= 2 && pw.len() < 16 {
        return true;
    }
    let low = pw.to_lowercase();
    if COMMON_FRAGMENTS.iter().any(|f| low.contains(f)) {
        return true;
    }
    false
}

pub fn audit(entries: &[Entry]) -> AuditReport {
    let now = Utc::now().timestamp();
    let cutoff = now - OLD_DAYS * 24 * 3600;
    let mut rep = AuditReport {
        total: entries.len(),
        ..Default::default()
    };
    let mut by_password: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in entries {
        if is_weak(&e.password) {
            rep.weak.push(e.title.clone());
        }
        if !e.password.is_empty() {
            by_password
                .entry(e.password.as_str())
                .or_default()
                .push(e.title.as_str());
        }
        if e.updated_at < cutoff {
            rep.old.push(e.title.clone());
        }
        if e.url.as_deref().unwrap_or("").trim().is_empty() {
            rep.without_url += 1;
        }
        if e.favorite {
            rep.favorites += 1;
        }
    }
    for titles in by_password.into_values() {
        if titles.len() > 1 {
            rep.reused
                .push(titles.into_iter().map(str::to_string).collect());
        }
    }
    rep.weak.sort();
    rep.old.sort();
    rep.reused.sort();
    rep
}

pub fn format_human(rep: &AuditReport) -> String {
    let mut s = String::new();
    s.push_str(&format!("entries: {}\n", rep.total));
    s.push_str(&format!(
        "weak: {} {}\n",
        rep.weak.len(),
        fmt_list(&rep.weak, 5)
    ));
    s.push_str(&format!("reused groups: {}", rep.reused.len()));
    if !rep.reused.is_empty() {
        s.push(' ');
        let groups: Vec<String> = rep
            .reused
            .iter()
            .take(3)
            .map(|g| format!("[{}]", g.join(", ")))
            .collect();
        s.push_str(&groups.join(" "));
        if rep.reused.len() > 3 {
            s.push_str(" …");
        }
    }
    s.push('\n');
    let cutoff_date = Utc
        .timestamp_opt(Utc::now().timestamp() - OLD_DAYS * 24 * 3600, 0)
        .single()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "90d ago".into());
    s.push_str(&format!(
        "older than {cutoff_date}: {} {}\n",
        rep.old.len(),
        fmt_list(&rep.old, 5)
    ));
    s.push_str(&format!("without url: {}\n", rep.without_url));
    s.push_str(&format!("favorites: {}\n", rep.favorites));
    s
}

fn fmt_list(items: &[String], max: usize) -> String {
    if items.is_empty() {
        return String::new();
    }
    let shown: Vec<&str> = items.iter().take(max).map(String::as_str).collect();
    let mut s = format!("[{}]", shown.join(", "));
    if items.len() > max {
        s.push_str(&format!(" +{} more", items.len() - max));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, pw: &str, updated: i64) -> Entry {
        Entry {
            id: format!("id-{title}"),
            title: title.into(),
            username: "u".into(),
            password: pw.into(),
            url: None,
            notes: None,
            tags: vec![],
            favorite: false,
            created_at: updated,
            updated_at: updated,
        }
    }

    #[test]
    fn flags_weak_and_reused() {
        let now = Utc::now().timestamp();
        let entries = vec![
            entry("a", "123456", now),
            entry("b", "123456", now),
            entry("c", "Xk9#mQ2$vL8wZ!p4Qr7", now),
        ];
        let rep = audit(&entries);
        assert_eq!(rep.total, 3);
        assert!(rep.weak.contains(&"a".to_string()));
        assert_eq!(rep.reused.len(), 1);
    }
}
