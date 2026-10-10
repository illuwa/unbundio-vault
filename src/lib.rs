//! unbundio-vault library: local-first encrypted password vault.
//!
//! Clean-room implementation. No code copied from Dashlane, 1Password,
//! Bitwarden or any other password manager. Crypto is composed from public
//! primitives (Argon2id + AES-256-GCM) per their open specifications.

// The C ABI in `ffi` is the one place with raw pointers. Requiring an explicit
// `unsafe` block inside every `unsafe fn` keeps each dereference justified.
#![deny(unsafe_op_in_unsafe_fn)]

pub mod audit;
pub mod crypto;
pub mod csv_io;
pub mod ffi;
pub mod generator;
pub mod host;
pub mod keychain;
pub mod vault;

pub use vault::{Entry, Vault};
