//! C ABI for the native shells (iOS / Android).
//!
//! Design rules, because this is a boundary:
//! - **Never panic across FFI.** Every entry point wraps its body in
//!   `catch_unwind` and reports failure as a null pointer / error code.
//! - **Strings cross as C strings owned by Rust.** The caller must release them
//!   with `uv_string_free`.
//! - **Handles are opaque.** A `*mut VaultHandle` is created by `uv_open` and
//!   destroyed by `uv_close`; it owns the decrypted vault in memory.
//! - **Errors are reported as text**, retrievable with `uv_last_error`, so the
//!   Swift/Kotlin side can show something a human can act on.
//!
//! The format and the sync semantics are identical to the CLI: a mobile client
//! is just another device. See docs/mobile-contract.md.

use crate::generator::GenOptions;
use crate::vault::Vault;
use std::ffi::{c_char, c_int, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Opaque handle owning a decrypted vault.
pub struct VaultHandle {
    vault: Vault,
}

thread_local! {
    static LAST_ERROR: std::cell::RefCell<Option<CString>> = const { std::cell::RefCell::new(None) };
}

fn set_error(msg: &str) {
    let clean = msg.replace('\0', " ");
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = CString::new(clean).ok();
    });
}

fn clear_error() {
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = None;
    });
}

/// Borrow the last error message as a C string owned by the library (valid
/// until the next call on this thread). Never returns null; returns "" when
/// there is no error.
#[no_mangle]
pub extern "C" fn uv_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| match slot.borrow().as_ref() {
        Some(s) => s.as_ptr(),
        None => c"".as_ptr(),
    })
}

/// Copy a NUL-terminated UTF-8 string out of C. Returns null on invalid input.
unsafe fn borrow_str(ptr: *const c_char) -> Option<&'static str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

fn to_c_string(value: &str) -> *mut c_char {
    match CString::new(value.replace('\0', " ")) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Release a string returned by any `uv_*_json` function.
///
/// # Safety
/// `s` must be a pointer previously returned by this library and not yet
/// freed; passing null is a no-op.
#[no_mangle]
pub unsafe extern "C" fn uv_string_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: guaranteed by the contract above — we own this allocation.
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Open a vault with the master password.
///
/// Returns null on failure (wrong password, missing file); call `uv_last_error`
/// for the reason. This performs the Argon2id key derivation, so it is the
/// expensive call — a mobile app should do it once per unlock.
///
/// # Safety
/// Both pointers must be valid NUL-terminated strings (or null, which is
/// reported as an error rather than dereferenced).
#[no_mangle]
pub unsafe extern "C" fn uv_open(
    vault_path: *const c_char,
    password: *const c_char,
) -> *mut VaultHandle {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (Some(path), Some(pw)) = (unsafe { borrow_str(vault_path) }, unsafe {
            borrow_str(password)
        }) else {
            set_error("path or password was null or not valid UTF-8");
            return std::ptr::null_mut();
        };
        match Vault::open(std::path::Path::new(path), pw) {
            Ok(vault) => Box::into_raw(Box::new(VaultHandle { vault })),
            Err(e) => {
                set_error(&format!("{e:#}"));
                std::ptr::null_mut()
            }
        }
    }));
    result.unwrap_or_else(|_| {
        set_error("internal error while opening the vault");
        std::ptr::null_mut()
    })
}

/// Create a new vault file. Returns 0 on success, non-zero on failure
/// (e.g. the file already exists).
///
/// # Safety
/// Both pointers must be valid NUL-terminated strings (or null → error).
#[no_mangle]
pub unsafe extern "C" fn uv_create(vault_path: *const c_char, password: *const c_char) -> c_int {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (Some(path), Some(pw)) = (unsafe { borrow_str(vault_path) }, unsafe {
            borrow_str(password)
        }) else {
            set_error("path or password was null or not valid UTF-8");
            return -1;
        };
        if pw.is_empty() {
            set_error("password must not be empty");
            return -1;
        }
        match Vault::create(std::path::Path::new(path), pw) {
            Ok(_) => 0,
            Err(e) => {
                set_error(&format!("{e:#}"));
                -1
            }
        }
    }));
    result.unwrap_or_else(|_| {
        set_error("internal error while creating the vault");
        -1
    })
}

/// Destroy a handle returned by `uv_open`. Safe to call with null.
///
/// # Safety
/// `handle` must come from `uv_open` and must not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn uv_close(handle: *mut VaultHandle) {
    if handle.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        drop(unsafe { Box::from_raw(handle) });
    }));
}

