# Mobile contract (C-2 foundation)

Everything a phone client needs is already in this repo. This document states
what exists, what a client must do, and what is deliberately not built yet.

## Verified today

The vault core cross-compiles for both phone targets, enforced in CI
(`.github/workflows/ci.yml` → `mobile-core`):

| Target | Command | State |
|---|---|---|
| iOS device (arm64) | `cargo build --release --lib --target aarch64-apple-ios` | ✅ |
| Android arm64 | `cargo build --release --lib --target aarch64-linux-android` | ✅ |
| Android armv7 | `cargo build --release --lib --target armv7-linux-androideabi` | ✅ |

Only the **library** cross-compiles. Linking a final app binary additionally
needs Xcode (iOS) and the Android NDK, which is why `cargo build --target
aarch64-linux-android` for the *bin* fails on a Mac without the NDK. That is
expected; the mobile shells own their own linking.

## The three contracts a mobile client codes against

### 1. Vault file (no new format)

Same file the CLI and the desktop app use. A phone is just another device.

```
{ "version": 1, "salt": "<b64 16B>", "nonce": "<b64 12B>", "ciphertext": "<b64>" }
```

Plaintext inside is `VaultData` = `{ "entries": [Entry…] }`, `Entry` =
`{id, title, username, password, url?, notes?, tags[], favorite, created_at,
updated_at}` (`src/vault.rs`). Encryption is Argon2id + AES-256-GCM with a
fresh salt and nonce on every write. Never fork this format for mobile —
a divergence would strand users on one client.

### 2. Sync transport (already a file)

```sh
unbundio-vault sync <shared-path>            # pull, then push
```

Merge rules are defined in `Vault::sync_pull`: union by entry id, newer
`updated_at` wins, same-entry edits reported as conflicts. A phone app
implements the same three rules over whatever transport it has (iCloud
ubiquity container, Google Drive/SAF, Syncthing, a local LAN folder). No
unbundio server is involved, so there is nothing to sign up for.

### 3. What a native shell must add

The core cannot do these; they are platform features, not crypto:

| Capability | iOS | Android |
|---|---|---|
| Unlock | Face ID / Touch ID gating the master password | BiometricPrompt |
| Autofill | `ASCredentialProviderExtension` | `AutofillService` |
| Store the vault | Keychain / iCloud container | Keystore / SAF document |
| Background refresh | BGTask | WorkManager |

Both autofill systems require the app to answer a credential request from the
OS **inside a process the OS controls**, which is why a phone app cannot be a
thin CLI wrapper the way the browser extension is.

## Device pairing (designed, not implemented)

The intended flow avoids typing the master password on a phone:

1. Desktop runs `unbundio-vault pair` and shows a QR containing the shared
   file location plus a one-time pairing secret (not the master password).
2. The phone scans it, saves the location, and the user types the master
   password **once** — or, better, the desktop exports a vault encrypted
   under a per-device key and the phone wraps that key once.
3. From then on both sides run the same pull/push merge.

## Honest status

Not built: any phone app, the pairing command, biometric gating, OS autofill,
or a signing pipeline. The core is ready for them; the shells are a separate,
larger milestone. This file exists so the next person (or agent) does not have
to rediscover what is already guaranteed.