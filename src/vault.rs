//! Encrypted vault storage: load / save / CRUD over an AES-GCM file.

use crate::crypto::{self, EncryptedVault};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub username: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Entry {
    pub fn matches(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        if self.id.to_lowercase().starts_with(&q) {
            return true;
        }
        self.title.to_lowercase().contains(&q)
            || self.username.to_lowercase().contains(&q)
            || self
                .url
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains(&q)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VaultData {
    #[serde(default)]
    pub entries: Vec<Entry>,
}

#[derive(Debug)]
pub struct Vault {
    pub path: PathBuf,
    pub data: VaultData,
}

#[derive(Debug, Clone, Default)]
pub struct NewEntry {
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
}

impl Vault {
    /// Create a brand-new empty vault file. Fails if the file already exists.
    pub fn create(path: &Path, password: &str) -> Result<Self> {
        if path.exists() {
            bail!("vault already exists at {}", path.display());
        }
        let vault = Self {
            path: path.to_path_buf(),
            data: VaultData { entries: vec![] },
        };
        vault.save(password)?;
        Ok(vault)
    }

    /// Open + decrypt an existing vault.
    pub fn open(path: &Path, password: &str) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read vault {}", path.display()))?;
        let ev: EncryptedVault =
            serde_json::from_str(&raw).context("vault file is not valid JSON")?;
        let plain = crypto::decrypt(&ev, password).map_err(|e| anyhow::anyhow!("{e}"))?;
        let data: VaultData = serde_json::from_slice(&plain).context("vault payload is corrupt")?;
        Ok(Self {
            path: path.to_path_buf(),
            data,
        })
    }

    /// Encrypt + atomically replace the vault file.
    pub fn save(&self, password: &str) -> Result<()> {
        let plain = serde_json::to_vec(&self.data).context("encode vault")?;
        let ev = crypto::encrypt(&plain, password).map_err(|e| anyhow::anyhow!("{e}"))?;
        let text = serde_json::to_string_pretty(&ev).context("encode envelope")?;
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("cannot create dir {}", parent.display()))?;
            }
        }
        // Atomic write: temp file + rename.
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, text).with_context(|| format!("cannot write {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("cannot replace {}", self.path.display()))?;
        Ok(())
    }

    pub fn add(&mut self, n: NewEntry) -> &Entry {
        let now = Utc::now().timestamp();
        self.data.entries.push(Entry {
            id: Uuid::new_v4().to_string(),
            title: n.title,
            username: n.username,
            password: n.password,
            url: n.url.filter(|s| !s.trim().is_empty()),
            notes: n.notes.filter(|s| !s.trim().is_empty()),
            tags: n.tags,
            favorite: n.favorite,
            created_at: now,
            updated_at: now,
        });
        let len = self.data.entries.len();
        &self.data.entries[len - 1]
    }

    pub fn find(&self, query: &str) -> Vec<&Entry> {
        self.data
            .entries
            .iter()
            .filter(|e| e.matches(query))
            .collect()
    }

    pub fn find_mut(&mut self, query: &str) -> Vec<&mut Entry> {
        self.data
            .entries
            .iter_mut()
            .filter(|e| e.matches(query))
            .collect()
    }

    /// Remove by id-prefix or query. Returns number removed.
    /// Refuses ambiguous queries (more than one match, none an exact id).
    pub fn remove(&mut self, query: &str) -> Result<usize> {
        let exact: Vec<usize> = self
            .data
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.id == query)
            .map(|(i, _)| i)
            .collect();
        if let Some(&i) = exact.first() {
            self.data.entries.remove(i);
            return Ok(1);
        }
        let prefix_ids: Vec<usize> = self
            .data
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.id.to_lowercase().starts_with(&query.to_lowercase()))
            .map(|(i, _)| i)
            .collect();
        if prefix_ids.len() == 1 {
            self.data.entries.remove(prefix_ids[0]);
            return Ok(1);
        }
        if prefix_ids.len() > 1 {
            bail!("ambiguous id prefix: {} entries match", prefix_ids.len());
        }
        let matches = self
            .data
            .entries
            .iter()
            .filter(|e| e.matches(query))
            .count();
        if matches == 0 {
            bail!("no entry matches '{query}'");
        }
        if matches > 1 {
            bail!("'{query}' matches {matches} entries; delete by exact id instead");
        }
        let before = self.data.entries.len();
        self.data.entries.retain(|e| !e.matches(query));
        Ok(before - self.data.entries.len())
    }

    pub fn touch(&mut self, id: &str) {
        let now = Utc::now().timestamp();
        for e in &mut self.data.entries {
            if e.id == id {
                e.updated_at = now;
            }
        }
    }

    /// Copy the encrypted vault file to `dest` (still encrypted; safe to
    /// upload anywhere). Used by `backup` and the file-based sync story.
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("cannot create dir {}", parent.display()))?;
            }
        }
        std::fs::copy(&self.path, dest).with_context(|| {
            format!(
                "cannot back up {} to {}",
                self.path.display(),
                dest.display()
            )
        })?;
        Ok(())
    }

    /// Publish the encrypted vault to a shared location (sync push).
    /// Only the ciphertext envelope is copied — the target never learns the
    /// master password. Writes via temp file + rename for atomicity.
    pub fn sync_push(&self, remote: &Path) -> Result<()> {
        if let Some(parent) = remote.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("cannot create dir {}", parent.display()))?;
            }
        }
        let raw = std::fs::read(&self.path)
            .with_context(|| format!("cannot read {}", self.path.display()))?;
        let tmp = remote.with_extension("tmp");
        std::fs::write(&tmp, raw).with_context(|| format!("cannot write {}", tmp.display()))?;
        std::fs::rename(&tmp, remote)
            .with_context(|| format!("cannot replace {}", remote.display()))?;
        Ok(())
    }

    /// Merge a remote published vault into this one (sync pull).
    ///
    /// Union by entry id, latest `updated_at` wins on conflict, so two devices
    /// that edited different entries both keep their change. The remote must
    /// use the same master password (it is encrypted with it).
    /// Returns (added, updated).
    pub fn sync_pull(&mut self, remote: &Path, password: &str) -> Result<(usize, usize)> {
        let other = Vault::open(remote, password)?;
        let (mut added, mut updated) = (0usize, 0usize);
        for e in other.data.entries {
            match self.data.entries.iter_mut().find(|x| x.id == e.id) {
                Some(existing) => {
                    if e.updated_at > existing.updated_at {
                        *existing = e;
                        updated += 1;
                    }
                }
                None => {
                    self.data.entries.push(e);
                    added += 1;
                }
            }
        }
        Ok((added, updated))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn tmp_path() -> (NamedTempFile, PathBuf) {
        let f = NamedTempFile::new().expect("tmp");
        let p = f.path().with_extension("vault.json");
        (f, p)
    }

    #[test]
    fn create_open_roundtrip() {
        let (_hold, path) = tmp_path();
        let mut v = Vault::create(&path, "master-pw").expect("create");
        v.add(NewEntry {
            title: "github".into(),
            username: "alice".into(),
            password: "s3cret!".into(),
            ..Default::default()
        });
        v.save("master-pw").expect("save");
        let v2 = Vault::open(&path, "master-pw").expect("open");
        assert_eq!(v2.data.entries.len(), 1);
        assert_eq!(v2.data.entries[0].title, "github");
    }

    #[test]
    fn wrong_password_rejected() {
        let (_hold, path) = tmp_path();
        Vault::create(&path, "right").expect("create");
        assert!(Vault::open(&path, "wrong").is_err());
    }

    #[test]
    fn create_refuses_overwrite() {
        let (_hold, path) = tmp_path();
        Vault::create(&path, "pw").expect("create");
        assert!(Vault::create(&path, "pw").is_err());
    }

    #[test]
    fn backup_is_encrypted_and_restorable() {
        let (_hold, path) = tmp_path();
        let mut v = Vault::create(&path, "pw").expect("create");
        v.add(NewEntry {
            title: "github".into(),
            username: "alice".into(),
            password: "s3cret!".into(),
            ..Default::default()
        });
        v.save("pw").expect("save");

        let dir = tempfile::tempdir().expect("tmpdir");
        let dest = dir.path().join("nested").join("copy.vault");
        v.backup_to(&dest).expect("backup");

        // The backup file must not contain the plaintext secret.
        let raw = std::fs::read_to_string(&dest).expect("read");
        assert!(!raw.contains("s3cret!"), "backup must stay encrypted");
        assert!(raw.contains("ciphertext"));

        // …and it must open with the same master password.
        let restored = Vault::open(&dest, "pw").expect("open backup");
        assert_eq!(restored.data.entries.len(), 1);
        assert_eq!(restored.data.entries[0].password, "s3cret!");
    }

    #[test]
    fn backup_refuses_unwritable_destination() {
        let (_hold, path) = tmp_path();
        let v = Vault::create(&path, "pw").expect("create");
        let dest = Path::new("/proc/definitely/not/writable/x.vault");
        assert!(v.backup_to(dest).is_err());
    }

    #[test]
    fn sync_push_pull_merges_both_sides() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let a_path = dir.path().join("a.vault");
        let b_path = dir.path().join("b.vault");
        let remote = dir.path().join("shared").join("vault.vault");
        let pw = "same-master-pw";

        // Device A creates github, publishes.
        let mut a = Vault::create(&a_path, pw).expect("create a");
        a.add(NewEntry {
            title: "github".into(),
            username: "alice".into(),
            password: "pw-a".into(),
            ..Default::default()
        });
        a.save(pw).expect("save a");
        a.sync_push(&remote).expect("push a");

        // Device B starts empty, pulls github.
        let mut b = Vault::create(&b_path, pw).expect("create b");
        let (added, updated) = b.sync_pull(&remote, pw).expect("pull b");
        assert_eq!((added, updated), (1, 0));
        assert_eq!(b.data.entries[0].title, "github");

        // B adds its own entry and pushes.
        b.add(NewEntry {
            title: "gmail".into(),
            username: "bob".into(),
            password: "pw-b".into(),
            ..Default::default()
        });
        b.save(pw).expect("save b");
        b.sync_push(&remote).expect("push b");

        // A pulls: gains gmail, keeps github.
        let (added, updated) = a.sync_pull(&remote, pw).expect("pull a");
        assert_eq!((added, updated), (1, 0));
        assert_eq!(a.data.entries.len(), 2);
        let titles: Vec<&str> = a.data.entries.iter().map(|e| e.title.as_str()).collect();
        assert!(titles.contains(&"github") && titles.contains(&"gmail"));

        // Pulling again is a no-op (idempotent).
        let (added, updated) = a.sync_pull(&remote, pw).expect("pull again");
        assert_eq!((added, updated), (0, 0));
    }

    #[test]
    fn sync_pull_newer_remote_wins_on_conflict() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let a_path = dir.path().join("a.vault");
        let remote = dir.path().join("shared.vault");
        let pw = "pw";

        let mut a = Vault::create(&a_path, pw).expect("create");
        let id = a
            .add(NewEntry {
                title: "site".into(),
                username: "old".into(),
                password: "old-pw".into(),
                ..Default::default()
            })
            .id
            .clone();
        a.save(pw).expect("save");
        a.sync_push(&remote).expect("push");

        // Simulate a newer remote edit of the same entry.
        let mut remote_vault = Vault::open(&remote, pw).expect("open remote");
        if let Some(e) = remote_vault.data.entries.iter_mut().find(|x| x.id == id) {
            e.username = "new".into();
            e.password = "new-pw".into();
            e.updated_at += 100;
        }
        remote_vault.save(pw).expect("save remote");

        let (added, updated) = a.sync_pull(&remote, pw).expect("pull");
        assert_eq!((added, updated), (0, 1));
        let merged = a.data.entries.iter().find(|x| x.id == id).expect("entry");
        assert_eq!(merged.username, "new");
        assert_eq!(merged.password, "new-pw");
    }

    #[test]
    fn sync_pull_rejects_wrong_master_password() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let a_path = dir.path().join("a.vault");
        let remote = dir.path().join("shared.vault");
        let mut a = Vault::create(&a_path, "right-pw").expect("create");
        a.add(NewEntry {
            title: "x".into(),
            username: "u".into(),
            password: "p".into(),
            ..Default::default()
        });
        a.save("right-pw").expect("save");
        a.sync_push(&remote).expect("push");

        let mut b = Vault::create(&dir.path().join("b.vault"), "right-pw").expect("create b");
        assert!(b.sync_pull(&remote, "wrong-pw").is_err());
        assert_eq!(b.data.entries.len(), 0, "failed pull must not half-merge");
    }
}
