#!/usr/bin/env bash
set -euo pipefail

platform="${1:?missing platform}"
shift

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
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
  "$python_bin" "$repo_root/platforms/rime/build.py" --region "$region" --display-name apple --output "$output_dir"
  "$python_bin" "$repo_root/platforms/rime/package_region.py" \
    "$output_dir" "$repo_root/build" "$region" "$platform"
done
