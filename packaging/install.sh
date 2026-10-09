#!/bin/bash
# unbundio-vault installer: no sudo, no server, no account.
# Usage:
#   ./install.sh [--prefix DIR] [--extension-id ID] [--browser chrome|firefox]
#                [--vault PATH] [--bin PATH]
#
# Typical first run (after loading the extension in Chrome once to get its ID):
#   ./install.sh --extension-id <ID-from-chrome://extensions>
set -euo pipefail

PREFIX="${HOME}/.local/bin"
EXTENSION_ID=""
BROWSER="chrome"
VAULT_PATH="${UNBUNDIO_VAULT:-${HOME}/unbundio-vault.vault}"
BIN=""

while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) PREFIX="$2"; shift 2 ;;
    --extension-id) EXTENSION_ID="$2"; shift 2 ;;
    --browser) BROWSER="$2"; shift 2 ;;
    --vault) VAULT_PATH="$2"; shift 2 ;;
    --bin) BIN="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,12p' "$0"
      exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 1 ;;
  esac
done

# 1. Locate the binary (same dir as this script, unless overridden).
if [ -z "$BIN" ]; then
  BIN="$(cd "$(dirname "$0")" && pwd)/unbundio-vault"
fi
if [ ! -x "$BIN" ]; then
  echo "error: binary not found at $BIN" >&2
  exit 1
fi

# 2. Install binary.
mkdir -p "$PREFIX"
cp -f "$BIN" "$PREFIX/unbundio-vault"
chmod +x "$PREFIX/unbundio-vault"
echo "installed binary to $PREFIX/unbundio-vault"
case ":$PATH:" in
  *":$PREFIX:"*) ;;
  *) echo "NOTE: add to PATH once: export PATH=\"$PREFIX:\$PATH\"" ;;
esac

EXE="$PREFIX/unbundio-vault"

# 3. Create vault (interactive; skipped when not a TTY or already exists).
if [ ! -f "$VAULT_PATH" ]; then
  if [ -t 0 ]; then
    echo "creating vault at $VAULT_PATH"
    "$EXE" --vault "$VAULT_PATH" init
  else
    echo "NOTE: create your vault later: $EXE --vault \"$VAULT_PATH\" init"
  fi
else
  echo "vault already exists at $VAULT_PATH"
fi

# 4. Browser host manifest (needs the extension ID).
if [ -z "$EXTENSION_ID" ]; then
  echo ""
  echo "NEXT (one time):"
  echo "  1. open chrome://extensions, enable Developer mode"
  echo "  2. 'Load unpacked' -> select the 'extension' folder from this package"
  echo "  3. copy its ID, then re-run:"
  echo "       ./install.sh --extension-id <ID>"
  exit 0
fi

"$EXE" --vault "$VAULT_PATH" install-extension \
  --browser "$BROWSER" --extension-id "$EXTENSION_ID" --binary "$EXE"

echo ""
echo "done. click the toolbar icon on any login page -> Fill."
echo "free forever: no account, no cloud, uninstall = delete the binary + $VAULT_PATH"
