/* unbundio-vault — C ABI for the native shells.
 * Hand-written to match src/ffi.rs; keep the two in sync.
 * Rust owns every returned string; release it with uv_string_free(). */
#ifndef UNBUNDIO_VAULT_H
#define UNBUNDIO_VAULT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque handle owning a decrypted vault in memory. */
typedef struct VaultHandle VaultHandle;

/* Last error on this thread. Valid until the next call. Never NULL. */
const char *uv_last_error(void);

/* Open (and decrypt) a vault. Returns NULL on failure; see uv_last_error().
 * Performs the Argon2id KDF — call once per unlock, not per query. */
void *uv_open(const char *vault_path, const char *password);

/* Create a vault file. 0 = success, non-zero = failure. */
int uv_create(const char *vault_path, const char *password);

/* Destroy a handle from uv_open. NULL is a no-op. */
void uv_close(void *handle);

/* JSON strings, owned by Rust. Release with uv_string_free(). */
char *uv_status_json(void *handle);   /* {"entries":N,"path":"…"} */
char *uv_list_json(void *handle);     /* entries WITHOUT passwords */
char *uv_get_json(void *handle, const char *entry_id); /* one entry, with password */
char *uv_audit_json(void *handle);    /* {"total":N,"weak":[…],"reused":[…],"old":[…]} */

/* Generate a password; length is clamped to 1..=256 and never fails. */
char *uv_generate(int length);

/* Release any string returned above. NULL is a no-op. */
void uv_string_free(char *s);

#ifdef __cplusplus
}
#endif

#endif /* UNBUNDIO_VAULT_H */