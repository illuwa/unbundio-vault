# unbundio-vault

**Your passwords. Your machine. No subscription.**

unbundio-vault is a local-first, free password manager from the unbundio
portfolio — a clean-room alternative built for users being forced off paid
password SaaS. No account, no cloud,
no telemetry, no subscription. Your vault is a single AES-256-GCM file
unlocked by your master password (Argon2id).

> **Portfolio note:** unbundio-vault is the free personal/offline companion to
> unbundio's `keep-my-password` (team E2EE sharing SaaS). Same person, two jobs:
> personal secrets here ($0 forever), team sharing there. CSV moves both ways.

## Why this exists (researched 2026-10-09)

Online communities, Reddit and SaaS pricing trackers all point the same way:
the "free forever" era is contracting, and password managers moved first.

| Signal | Source |
|---|---|
| **Dashlane Free discontinued Sept 16, 2025.** Free users got a Premium trial, then read-only export-or-pay. Paid Premium ≈ $60/yr. | Dashlane official blog (2025-08-05), FAQ, PCMag, PCWorld |
| **1Password: no free tier at all** (14-day trial only). Individual from ≈ $2.39/mo. | PasswordManager.com comparison, vendor pricing |
| **Category-wide shift:** ToolsRanks 2025–2026 tracker lists Dashlane + 1Password together under "password managers moved together". | ToolsRanks SaaS Price Hikes & Free-Tier Cuts |
| **Same pattern elsewhere:** Mailchimp free cut 2,000 → 250 contacts (−96%), Postman Free limited to 1 user (Mar 2026, Team $19/user/mo), Asana free capped, Teachable/Thinkific/Podia free removed. SaaS inflation ≈ 11.4% vs 2.7% market. | GroupMail/Brevo/Mailchimp trackers, Crosscheck Postman comparison, r/SaaS, SaaStr |
| **r/selfhosted + r/SaaS demand:** "Postman 1-user limit — are you paying now?", "looking for free Dashlane alternative", NocoDB 63k★ / Bruno 44k★ as proof that local-first clones win. | Reddit, GitHub stars |

