#!/usr/bin/env bash
set -euo pipefail

platform="${1:?missing platform}"
shift

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="$(awk -F '"' '/^version[[:space:]]*=/ { print $2; exit }' "$repo_root/Cargo.toml")"
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

regions=()
regions+=("$@")
if [ "${#regions[@]}" -eq 0 ]; then
  while IFS= read -r region; do
    [ -n "$region" ] && regions+=("$region")
  done < <("$python_bin" "$repo_root/platforms/rime/build.py" --list-regions)
fi

for region in "${regions[@]}"; do
  output_dir="$repo_root/build/rime/$region"
  if [[ "$platform" == universal ]]; then
    archive="$repo_root/build/GonnyuGeneralIME-${version}-rime-${region}.zip"
  else
    archive="$repo_root/build/GonnyuGeneralIME-${version}-rime-${platform}-${region}.zip"
  fi
  "$python_bin" "$repo_root/platforms/rime/build.py" --region "$region" --display-name apple --output "$output_dir"
  (
    cd "$output_dir"
    "$python_bin" - "$archive" "$region" <<'PYTHON'
from pathlib import Path
import sys
from zipfile import ZIP_DEFLATED, ZipFile

archive = Path(sys.argv[1])
region = sys.argv[2]
files = (
    "default.custom.yaml", f"gannyu_{region}.schema.yaml", f"gannyu_{region}.dict.yaml",
    "resource-manifest.json", "lua/gannyu_annotation_filter.lua", "lua/gannyu_single_char_filter.lua", "lua/gannyu_relation_filter.lua",
    f"lua/gannyu_{region}_data.lua",
)
with ZipFile(archive, "w", ZIP_DEFLATED) as package:
    for name in files:
        package.write(name, name)
with ZipFile(archive) as package:
    assert tuple(package.namelist()) == files
PYTHON
  )
done
