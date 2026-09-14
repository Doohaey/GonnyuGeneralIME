#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
output_root="${GANNYU_IOS_BUILD_ROOT:-$repo_root/build/ios}"
signing_config="$script_dir/Config/Signing.xcconfig"

[[ "$(uname -s)" == "Darwin" ]] || { echo "this builder must run on macOS" >&2; exit 1; }
[[ -f "$signing_config" ]] || { echo "copy Config/Signing.xcconfig.example to Config/Signing.xcconfig and set your Apple team and App Group" >&2; exit 1; }
grep -q '^DEVELOPMENT_TEAM = [^Y]' "$signing_config" || { echo "Signing.xcconfig must set DEVELOPMENT_TEAM" >&2; exit 1; }
grep -q '^GANNYU_APP_GROUP = group\.' "$signing_config" || { echo "Signing.xcconfig must set a valid GANNYU_APP_GROUP" >&2; exit 1; }
grep -q '^GANNYU_APP_BUNDLE_IDENTIFIER = ' "$signing_config" || { echo "Signing.xcconfig must set GANNYU_APP_BUNDLE_IDENTIFIER" >&2; exit 1; }
grep -q '^GANNYU_KEYBOARD_BUNDLE_IDENTIFIER = ' "$signing_config" || { echo "Signing.xcconfig must set GANNYU_KEYBOARD_BUNDLE_IDENTIFIER" >&2; exit 1; }
command -v xcodebuild >/dev/null || { echo "Xcode not found" >&2; exit 1; }

cd "$repo_root"

python_bin="${PYTHON_BIN:-python3}"
if ! "$python_bin" -c 'import tomllib' >/dev/null 2>&1; then
  for candidate in /opt/homebrew/bin/python3 python3.14 python3.13 python3.12 python3.11; do
    if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'import tomllib' >/dev/null 2>&1; then
      python_bin="$candidate"
      break
    fi
  done
fi
"$python_bin" -c 'import tomllib' >/dev/null 2>&1 || {
  echo "Python 3.11+ with tomllib is required; set PYTHON_BIN" >&2
  exit 1
}

host_deployer="$repo_root/build/rime-mobile/host/build/bin/rime_deployer"
if [[ "${GANNYU_IOS_REUSE_HOST_TOOLS:-0}" != "1" || ! -x "$host_deployer" ]]; then
  PYTHON_BIN="$python_bin" bash "$repo_root/platforms/rime/mobile/build_host.sh"
fi
"$python_bin" "$repo_root/platforms/rime/mobile/build_resources.py" \
  --output "$output_root/rime" \
  --deployer "$host_deployer"
GANNYU_IOS_BUILD_ROOT="$output_root" PYTHON_BIN="$python_bin" \
  bash "$repo_root/platforms/rime/mobile/build_ios_xcframework.sh"

xcodebuild \
  -project "$script_dir/GonnyuInput.xcodeproj" \
  -scheme GonnyuInput \
  -configuration Release \
  -sdk iphoneos \
  -destination 'generic/platform=iOS' \
  -xcconfig "$signing_config" \
  -archivePath "$output_root/GonnyuInput.xcarchive" \
  archive
