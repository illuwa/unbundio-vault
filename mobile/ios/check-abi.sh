#!/usr/bin/env bash
# Verify the C header and the Rust exports have not drifted apart.
#
#   src/ffi.rs            defines #[no_mangle] extern "C" functions
#   mobile/ios/UnbundioVault.h  declares them for Swift
#
# A mismatch only shows up at link time inside Xcode, which is a slow and
# confusing way to find out. This catches it in a second.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

LIB="$ROOT/target/aarch64-apple-ios-sim/release/libunbundio_vault.a"
HEADER="$ROOT/mobile/ios/Sources/UnbundioVault.h"

[ -f "$LIB" ] || { echo "missing $LIB — run ./mobile/ios/build.sh sim" >&2; exit 1; }

# Rust side: every #[no_mangle] extern "C" fn uv_*, including the ones the
# json_call! macro emits.
rust_syms() {
  {
    grep -oE 'pub (unsafe )?extern "C" fn uv_[a-z_]+' src/ffi.rs | sed -E 's/.*fn //'
    grep -oE 'json_call!\(uv_[a-z_]+' src/ffi.rs | sed -E 's/.*\((.*)/\1/'
  } | sort -u
}
# Header side: every declared prototype
header_syms() {
  grep -oE '\buv_[a-z_]+\(' "$HEADER" | tr -d '(' | sort -u
}

rust_list="$(rust_syms)"
header_list="$(header_syms)"
lib_list="$(nm -gU "$LIB" 2>/dev/null | sed -nE 's/.* _(uv_[a-z_]+)$/\1/p' | sort -u)"

status=0

missing_in_header="$(comm -23 <(echo "$rust_list") <(echo "$header_list") || true)"
if [ -n "$missing_in_header" ]; then
  echo "FAIL  declared in Rust but not in the header:"
  echo "$missing_in_header" | sed 's/^/        /'
  status=1
else
  echo "PASS  every Rust export is declared in UnbundioVault.h"
fi

missing_in_lib="$(comm -23 <(echo "$rust_list") <(echo "$lib_list") || true)"
if [ -n "$missing_in_lib" ]; then
  echo "FAIL  declared in Rust but not exported by the static library:"
  echo "$missing_in_lib" | sed 's/^/        /'
  status=1
else
  echo "PASS  every Rust export exists in libunbundio_vault.a"
fi

stale_in_header="$(comm -13 <(echo "$rust_list") <(echo "$header_list") || true)"
if [ -n "$stale_in_header" ]; then
  echo "FAIL  declared in the header but not in Rust (stale?):"
  echo "$stale_in_header" | sed 's/^/        /'
  status=1
else
  echo "PASS  no stale declarations in UnbundioVault.h"
fi

exit "$status"