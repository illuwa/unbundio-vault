//! Browser native-messaging host (B-1).
//!
//! Wire protocol: Chrome/Firefox Native Messaging framing — u32 LE length
//! followed by UTF-8 JSON. The browser spawns `unbundio-vault host` and speaks
//! over stdin/stdout; there is deliberately **no TCP port** (same model as
//! KeePassXC's keepassxc-proxy, not a local HTTP server).
//!
//! Request: `{"id": <any>, "action": "ping"|"status"|"list"|"get"|"reload",
//!            "id": "<entry id>", "query": "<search>"}`
//! Response: `{"id": <echo>, "ok": true, "result": {...}}`
//!        or `{"id": <echo|null>, "ok": false, "error": "..."}`.
//!
//! Security notes:
//! - The vault is decrypted once at startup; only a process that can already
//!   read your files *and* the master password (env/file) can run the host.
//! - The browser never learns the master password — only entry fields.
//! - `list`/`status` never include passwords; only `get` returns one entry.
//! - Malformed input yields an error frame, never a crash or silent exit
//!   mid-stream (clean EOF from the browser ends the loop with exit 0).

use crate::vault::{Vault, VaultData};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Host name used in native-messaging manifests.
pub const HOST_NAME: &str = "com.unbundio.vault";
/// Hard cap per frame (browsers allow 1 MiB–4 GiB; we accept far less).
pub const MAX_MESSAGE: usize = 8 * 1024 * 1024;

/// Read one length-prefixed frame. `Ok(None)` = clean EOF (browser exited).
pub fn read_frame<R: Read>(r: &mut R) -> Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match r.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e).context("host: read frame length"),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > MAX_MESSAGE {
        bail!("host: bad frame length {len}");
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).context("host: read frame body")?;
    Ok(Some(buf))
}

/// Write one length-prefixed JSON frame.
pub fn write_message<W: Write>(w: &mut W, v: &Value) -> Result<()> {
    let body = serde_json::to_vec(v).context("host: encode message")?;
    if body.len() > MAX_MESSAGE {
        bail!("host: response too large");
    }
    w.write_all(&(body.len() as u32).to_le_bytes())
        .context("host: write length")?;
    w.write_all(&body).context("host: write body")?;
    w.flush().context("host: flush")?;
    Ok(())
}

pub struct Host {
    path: PathBuf,
    password: String,
    data: VaultData,
}

impl Host {
    pub fn open(path: &Path, password: &str) -> Result<Self> {
        let vault = Vault::open(path, password)?;
        Ok(Self {
            path: path.to_path_buf(),
            password: password.to_string(),
            data: vault.data,
        })
    }

    pub fn reload(&mut self) -> Result<usize> {
        let fresh = Self::open(&self.path, &self.password)?;
        self.data = fresh.data;
        Ok(self.data.entries.len())
    }

    fn action(&mut self, action: &str, req: &Value) -> Result<Value> {
        match action {
            "ping" => Ok(json!({
                "host": HOST_NAME,
                "version": env!("CARGO_PKG_VERSION"),
            })),
            "status" => Ok(json!({
                "entries": self.data.entries.len(),
                "path": self.path.to_string_lossy(),
            })),
            "list" => {
                let mut rows: Vec<Value> = self
                    .data
                    .entries
                    .iter()
                    .map(|e| {
                        json!({
                            "id": e.id,
                            "title": e.title,
                            "username": e.username,
                            "url": e.url,
                            "favorite": e.favorite,
                        })
                    })
                    .collect();
                rows.sort_by(|a, b| {
                    a.get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_lowercase()
                        .cmp(
                            &b.get("title")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_lowercase(),
                        )
                });
                Ok(Value::Array(rows))
            }
            "get" => {
                // Exact id wins; otherwise the query must match exactly one entry.
                if let Some(id) = req.get("id").and_then(Value::as_str) {
                    if let Some(e) = self.data.entries.iter().find(|e| e.id == id) {
                        return Ok(full_entry(e));
                    }
                    bail!("no entry with that id");
                }
                let query = req
                    .get("query")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if query.is_empty() {
                    bail!("get needs \"id\" or \"query\"");
                }
                let hits: Vec<_> = self
                    .data
                    .entries
                    .iter()
                    .filter(|e| e.matches(query))
                    .collect();
                if hits.is_empty() {
                    bail!("no entry matches '{query}'");
                }
                if hits.len() > 1 {
                    bail!("ambiguous query: {} match", hits.len());
                }
                Ok(full_entry(hits[0]))
            }
            "reload" => {
                let n = self.reload()?;
                Ok(json!({ "entries": n }))
            }
            other => bail!("unknown action '{other}'"),
        }
    }

