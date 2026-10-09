//! unbundio-vault library: local-first encrypted password vault.
//!
//! Clean-room implementation. No code copied from Dashlane, 1Password,
//! Bitwarden or any other password manager. Crypto is composed from public
//! primitives (Argon2id + AES-256-GCM) per their open specifications.

pub mod audit;
pub mod crypto;
pub mod csv_io;
pub mod generator;
pub mod vault;

pub use vault::{Entry, Vault};
