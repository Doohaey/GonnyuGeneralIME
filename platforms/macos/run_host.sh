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
"${GANNYU_MACOS_APP_BUNDLE:-$repo_root/build/macos/GonnyuInputMethod.app}/Contents/MacOS/GannyuInputMethodHost"
