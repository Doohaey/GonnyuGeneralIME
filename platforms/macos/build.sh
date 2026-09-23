#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
test_env="$script_dir/test_local.env"
bundle_id="${GANNYU_IMK_BUNDLE_ID:-org.doohaey.inputmethod.gonnyu.native}"
connection_name="${GANNYU_IMK_CONNECTION:-${bundle_id}_Connection}"
bundle_root="${GANNYU_MACOS_APP_BUNDLE:-$repo_root/build/macos/GonnyuInputMethod.app}"
rime_build_root="${GANNYU_MACOS_RIME_BUILD_ROOT:-$repo_root/../dependencies/cache/macos/rime-engine}"
rime_resource_root="${GANNYU_MACOS_RIME_RESOURCE_OUTPUT:-$repo_root/build/rime-macos/resources}"
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
adapter_library="$rime_build_root/adapter/libgannyu_rime_engine.a"
librime_library="$rime_build_root/prefix/lib/librime.a"
resource_manifest="$rime_resource_root/resource-manifest.json"
engine_inputs=(
  "$repo_root/engines/rime/CMakeLists.txt"
  "$repo_root/engines/rime/gannyu_rime_engine.cpp"
  "$repo_root/platforms/rime/mobile/engine-lock.json"
)

engine_rebuild=0
if [[ "${GANNYU_MACOS_FORCE_RIME_REBUILD:-0}" == "1" || ! -f "$adapter_library" || ! -f "$librime_library" ]]; then
  engine_rebuild=1
else
  for input in "${engine_inputs[@]}"; do
    if [[ "$input" -nt "$adapter_library" ]]; then
      engine_rebuild=1
      break
    fi
  done
fi
if [[ "$engine_rebuild" == "1" ]]; then
  GANNYU_MACOS_RIME_BUILD_ROOT="$rime_build_root" bash "$script_dir/build_rime_engine.sh"
else
  echo "reusing cached macOS librime engine: $rime_build_root"
fi

resource_rebuild=0
if [[ "${GANNYU_MACOS_FORCE_RIME_REBUILD:-0}" == "1" || ! -f "$resource_manifest" || ! -d "$rime_resource_root/shared" || ! -d "$rime_resource_root/prebuilt" ]]; then
  resource_rebuild=1
elif find "$repo_root/resources" "$repo_root/platforms/rime/mobile" -type f -newer "$resource_manifest" -print -quit | grep -q .; then
  resource_rebuild=1
fi
if [[ "$resource_rebuild" == "1" ]]; then
  GANNYU_MACOS_RIME_RESOURCE_OUTPUT="$rime_resource_root" bash "$script_dir/prepare_rime_resources.sh"
else
  echo "reusing cached macOS Rime resources: $rime_resource_root"
fi
package_rime_root="$repo_root/build/rime-macos"
if [[ ! -e "$package_rime_root" ]]; then
  mkdir -p "$(dirname "$package_rime_root")"
  ln -s "$rime_build_root" "$package_rime_root"
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
ditto "$rime_resource_root" "$bundle_root/Contents/Resources/rime"
icon_resource="$repo_root/resources/icon.png"
[[ -f "$icon_resource" ]] || { echo "missing canonical icon resource: $icon_resource" >&2; exit 2; }
cp "$icon_resource" "$bundle_root/Contents/Resources/icon.png"
iconset_root="$(mktemp -d "${TMPDIR:-/private/tmp}/gonnyu-iconset.XXXXXX")"
iconset_dir="$iconset_root/Gonny.iconset"
mkdir "$iconset_dir"
trap 'rm -rf "$iconset_root"' EXIT
for icon_size in 16 32 128 256 512; do
  sips -z "$icon_size" "$icon_size" "$icon_resource" --out "$iconset_dir/icon_${icon_size}x${icon_size}.png" >/dev/null
  sips -z "$((icon_size * 2))" "$((icon_size * 2))" "$icon_resource" --out "$iconset_dir/icon_${icon_size}x${icon_size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset_dir" -o "$bundle_root/Contents/Resources/Gonny.icns"
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