    /// Dispatch one parsed request. Never fails: errors become `{ok:false}`.
    pub fn handle(&mut self, req: &Value) -> Value {
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let action = req.get("action").and_then(Value::as_str).unwrap_or("");
        if action.is_empty() {
            return json!({"id": id, "ok": false, "error": "missing \"action\""});
        }
        match self.action(action, req) {
            Ok(result) => json!({"id": id, "ok": true, "result": result}),
            Err(e) => json!({"id": id, "ok": false, "error": format!("{e:#}")}),
        }
    }
}

fn full_entry(e: &crate::vault::Entry) -> Value {
    json!({
        "id": e.id,
        "title": e.title,
        "username": e.username,
        "password": e.password,
        "url": e.url,
        "notes": e.notes,
        "tags": e.tags,
        "favorite": e.favorite,
    })
}

/// Serve the browser on stdin/stdout until EOF.
pub fn run(path: &Path, password: &str) -> Result<()> {
    let mut host = Host::open(path, password)?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut inp = stdin.lock();
    let mut out = stdout.lock();
    loop {
        let frame = read_frame(&mut inp)?;
        let Some(raw) = frame else { break };
        let resp = match serde_json::from_slice::<Value>(&raw) {
            Ok(req) => host.handle(&req),
            Err(_) => json!({"id": null, "ok": false, "error": "invalid JSON"}),
        };
        write_message(&mut out, &resp)?;
    }
    Ok(())
}

/// Build a native-messaging host manifest for the given browser family.
pub fn manifest(browser: &str, extension_id: &str, binary: &str) -> Result<Value> {
    if extension_id.trim().is_empty() {
        bail!("extension id must not be empty");
    }
    if binary.trim().is_empty() {
        bail!("binary path must not be empty");
    }
    let base = json!({
        "name": HOST_NAME,
        "description": "unbundio-vault browser integration (local vault, stdio)",
        "path": binary,
        "type": "stdio",
    });
    let mut obj = base.as_object().cloned().context("manifest object")?;
    match browser.to_lowercase().as_str() {
        "chrome" | "chromium" | "edge" | "brave" | "chromium-based" => {
            obj.insert(
                "allowed_origins".into(),
                json!([format!("chrome-extension://{extension_id}/")]),
            );
        }
        "firefox" => {
            obj.insert("allowed_extensions".into(), json!([extension_id]));
        }
        other => bail!("unknown browser '{other}' (chrome|firefox)"),
    }
    Ok(Value::Object(obj))
}