fn with_handle<T>(handle: *mut VaultHandle, f: impl FnOnce(&Vault) -> T) -> Option<T> {
    if handle.is_null() {
        set_error("handle is null (already closed?)");
        return None;
    }
    // SAFETY: the caller of this function checked for null above, and handles
    // are owned by the caller until uv_close.
    let vault_ref: &Vault = unsafe { &(*handle) }.vault_ref();
    Some(f(vault_ref))
}

impl VaultHandle {
    fn vault_ref(&self) -> &Vault {
        &self.vault
    }
}

macro_rules! json_call {
    ($name:ident, $body:expr) => {
        #[no_mangle]
        pub extern "C" fn $name(handle: *mut VaultHandle) -> *mut c_char {
            clear_error();
            let out = catch_unwind(AssertUnwindSafe(|| {
                with_handle(handle, $body).map(|value| {
                    serde_json::to_string(&value).unwrap_or_else(|e| {
                        set_error(&format!("encode failed: {e}"));
                        String::from("{\"error\":\"encode failed\"}")
                    })
                })
            }));
            match out.unwrap_or(None) {
                Some(text) => to_c_string(&text),
                None => std::ptr::null_mut(),
            }
        }
    };
}

json_call!(uv_status_json, |v: &Vault| {
    serde_json::json!({
        "entries": v.data.entries.len(),
        "path": v.path.to_string_lossy(),
    })
});

// All entries WITHOUT passwords — safe to render as a list.
json_call!(uv_list_json, |v: &Vault| {
    let mut rows: Vec<_> = v
        .data
        .entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "title": e.title,
                "username": e.username,
                "url": e.url,
                "favorite": e.favorite,
                "tags": e.tags,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        a.get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase()
            .cmp(
                &b.get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase(),
            )
    });
    rows
});

/// One full entry, password included. `entry_id` is the exact entry id.
///
/// # Safety
/// `handle` must be a live handle and `entry_id` a valid NUL-terminated
/// string (null is reported as an error rather than dereferenced).
#[no_mangle]
pub unsafe extern "C" fn uv_get_json(
    handle: *mut VaultHandle,
    entry_id: *const c_char,
) -> *mut c_char {
    clear_error();
    let out = catch_unwind(AssertUnwindSafe(|| {
        let Some(id) = (unsafe { borrow_str(entry_id) }) else {
            set_error("entry id was null or not valid UTF-8");
            return None;
        };
        with_handle(handle, |v: &Vault| {
            v.data.entries.iter().find(|e| e.id == id).map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "title": e.title,
                    "username": e.username,
                    "password": e.password,
                    "url": e.url,
                    "notes": e.notes,
                })
            })
        })
        .flatten()
    }));
    match out.unwrap_or(None) {
        Some(value) => match serde_json::to_string(&value) {
            Ok(text) => to_c_string(&text),
            Err(e) => {
                set_error(&format!("encode failed: {e}"));
                std::ptr::null_mut()
            }
        },
        None => {
            set_error("no entry with that id");
            std::ptr::null_mut()
        }
    }
}

// Security posture: weak / reused / old counts.
json_call!(uv_audit_json, |v: &Vault| {
    let rep = crate::audit::audit(&v.data.entries);
    serde_json::json!({
        "total": rep.total,
        "weak": rep.weak,
        "reused": rep.reused,
        "old": rep.old,
    })
});

