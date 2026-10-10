# Mobile contract (C-2 foundation)

Everything a phone client needs is already in this repo. This document states
what exists, what a client must do, and what is deliberately not built yet.

## The C ABI (built)

`src/ffi.rs` exposes the core to native shells. Rules it enforces, because it is
a trust boundary:

- **No panic ever crosses FFI.** Every entry point wraps its body in
  `catch_unwind`; failure is a null pointer or a non-zero code.
- **Strings are owned by Rust.** Swift/Kotlin must copy and call
  `uv_string_free`.
- **Handles are opaque** and `VaultSession.deinit` closes them, so a decrypted
  vault cannot leak in memory.
- **`uv_list_json` never returns passwords**; only `uv_get_json` does, for one
  entry.

| Function | Purpose |
|---|---|
| `uv_open(path, password)` | decrypt; the expensive Argon2id call |
| `uv_create(path, password)` | create a vault file, 0 = ok |
| `uv_close(handle)` | release |
| `uv_status_json(h)` | `{entries, path}` |
| `uv_list_json(h)` | entries, **no passwords** |
| `uv_get_json(h, id)` | one entry, with password |
| `uv_audit_json(h)` | weak / reused / old |
| `uv_generate(len)` | password, clamped 1..=256, never fails |
| `uv_last_error()` | message for the last failure |

`mobile/ios/UnbundioVault.h` mirrors it; `mobile/ios/check-abi.sh` fails CI if
the two drift (a mismatch otherwise only shows up as an Xcode link error).

## Verified today

The vault core cross-compiles for every phone target, enforced in CI
(`.github/workflows/ci.yml` → `mobile-core`, running on macOS):

| Target | Command | State |
|---|---|---|
| iOS device (arm64) | `cargo build --release --lib --target aarch64-apple-ios` | ✅ 19 MB `.a` |
| iOS simulator | `cargo build --release --lib --target aarch64-apple-ios-sim` | ✅ |
| Android arm64 | `cargo build --release --lib --target aarch64-linux-android` | ✅ |
| Android armv7 | `cargo build --release --lib --target armv7-linux-androideabi` | ✅ |

`crate-type` is `["lib", "staticlib"]` — deliberately **not** `cdylib`, because
producing a `.so` needs a platform linker even for `--lib`, which would break
cross-compiling the core on a plain CI machine.

## The iOS shell (built, not yet run)

[`mobile/ios/`](../mobile/ios) holds a SwiftUI app:

- `UnlockView` — master password, plus **Face ID** unlock via `LocalAuthentication`
  (the password is kept in the Keychain behind `WhenUnlockedThisDeviceOnly`)
- `VaultListView` — search, audit badge, reveal sheet, 30-second clipboard clear
- `GeneratorView` — length control, copy
- `build.sh sim|device` — builds the static library and compiles the Swift
  against the C header, so a signature mistake fails outside Xcode

**Not done:** an Xcode project/Info.plist, code signing, and actually running it
in a simulator. The Swift compiles and links conceptually, but nothing here has
been launched on a device or simulator — do not assume the UI has been seen.

## What a native shell must still add

The core cannot do these; they are platform features, not crypto:

| Capability | iOS | Android |
|---|---|---|
| Autofill | `ASCredentialProviderExtension` — not built | `AutofillService` — not built |
| Store the vault | Keychain / iCloud container | Keystore / SAF document |
| Background refresh | BGTask | WorkManager |

Both autofill systems require the app to answer a credential request from the
OS **inside a process the OS controls**, which is why a phone app cannot be a
thin CLI wrapper the way the browser extension is.

### Unlocks

| Capability | iOS | Android |
|---|---|---|
| Biometric unlock | Face ID via `LocalAuthentication` — **done** in `mobile/ios` | `BiometricPrompt` — not built |

## The data contracts a mobile client codes against

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

### 3. Sync and unlocks

Sync is section 2 above; unlocks are platform features (see "What a native
shell must still add").

## Device pairing (designed, not implemented)

The intended flow avoids typing the master password on a phone:

1. Desktop runs `unbundio-vault pair` and shows a QR containing the shared
   file location plus a one-time pairing secret (not the master password).
2. The phone scans it, saves the location, and the user types the master
   password **once** — or, better, the desktop exports a vault encrypted
   under a per-device key and the phone wraps that key once.
3. From then on both sides run the same pull/push merge.

## Honest status

Built: the C ABI, four cross-compiled static libraries, and a SwiftUI shell
that compiles for both the iOS simulator and device (Face ID unlock, search,
reveal, copy, generator).

Not built: an Xcode project or signing, **anything actually launched** on a
simulator or device, the Android shell, OS autofill providers, and the pairing
command. The Swift compiles; nobody has looked at the UI yet — do not assume
the screens have been seen.