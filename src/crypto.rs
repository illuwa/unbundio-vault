//! Key derivation (Argon2id) + authenticated encryption (AES-256-GCM).
//!
//! File format (v1, JSON):
//! ```json
//! {"version":1,"salt":"base64(16B)","nonce":"base64(12B)","ciphertext":"base64(AES-GCM(plaintext))"}
//! ```
//! Plaintext is the JSON encoding of [`crate::vault::VaultData`].
//! Wrong master password surfaces as [`CryptoError::DecryptionFailed`].

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("crypto randomness unavailable")]
    Rng,
    #[error("key derivation failed")]
    Kdf,
    #[error("encryption failed")]
    EncryptionFailed,
    #[error("decryption failed (wrong password or corrupt vault)")]
    DecryptionFailed,
    #[error("vault encoding error: {0}")]
    Encoding(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedVault {
    pub version: u32,
    /// base64, 16 bytes
    pub salt: String,
    /// base64, 12 bytes
    pub nonce: String,
    /// base64, AES-256-GCM ciphertext (includes 16B auth tag)
    pub ciphertext: String,
}

pub const VAULT_VERSION: u32 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

fn random_bytes(buf: &mut [u8]) -> Result<(), CryptoError> {
    // OsRng::fill_bytes cannot fail in rand 0.8 (panics only on catastrophic
    // OS failure); map any such state explicitly so callers see a Result.
    // We use try_fill_bytes which is fallible.
    OsRng.try_fill_bytes(buf).map_err(|_| CryptoError::Rng)
}

fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; 32], CryptoError> {
    let argon = Argon2::default();
    let mut key = [0u8; 32];
    argon
        .hash_password_into(password, salt, &mut key)
        .map_err(|_| CryptoError::Kdf)?;
    Ok(key)
}

/// Encrypt `plaintext` under `password`. Fresh random salt + nonce every call.
pub fn encrypt(plaintext: &[u8], password: &str) -> Result<EncryptedVault, CryptoError> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    random_bytes(&mut salt)?;
    random_bytes(&mut nonce_bytes)?;

    let mut key = derive_key(password.as_bytes(), &salt)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    key.zeroize();

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)?;

    Ok(EncryptedVault {
        version: VAULT_VERSION,
        salt: B64.encode(salt),
        nonce: B64.encode(nonce_bytes),
        ciphertext: B64.encode(ciphertext),
    })
}

/// Decrypt a vault. Wrong password and corruption both map to
/// [`CryptoError::DecryptionFailed`] (no oracle).
pub fn decrypt(ev: &EncryptedVault, password: &str) -> Result<Vec<u8>, CryptoError> {
    if ev.version != VAULT_VERSION {
        return Err(CryptoError::Encoding(format!(
            "unsupported vault version {}",
            ev.version
        )));
    }
    let salt = B64
        .decode(ev.salt.as_bytes())
        .map_err(|e| CryptoError::Encoding(e.to_string()))?;
    let nonce_bytes = B64
        .decode(ev.nonce.as_bytes())
        .map_err(|e| CryptoError::Encoding(e.to_string()))?;
    let ciphertext = B64
        .decode(ev.ciphertext.as_bytes())
        .map_err(|e| CryptoError::Encoding(e.to_string()))?;
    if salt.len() != SALT_LEN || nonce_bytes.len() != NONCE_LEN {
        return Err(CryptoError::DecryptionFailed);
    }

    let mut key = derive_key(password.as_bytes(), &salt)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    key.zeroize();

    let nonce = Nonce::from_slice(nonce_bytes.as_slice());
    cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| CryptoError::DecryptionFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let pt = b"{\"entries\":[]}";
        let ev = encrypt(pt, "correct horse battery staple").expect("encrypt");
        let back = decrypt(&ev, "correct horse battery staple").expect("decrypt");
        assert_eq!(back, pt);
    }

    #[test]
    fn wrong_password_fails() {
        let ev = encrypt(b"secret", "pw-one").expect("encrypt");
        let err = decrypt(&ev, "pw-two").unwrap_err();
        assert!(matches!(err, CryptoError::DecryptionFailed));
    }

    #[test]
    fn salt_and_nonce_are_fresh() {
        let a = encrypt(b"x", "pw").expect("encrypt");
        let b = encrypt(b"x", "pw").expect("encrypt");
        assert_ne!(a.salt, b.salt);
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.ciphertext, b.ciphertext);
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let mut ev = encrypt(b"hello", "pw").expect("encrypt");
        let mut raw = B64.decode(ev.ciphertext.as_bytes()).expect("b64");
        if let Some(first) = raw.first_mut() {
            *first ^= 1;
        }
        ev.ciphertext = B64.encode(raw);
        assert!(decrypt(&ev, "pw").is_err());
    }
}
