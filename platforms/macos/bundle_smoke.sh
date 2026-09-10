#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
test_env="$script_dir/test_local.env"

if [[ -f "$test_env" ]]; then
  set -a
  source "$test_env"
  set +a
fi

bash "$script_dir/build.sh"

bundle="${GANNYU_MACOS_APP_BUNDLE:-$repo_root/build/macos/GonnyuInputMethod.app}"
plist="$bundle/Contents/Info.plist"
binary="$bundle/Contents/MacOS/GannyuInputMethodHost"

plutil -lint "$plist" >/dev/null
test -x "$binary"
/usr/libexec/PlistBuddy -c "Print :InputMethodServerControllerClass" "$plist" | grep -qx "GannyuInputController"
/usr/libexec/PlistBuddy -c "Print :InputMethodConnectionName" "$plist" | grep -q "_Connection$"
/usr/libexec/PlistBuddy -c "Print :ComponentInputModeDict:tsInputModeListKey:org.doohaey.inputmethod.gonnyu.native.Gan:TISInputSourceID" "$plist" | grep -qx "org.doohaey.inputmethod.gonnyu.native.Gan"
codesign --verify --deep --strict "$bundle"

export GANNYU_MANIFEST="${GANNYU_MANIFEST:-$repo_root/resources/manifest.toml}"
export GANNYU_IMK_SELFTEST=1
"$binary"
