#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
output_dir="$repo_root/build/rime"

python3 "$script_dir/build.py" --region all --output "$output_dir"
bash "$script_dir/install.sh" "$output_dir"