/// Generate a password without needing the vault.
///
/// `length` is clamped to 1..=256. A phone caller asking for a very short
/// password must not get a null back, so when the requested length cannot hold
/// one character per enabled class, classes are dropped (symbols first, then
/// digits, then the letter cases) until the generator succeeds. The returned
/// string's length equals `clamp(length, 1, 256)`.
#[no_mangle]
pub extern "C" fn uv_generate(length: c_int) -> *mut c_char {
    clear_error();
    let want = (length as usize).clamp(1, 256);
    let out = catch_unwind(|| {
        let mut opts = GenOptions {
            length: want,
            ..Default::default()
        };
        // Reduce the alphabet, never the length, so the caller's order holds.
        for (drop_symbols, drop_digits, drop_lower, drop_upper) in [
            (true, false, false, false),
            (true, true, false, false),
            (true, true, true, false),
            (false, false, false, false),
        ] {
            opts.symbols = !drop_symbols;
            opts.digits = !drop_digits;
            opts.lowercase = !drop_lower;
            opts.uppercase = !drop_upper;
            if let Ok(pw) = crate::generator::generate(&opts) {
                return Ok(pw);
            }
        }
        Err("cannot generate a password at this length".to_string())
    });
    match out.unwrap_or_else(|_| Err("internal error".to_string())) {
        Ok(pw) => to_c_string(&pw),
        Err(e) => {
            set_error(&e);
            std::ptr::null_mut()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::NewEntry;
    use tempfile::tempdir;

    fn cstr(s: &str) -> CString {
        CString::new(s).expect("cstring")
    }

    fn take(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null(), "expected a string, got null");
        // SAFETY: the pointer came from one of the uv_* functions above and is
        // released exactly once here.
        let out = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { uv_string_free(ptr) };
        out
    }

    #[test]
    fn open_list_get_close_roundtrip() {
        let dir = tempdir().expect("tmpdir");
        let path = dir.path().join("v.vault");
        let pw = cstr("ffi-master-pw");
        let cpath = cstr(path.to_string_lossy().as_ref());

        let mut vault = Vault::create(&path, "ffi-master-pw").expect("create");
        vault.add(NewEntry {
            title: "site".into(),
            username: "bob".into(),
            password: "S3cret!v4lue".into(),
            ..Default::default()
        });
        vault.save("ffi-master-pw").expect("save");

        let h = unsafe { uv_open(cpath.as_ptr(), pw.as_ptr()) };
        assert!(!h.is_null(), "open failed");

        let status = take(uv_status_json(h));
        assert!(status.contains("\"entries\":1"), "{status}");

        let list = take(uv_list_json(h));
        assert!(list.contains("site"));
        assert!(!list.contains("S3cret"), "list must not leak passwords");

        let id = CString::new(vault.data.entries[0].id.clone()).expect("id");
        let one = take(unsafe { uv_get_json(h, id.as_ptr()) });
        assert!(one.contains("S3cret!v4lue"));

        unsafe { uv_close(h) };
        unsafe { uv_close(std::ptr::null_mut()) }; // must not crash
    }

    #[test]
    fn wrong_password_returns_null_with_message() {
        let dir = tempdir().expect("tmpdir");
        let path = dir.path().join("v.vault");
        Vault::create(&path, "right").expect("create");
        let cpath = cstr(path.to_string_lossy().as_ref());
        let wrong = cstr("wrong");

        let h = unsafe { uv_open(cpath.as_ptr(), wrong.as_ptr()) };
        assert!(h.is_null());
        // SAFETY: uv_last_error always returns a valid C string.
        let msg = unsafe { CStr::from_ptr(uv_last_error()) }
            .to_string_lossy()
            .into_owned();
        assert!(!msg.is_empty(), "an error must be reported");
    }

    #[test]
    fn null_inputs_are_rejected_not_crashed() {
        assert!(unsafe { uv_open(std::ptr::null(), std::ptr::null()) }.is_null());
        assert!(uv_list_json(std::ptr::null_mut()).is_null());
        assert!(unsafe { uv_get_json(std::ptr::null_mut(), std::ptr::null()) }.is_null());
        assert!(!uv_generate(12).is_null());
    }

    #[test]
    fn generate_clamps_length() {
        for (input, expect) in [(0usize, 1usize), (24, 24), (9999, 256)] {
            let s = take(uv_generate(input as c_int));
            assert_eq!(s.chars().count(), expect, "input {input}");
        }
    }

    #[test]
    fn generate_never_fails_for_short_requests() {
        // A phone UI may ask for a short password; it must still get one.
        for len in 1..=6 {
            let s = take(uv_generate(len as c_int));
            assert_eq!(s.chars().count(), len as usize, "len {len}");
        }
    }

    #[test]
    fn create_reports_failure_without_panicking() {
        let dir = tempdir().expect("tmpdir");
        let path = dir.path().join("v.vault");
        let cpath = cstr(path.to_string_lossy().as_ref());
        let pw = cstr("pw");
        assert_eq!(unsafe { uv_create(cpath.as_ptr(), pw.as_ptr()) }, 0);
        // Second create must fail cleanly.
        assert_ne!(unsafe { uv_create(cpath.as_ptr(), pw.as_ptr()) }, 0);

        let empty = cstr("");
        let fresh = dir.path().join("fresh.vault");
        let cfresh = cstr(fresh.to_string_lossy().as_ref());
        assert_ne!(unsafe { uv_create(cfresh.as_ptr(), empty.as_ptr()) }, 0);
    }
}
