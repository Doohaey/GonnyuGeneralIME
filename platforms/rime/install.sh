#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_dir="${1:-$repo_root/build/rime}"
user_dir="${XDG_DATA_HOME:-$HOME/.local/share}/fcitx5/rime"
shared_dir="/usr/share/rime-data"

if ! compgen -G "$output_dir/gannyu_*.schema.yaml" >/dev/null; then
  echo "未找到 Rime schema: $output_dir" >&2
  echo "请先运行: python3 platforms/rime/build.py" >&2
  exit 1
fi

command -v rime_deployer >/dev/null || { echo "missing rime_deployer" >&2; exit 1; }
test -f /usr/lib/x86_64-linux-gnu/rime-plugins/librime-lua.so ||
  echo "warning: librime-lua was not found in the standard plugin directory" >&2

mkdir -p "$user_dir/lua"
for schema in "$output_dir"/gannyu_*.schema.yaml; do install -m 0644 "$schema" "$user_dir/"; done
install -m 0644 "$output_dir"/gannyu_*.dict.yaml "$user_dir/"
install -m 0644 "$output_dir/lua/gannyu_filter.lua" "$user_dir/lua/gannyu_filter.lua"
for data in "$output_dir"/lua/gannyu_*_data.lua; do install -m 0644 "$data" "$user_dir/lua/"; done
rm -f "$user_dir/gannyu.schema.yaml" "$user_dir/lua/gannyu_data.lua" "$user_dir/build/gannyu.schema.yaml"

if [[ ! -e "$user_dir/default.custom.yaml" ]] || grep -q "schema: gannyu$" "$user_dir/default.custom.yaml"; then
  install -m 0644 "$output_dir/default.custom.yaml" "$user_dir/default.custom.yaml"
else
  while IFS= read -r region; do
    grep -q "schema: gannyu_${region}" "$user_dir/default.custom.yaml" || echo "warning: add gannyu_${region} to $user_dir/default.custom.yaml" >&2
  done < <(python3 "$repo_root/platforms/rime/build.py" --list-regions)
fi

rime_deployer --build "$user_dir" "$shared_dir" "$user_dir/build"
fcitx5-remote -r >/dev/null 2>&1 || true

echo "installed Gannyu Rime schema into $user_dir"
echo "select a Gonnyu region from the Rime schema menu"
