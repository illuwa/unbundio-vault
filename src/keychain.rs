//! Password-less unlock via the OS secret store.
//!
//! macOS: the system Keychain, driven through `/usr/bin/security` — no crate
//! dependency, no FFI, and the item is unlocked by the login session (Touch
//! ID on modern macOS). That is what "password-less" means for a desktop
//! helper: the master password is typed once at setup, then kept by the OS.
//!
//! Other platforms: not implemented here. `status` reports it honestly rather
//! than pretending.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

pub const SERVICE: &str = "com.unbundio.vault";

/// The keychain account name: the vault file name, so two vaults don't clash.
pub fn account_for(vault: &Path) -> String {
    vault
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "vault".to_string())
}

#[cfg(target_os = "macos")]
fn security() -> Result<Command> {
    let p = Path::new("/usr/bin/security");
    if !p.exists() {
        bail!("macOS 'security' tool not found at {}", p.display());
    }
    Ok(Command::new(p))
}

#[cfg(target_os = "macos")]
/// Store the master password in the login keychain. `-U` updates an existing
/// item. Pass `-T` empty is not needed: this app has no UI to authorize, and
/// `security` itself authorizes reads via the login session.
pub fn store(vault: &Path, password: &str) -> Result<()> {
    if password.is_empty() {
        bail!("refusing to store an empty password");
    }
    security()?
        .args([
            "add-generic-password",
            "-U",
            "-s",
            SERVICE,
            "-a",
            &account_for(vault),
            "-w",
            password,
        ])
        .output()
        .context("cannot run security add-generic-password")?;
    Ok(())
}

#[cfg(target_os = "macos")]
/// Read the password back. `Ok(None)` = no item (never prompt on stderr).
pub fn retrieve(vault: &Path) -> Result<Option<String>> {
    let out = security()?
        .args([
            "find-generic-password",
            "-s",
            SERVICE,
            "-a",
            &account_for(vault),
            "-w",
        ])
        .output()
        .context("cannot run security find-generic-password")?;
    if !out.status.success() {
        return Ok(None);
    }
    let pw = String::from_utf8_lossy(&out.stdout)
        .trim_end_matches('\n')
        .to_string();
    if pw.is_empty() {
        return Ok(None);
    }
    Ok(Some(pw))
}

#[cfg(target_os = "macos")]
pub fn remove(vault: &Path) -> Result<bool> {
    let out = security()?
        .args([
            "delete-generic-password",
            "-s",
            SERVICE,
            "-a",
            &account_for(vault),
        ])
        .output()
        .context("cannot run security delete-generic-password")?;
    Ok(out.status.success())
}

#[cfg(target_os = "macos")]
pub fn supported() -> bool {
    Path::new("/usr/bin/security").exists()
}

#[cfg(not(target_os = "macos"))]
pub fn store(_vault: &Path, _password: &str) -> Result<()> {
    bail!("keychain storage is macOS-only in v0.1 (use --password-file or the 0600 file)")
}

#[cfg(not(target_os = "macos"))]
pub fn retrieve(_vault: &Path) -> Result<Option<String>> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
pub fn remove(_vault: &Path) -> Result<bool> {
    Ok(false)
}

#[cfg(not(target_os = "macos"))]
pub fn supported() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_is_file_name() {
        assert_eq!(
            account_for(Path::new("/home/u/unbundio-vault.vault")),
            "unbundio-vault.vault"
        );
        assert_eq!(account_for(Path::new("/")), "vault");
    }
}
