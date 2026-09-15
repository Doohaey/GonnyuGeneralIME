#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
python_bin="${PYTHON_BIN:-python3}"
output_root="${GANNYU_MOBILE_RESOURCE_ROOT:-$repo_root/build/rime-mobile/mobile-resources}"

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

PYTHON_BIN="$python_bin" bash "$script_dir/build_host.sh"
"$python_bin" "$script_dir/build_resources.py" \
  --output "$output_root" \
  --deployer "$repo_root/build/rime-mobile/host/build/bin/rime_deployer"

echo "prepared canonical mobile Rime resources: $output_root"
