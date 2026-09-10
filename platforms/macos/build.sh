#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
test_env="$script_dir/test_local.env"
bundle_id="${GANNYU_IMK_BUNDLE_ID:-org.doohaey.inputmethod.gonnyu.native}"
connection_name="${GANNYU_IMK_CONNECTION:-${bundle_id}_Connection}"
bundle_root="${GANNYU_MACOS_APP_BUNDLE:-$repo_root/build/macos/GonnyuInputMethod.app}"
plist_template="$script_dir/Info.plist.template"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "this builder must run on macOS" >&2
  exit 1
fi

if [[ -f "$test_env" ]]; then
  set -a
  source "$test_env"
  set +a
fi

if [[ -z "${GANNYU_RESOURCE_KEY:-}" ]]; then
  key_file="${GANNYU_RESOURCE_KEY_FILE:-$HOME/.config/gonnyu/resource-key}"
  if [[ -r "$key_file" ]]; then
    IFS= read -r GANNYU_RESOURCE_KEY < "$key_file"
    export GANNYU_RESOURCE_KEY
  fi
fi

command -v cargo >/dev/null || { echo "cargo not found; install rustup first" >&2; exit 1; }
command -v swift >/dev/null || { echo "swift not found; install Xcode Command Line Tools first" >&2; exit 1; }
command -v python3 >/dev/null || { echo "python3 not found" >&2; exit 1; }

cd "$repo_root"
cargo build -p gannyu-input-ffi --release
swift build --package-path "$script_dir" -c release

bin_dir="$(swift build --package-path "$script_dir" -c release --show-bin-path)"
version="$(python3 - <<'PYTHON'
import re
from pathlib import Path

content = Path("Cargo.toml").read_text(encoding="utf-8")
match = re.search(r'workspace\.package\]\s*version\s*=\s*"([^"]+)"', content, re.S)
if not match:
    raise SystemExit("missing workspace package version")
print(match.group(1))
PYTHON
)"

rm -rf "$bundle_root"
mkdir -p "$bundle_root/Contents/MacOS" "$bundle_root/Contents/Resources"
install -m 0755 "$bin_dir/GannyuInputMethodHost" "$bundle_root/Contents/MacOS/GannyuInputMethodHost"
python3 - "$plist_template" "$bundle_root/Contents/Info.plist" "$version" "$bundle_id" "$connection_name" <<'PYTHON'
from pathlib import Path
import sys

template = Path(sys.argv[1]).read_text(encoding="utf-8")
template = template.replace("@VERSION@", sys.argv[3])
template = template.replace("@BUNDLE_ID@", sys.argv[4])
template = template.replace("@CONNECTION_NAME@", sys.argv[5])
Path(sys.argv[2]).write_text(template, encoding="utf-8")
PYTHON

# Prefer an Apple Development identity for local TIS registration. Release
# packaging can override this with its Developer ID identity.
signing_identity="${GANNYU_MACOS_SIGN_IDENTITY:-}"
if [[ -z "$signing_identity" ]]; then
  signing_identity="$(security find-identity -v -p codesigning 2>/dev/null | awk '/"Apple Development:/{ print $2; exit }')"
fi
codesign --force --deep --sign "${signing_identity:--}" "$bundle_root"
codesign --verify --deep --strict "$bundle_root"

echo "packaged $bundle_root"
