#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
source_root="${GANNYU_MACOS_RIME_RESOURCE_SOURCE:-$repo_root/build/rime-mobile/mobile-resources}"
output_root="${GANNYU_MACOS_RIME_RESOURCE_OUTPUT:-$repo_root/build/rime-macos/resources}"

if [[ "${GANNYU_MACOS_SKIP_RESOURCE_REBUILD:-0}" != "1" ]]; then
  bash "$repo_root/platforms/rime/mobile/prepare_resources.sh"
fi

[[ -f "$source_root/resource-manifest.json" ]] || { echo "missing Rime resource manifest: $source_root" >&2; exit 2; }
[[ -d "$source_root/shared" ]] || { echo "missing Rime shared data: $source_root/shared" >&2; exit 2; }
[[ -d "$source_root/prebuilt" ]] || { echo "missing Rime prebuilt data: $source_root/prebuilt" >&2; exit 2; }

rm -rf "$output_root"
mkdir -p "$(dirname "$output_root")"
ditto "$source_root" "$output_root"

python3 -c 'import json, sys; data = json.load(open(sys.argv[1])); assert data.get("regions") and data.get("files")' "$output_root/resource-manifest.json"
echo "prepared macOS Rime resources: $output_root"