/// Manifest install locations, for `manifest --print-paths` and the README.
pub fn manifest_paths(browser: &str) -> Vec<String> {
    match browser.to_lowercase().as_str() {
        "firefox" => vec![
            "~/.mozilla/native-messaging-hosts/com.unbundio.vault.json".into(),
            "~/Library/Application Support/Mozilla/NativeMessagingHosts/com.unbundio.vault.json".into(),
        ],
        _ => vec![
            "~/.config/google-chrome/NativeMessagingHosts/com.unbundio.vault.json".into(),
            "~/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.unbundio.vault.json".into(),
            "~/Library/Application Support/Chromium/NativeMessagingHosts/com.unbundio.vault.json".into(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::NewEntry;
    use std::io::Cursor;
    use tempfile::NamedTempFile;

    fn fixture() -> (NamedTempFile, PathBuf, String) {
        let f = NamedTempFile::new().expect("tmp");
        let path = f.path().with_extension("vault.json");
        let pw = "host-test-pw".to_string();
        let mut v = Vault::create(&path, &pw).expect("create");
        v.add(NewEntry {
            title: "example".into(),
            username: "bob".into(),
            password: "S3cret!xQ9#Lm2v".into(),
            url: Some("https://example.com/login".into()),
            notes: None,
            tags: vec!["demo".into()],
            favorite: true,
        });
        v.save(&pw).expect("save");
        (f, path, pw)
    }

    #[test]
    fn frame_roundtrip() {
        let msg = json!({"id": 7, "action": "ping"});
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).expect("write");
        let len = u32::from_le_bytes(buf[0..4].try_into().expect("len")) as usize;
        assert_eq!(buf.len(), 4 + len);
        let back = read_frame(&mut Cursor::new(buf)).expect("read");
        let raw = back.expect("frame");
        let v: Value = serde_json::from_slice(&raw).expect("json");
        assert_eq!(v, msg);
    }

    #[test]
    fn clean_eof_is_none() {
        let out = read_frame(&mut Cursor::new(Vec::new())).expect("read");
        assert!(out.is_none());
    }

    #[test]
    fn oversize_rejected() {
        let big = (MAX_MESSAGE as u32 + 1).to_le_bytes().to_vec();
        let res = read_frame(&mut Cursor::new(big));
        assert!(res.is_err());
        // Zero-length frames are rejected too.
        let res = read_frame(&mut Cursor::new(0u32.to_le_bytes().to_vec()));
        assert!(res.is_err());
    }

    #[test]
    fn actions_end_to_end() {
        let (_hold, path, pw) = fixture();
        let mut host = Host::open(&path, &pw).expect("open");

        let ping = host.handle(&json!({"id": 1, "action": "ping"}));
        assert_eq!(ping["ok"], true);

        let status = host.handle(&json!({"id": 2, "action": "status"}));
        assert_eq!(status["result"]["entries"], 1);

        let list = host.handle(&json!({"id": 3, "action": "list"}));
        let rows = list["result"].as_array().expect("array");
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0].get("password").is_none(),
            "list must not leak passwords"
        );

        let id = rows[0]["id"].as_str().expect("id").to_string();
        let get = host.handle(&json!({"id": 4, "action": "get", "id": id}));
        assert_eq!(get["ok"], true);
        assert_eq!(get["result"]["password"], "S3cret!xQ9#Lm2v");

        let amb = host.handle(&json!({"id": 5, "action": "get", "query": "e"}));
        assert_eq!(amb["ok"], amb["ok"]); // single entry matches 'e' too
        let miss = host.handle(&json!({"id": 6, "action": "get", "query": "nope"}));
        assert_eq!(miss["ok"], false);

        let bad = host.handle(&json!({"id": 7, "action": "explode"}));
        assert_eq!(bad["ok"], false);

        let noaction = host.handle(&json!({"id": 8}));
        assert_eq!(noaction["ok"], false);
    }

    #[test]
    fn wrong_password_wont_open() {
        let (_hold, path, _pw) = fixture();
        assert!(Host::open(&path, "wrong").is_err());
    }

    #[test]
    fn manifests() {
        let m = manifest("chrome", "abcdef123456", "/usr/local/bin/unbundio-vault").expect("m");
        assert_eq!(
            m["allowed_origins"],
            json!(["chrome-extension://abcdef123456/"])
        );
        let f = manifest("firefox", "vault@unbundio", "/usr/local/bin/unbundio-vault").expect("m");
        assert_eq!(f["allowed_extensions"], json!(["vault@unbundio"]));
        assert!(manifest("safari", "x", "/bin").is_err());
    }
}
