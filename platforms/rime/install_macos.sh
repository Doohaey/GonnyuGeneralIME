#!/usr/bin/env bash
set -euo pipefail

[[ "$(uname -s)" == "Darwin" ]] || { echo "this installer must run on macOS" >&2; exit 1; }

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_dir="$repo_root/build/rime"
user_dir="${GANNYU_RIME_USER_DIR:-$HOME/Library/Rime}"

python3 "$repo_root/platforms/rime/build.py" --region all --display-name apple --output "$output_dir"
mkdir -p "$user_dir/lua"
for schema in "$output_dir"/gannyu_*.schema.yaml; do install -m 0644 "$schema" "$user_dir/"; done
install -m 0644 "$output_dir"/gannyu_*.dict.yaml "$user_dir/"
install -m 0644 "$output_dir/lua/gannyu_filter.lua" "$user_dir/lua/gannyu_filter.lua"
for data in "$output_dir"/lua/gannyu_*_data.lua; do install -m 0644 "$data" "$user_dir/lua/"; done

if [[ ! -e "$user_dir/default.custom.yaml" ]]; then
  install -m 0644 "$output_dir/default.custom.yaml" "$user_dir/default.custom.yaml"
else
  while IFS= read -r region; do
    grep -q "schema: gannyu_${region}" "$user_dir/default.custom.yaml" || echo "warning: add gannyu_${region} to $user_dir/default.custom.yaml" >&2
  done < <(python3 "$repo_root/platforms/rime/build.py" --list-regions)
fi

echo "installed Gannyu Rime schemas into $user_dir"
echo "choose a Gonnyu region from the Rime schema menu"