unbundio-vault targets the sharpest pain: **forced migration with a deadline**
(Dashlane's Sept 2025 cutoff), with a scope small enough to clone cleanly:
vault + generator + CSV migration + audit.

## Clean-room statement

- No code, icons, binaries, or file formats copied from Dashlane, 1Password,
  Bitwarden, or any other manager. No Adobe-style asset reuse either.
- Interoperability only: CSV import accepts the *documented column shapes*
  those tools export (title/url/username/password), same as any spreadsheet.
- Crypto composed from public primitives per open specs: Argon2 (RFC 9106),
  AES-GCM (NIST SP 800-38D). Dependencies are permissive
  (MIT/Apache-2.0/BSD/ISC) — see `cargo tree -e normal --prefix none`.
- This repo contains zero real credentials. Tests use synthetic passwords.

## Quick start

**No Rust needed:** download
`unbundio-vault-0.1.0-macos-universal.zip` from
[Releases](https://github.com/illuwa/unbundio-vault/releases), unzip,
`./install.sh` — full steps in `INSTALL.ko.md` (also at
[`packaging/INSTALL.ko.md`](packaging/INSTALL.ko.md)). Free forever, MIT.

From source:

```sh
cargo build --release
./target/release/unbundio-vault init
./target/release/unbundio-vault add --title github --username alice --generate
./target/release/unbundio-vault list
./target/release/unbundio-vault get github        # masked; add --show to reveal
./target/release/unbundio-vault audit
./target/release/unbundio-vault gen --length 24
```

Non-interactive (scripts/CI):

```sh
export UNBUNDIO_VAULT=~/unbundio-vault.vault UNBUNDIO_VAULT_PASSWORD='…'
unbundio-vault list --json
```

### Migrating off Dashlane / Chrome / 1Password

1. In your old manager: Export → CSV (keep the file private, delete after).
2. `unbundio-vault import --path export.csv`
3. `unbundio-vault audit` → fix `weak` / `reused`.
4. Delete the CSV: `shred -u export.csv` (or Secure Empty Trash).

```sh
unbundio-vault export --format csv --out backup.csv   # PLAINTEXT warning intended
unbundio-vault import --path backup.csv
```

## Security model

- **KDF:** Argon2id default params (`m=19MiB, t=2, p=1`), 16-byte random salt.
- **Cipher:** AES-256-GCM, fresh 12-byte nonce per save, wrong-password and
  corruption both surface as `decryption failed` (no oracle).
- **Storage:** one JSON envelope `{version, salt, nonce, ciphertext}`.
  Writes are atomic (temp file + rename). No network calls, no telemetry.
- **Memory:** derived keys are zeroized after use (`zeroize` crate).
- **Deletes:** ambiguous text queries refuse to delete; use the exact id.
- **Not yet:** browser autofill, mobile sync, attachments, breach-API lookup,
  hardware-key unlock. Deliberately out of scope for v0.1 — see Roadmap.

Threat model: protects a stolen laptop file and a curious process list.
Does not protect a compromised machine (keyloggers, memory scrapers) — no
password manager does.

## Commands

| Command | What it does |
|---|---|
| `init` | create empty vault (prompts twice) |
| `add [--generate]` | add login; prompts for missing fields |
| `get <query> [--show]` | show one entry (masked by default) |
| `list [--search] [--tag] [--json]` | list entries |
| `update <query> [--title …]` | update fields |
| `rm <query>` | delete (exact id when ambiguous) |
| `gen [--length] [--no-symbols] …` | random password, no vault needed |
| `export --format csv\|json --out` | plaintext backup (warns) |
| `import --path export.csv` | Chrome/Dashlane/1Password CSV |
| `audit [--json]` | weak / reused / old report |
| `keychain save\|delete\|status` | password-less unlock in the macOS keychain |
| `passwd` | change master password (re-encrypts) |
| `backup [--dir DIR]` | timestamped copy of the encrypted vault |
| `sync <REMOTE> [--direction]` | push/pull/both against a shared encrypted vault file |
| `host` | serve the browser over Native Messaging stdio |
| `manifest --extension-id …` | print host manifest JSON |
| `install-extension --extension-id …` | install manifest + optional 0600 host password |

Global: `--vault <path>`, `--password-file <path>`, `--json`,
`$UNBUNDIO_VAULT`, `$UNBUNDIO_VAULT_PASSWORD`.

## Sync across your own devices

`remote` is **any shared folder** — a synced cloud folder (iCloud Drive,
Syncthing, Dropbox…), a USB stick, an S3 bucket you mount. There is no
unbundio server, no account, no plan: only your already-encrypted vault file
moves, so a folder provider sees ciphertext and nothing else.

```sh
unbundio-vault sync ~/Library/Mobile\ Documents/com.unbundio.vault/sync.vault
unbundio-vault sync ~/sync.vault --direction pull   # fetch only
unbundio-vault sync ~/sync.vault --direction push   # publish only
unbundio-vault sync ~/sync.vault --dry-run          # report, change nothing
```

How it works: entries are merged by id, and on a conflict the newer
`updated_at` wins, so two devices that edited *different* entries both keep
their change. Both sides must use the **same master password** (the file is
encrypted with it). Pulling is idempotent — running it twice adds nothing.

Honest limits: no per-entry history, and two devices editing the *same*
entry offline keep the later change only (last-writer-wins, not a 3-way
merge). Good enough for one person's devices; not a team-sharing model —
that is `keep-my-password`'s job.

## Browser integration (B-1: native host)

No local TCP server — same model as KeePassXC's proxy: the browser spawns
`unbundio-vault host` and speaks length-prefixed JSON over stdin/stdout
(Chrome/Firefox Native Messaging framing). The browser never learns the
master password; only `get` returns a password, `list`/`status` never do.

Because the native-messaging manifest cannot pass a subcommand, the binary
also accepts the browser's raw invocation form — `unbundio-vault
chrome-extension://<id>/` behaves exactly like `unbundio-vault host`.

```sh
# 1. print the manifest for your extension id
unbundio-vault manifest --browser chrome --extension-id <EXT_ID> \
  --binary ~/.cargo/bin/unbundio-vault --print-paths \
  > ~/Library/Application\ Support/Google/Chrome/NativeMessagingHosts/com.unbundio.vault.json

# 2. the host needs the vault password from the environment, e.g. launchd:
#    UNBUNDIO_VAULT=~/unbundio-vault.vault UNBUNDIO_VAULT_PASSWORD='…'
```

Protocol: `{"id":1,"action":"ping"|"status"|"list"|"get"|"reload","id":"…","query":"…"}`
→ `{"id":1,"ok":true,"result":{…}}` or `{"id":1,"ok":false,"error":"…"}`.
Malformed frames get error responses (or a clean exit on EOF) — never a crash.

### Install-only setup (no server to run)

```sh
cargo install --path .                      # once: puts unbundio-vault on PATH
unbundio-vault --vault ~/unbundio-vault.vault init   # once: create vault
unbundio-vault install-extension --extension-id <ID from chrome://extensions>
# -> writes the host manifest, then asks (default No) to save the host
#    password in a 0600 file so the browser-spawned host can unlock unattended
```

Then in Chrome: Developer mode → Load unpacked → this repo's `extension/`
folder → toolbar icon → Fill. Passwords are fetched per click; the browser
never sees the master password.

Trade-off, stated plainly: the 0600 host-password file means anyone who can
already read your files can unlock the vault — same bar as the vault file
itself, weaker than typing the password each time. Skip it and the host only
unlocks via `$UNBUNDIO_VAULT_PASSWORD` / `--password-file` (e.g. launchd env).

Next (B-2 done, needs a real-browser trial): Chrome extension MVP (popup
search → one-click fill) speaking this protocol. Then sync + mobile (Roadmap).

## Development

```sh
cargo test                       # 30 tests: crypto, generator, vault, csv, audit, host, sync, keychain
cargo clippy --all-targets
cargo fmt --check
node tests/browser-fill.mjs      # browser e2e (needs playwright; see below)
```

`tests/browser-fill.mjs` boots a real Chromium with the extension loaded and
proves the whole chain: host spawn → stdio protocol → decrypted list → content
script filling a real login form → popup rendering. It needs `npm i -D
playwright` (or `PLAYWRIGHT_DIR` pointing at an existing install). One hop
cannot be automated and is verified by hand instead: the **toolbar click**
that grants `activeTab`, which Chrome only grants from a real user gesture.

## Roadmap

- [x] B-1 `host` (Native Messaging stdio) + `manifest` generator
- [x] B-2 Chrome extension MVP (`extension/`: popup search → Fill/Copy) + `install-extension` (no server, 0600 host password with consent)
- [x] Real-browser trial (load unpacked → Fill on a live login page)
- [x] `backup` (encrypted, restorable)
- [x] C-1 sync: file-based `sync push/pull` over any shared folder — no unbundio
      server, no accounts. (Rejected relay reuse: keep-my-password's relay needs
      accounts + billing, which would break this project's free-forever promise.)
      Merge = union by entry id, newer `updated_at` wins, conflicts reported.
- [x] sync conflict reporting (same-entry edits are named, not silently dropped)
- [x] pw-less unlock via the macOS keychain (`keychain save`), no crate deps
- [x] popup: generator, audit summary, keyboard navigation, autoselect by site
- [x] browser e2e harness (`tests/browser-fill.mjs`, 14 checks in real Chromium)
- [ ] C-2 mobile. **Core + iOS shell are built**: the library cross-compiles for
      `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-linux-android` and
      `armv7-linux-androideabi`, exposed over a C ABI (`src/ffi.rs`), with a
      SwiftUI shell in [`mobile/ios/`](mobile/ios) that unlocks with Face ID,
      searches, reveals and copies, plus a generator. CI builds all targets and
      checks the header against the Rust exports. **Not yet done:** running it in
      a simulator/phone (needs an Xcode project + signing), Android shell, and
      the OS autofill providers (`ASCredentialProviderExtension`,
      `AutofillService`). See [docs/mobile-contract.md](docs/mobile-contract.md).
- [ ] C-2 mobile (shared Rust core + thin native UI, biometrics, OS autofill)
- [ ] `totp` field + `get --totp` (RFC 6238, local clock)
- [ ] `unbundio-vault serve --port` loopback autofill helper (Type-to-app stays manual until then)
- [ ] Encrypted JSON backup with re-import integrity check
- [ ] `audit --fix` interactive weak-password rotation
- [ ] Passkey notes field (not full WebAuthn — out of scope)

## License

MIT — see [LICENSE-MIT](LICENSE-MIT).
