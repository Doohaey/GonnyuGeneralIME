#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
test_env="$script_dir/test_local.env"
bundle_id="${GANNYU_IMK_BUNDLE_ID:-org.doohaey.inputmethod.gonnyu.native}"
connection_name="${GANNYU_IMK_CONNECTION:-${bundle_id}_Connection}"
bundle_root="${GANNYU_MACOS_APP_BUNDLE:-$repo_root/build/macos/GonnyuInputMethod.app}"
plist_template="$script_dir/Info.plist.template"
requested_signing_identity="${GANNYU_MACOS_SIGN_IDENTITY:-}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "this builder must run on macOS" >&2
  exit 1
fi

if [[ -f "$test_env" ]]; then
  set -a
  source "$test_env"
  set +a
fi
if [[ -n "$requested_signing_identity" ]]; then
  export GANNYU_MACOS_SIGN_IDENTITY="$requested_signing_identity"
fi

if [[ -z "${GANNYU_RESOURCE_KEY:-}" ]]; then
  key_file="${GANNYU_RESOURCE_KEY_FILE:-$HOME/.config/gonnyu/resource-key}"
  if [[ -r "$key_file" ]]; then
    IFS= read -r GANNYU_RESOURCE_KEY < "$key_file"
    export GANNYU_RESOURCE_KEY
  fi
fi

command -v swift >/dev/null || { echo "swift not found; install Xcode Command Line Tools first" >&2; exit 1; }
command -v python3 >/dev/null || { echo "python3 not found" >&2; exit 1; }

cd "$repo_root"
if [[ "${GANNYU_MACOS_REUSE_RIME_BUILD:-0}" != "1" ]]; then
  bash "$script_dir/build_rime_engine.sh"
  bash "$script_dir/prepare_rime_resources.sh"
else
  [[ -f "$repo_root/build/rime-macos/adapter/libgannyu_rime_engine.a" ]] || { echo "missing cached librime adapter" >&2; exit 2; }
  [[ -f "$repo_root/build/rime-macos/resources/resource-manifest.json" ]] || { echo "missing cached Rime resources" >&2; exit 2; }
fi
swift build --package-path "$script_dir" -c release --arch arm64 --arch x86_64

bin_dir="$(swift build --package-path "$script_dir" -c release --arch arm64 --arch x86_64 --show-bin-path)"
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
short_version="${version%%-pre.*}"
build_version="$(python3 "$script_dir/installer_version.py" "$version")"

rm -rf "$bundle_root"
mkdir -p "$bundle_root/Contents/MacOS" "$bundle_root/Contents/Resources"
install -m 0755 "$bin_dir/GannyuInputMethodHost" "$bundle_root/Contents/MacOS/GannyuInputMethodHost"
ditto "$repo_root/build/rime-macos/resources" "$bundle_root/Contents/Resources/rime"
icon_resource="$repo_root/resources/Gonnyu.icns"
[[ -f "$icon_resource" ]] || { echo "missing macOS icon resource: $icon_resource" >&2; exit 2; }
cp "$icon_resource" "$bundle_root/Contents/Resources/Gonnyu.icns"
for localization in en zh-Hans; do
  localization_source="$repo_root/platforms/macos/Resources/$localization.lproj"
  [[ -d "$localization_source" ]] || { echo "missing macOS localization resource: $localization_source" >&2; exit 2; }
  ditto "$localization_source" "$bundle_root/Contents/Resources/$localization.lproj"
done
python3 - "$plist_template" "$bundle_root/Contents/Info.plist" "$version" "$short_version" "$build_version" "$bundle_id" "$connection_name" <<'PYTHON'
from pathlib import Path
import sys

template = Path(sys.argv[1]).read_text(encoding="utf-8")
template = template.replace("@VERSION@", sys.argv[3])
template = template.replace("@SHORT_VERSION@", sys.argv[4])
template = template.replace("@BUILD_VERSION@", sys.argv[5])
template = template.replace("@BUNDLE_ID@", sys.argv[6])
template = template.replace("@CONNECTION_NAME@", sys.argv[7])
Path(sys.argv[2]).write_text(template, encoding="utf-8")
PYTHON

# Prefer an Apple Development identity for local TIS registration. Release
# packaging can override this with its Developer ID identity.
signing_identity="${GANNYU_MACOS_SIGN_IDENTITY:-}"
if [[ -z "$signing_identity" ]]; then
  signing_identity="$(security find-identity -v -p codesigning 2>/dev/null | awk '/"Apple Development:/{ print $2; exit }')"
fi
if [[ "${GANNYU_MACOS_CI_ADHOC:-0}" == "1" && -z "$signing_identity" ]]; then
  signing_identity="-"
fi
[[ -n "$signing_identity" ]] || {
  echo "no Apple signing identity found; set GANNYU_MACOS_CI_ADHOC=1 only for unsigned CI tests" >&2
  exit 1
}
if [[ "$signing_identity" == "Developer ID Application:"* ]]; then
  codesign --force --deep --options runtime --timestamp --sign "$signing_identity" "$bundle_root"
else
  codesign --force --deep --sign "${signing_identity:--}" "$bundle_root"
fi
codesign --verify --deep --strict "$bundle_root"
lipo "$bundle_root/Contents/MacOS/GannyuInputMethodHost" -verify_arch arm64 x86_64

echo "packaged $bundle_root"
