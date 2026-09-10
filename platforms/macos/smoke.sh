#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
test_env="$script_dir/test_local.env"
skip_build=0

if [[ "${1:-}" == "--skip-build" ]]; then
  skip_build=1
fi

if [[ -f "$test_env" ]]; then
  set -a
  source "$test_env"
  set +a
fi

export GANNYU_MANIFEST="${GANNYU_MANIFEST:-$repo_root/resources/manifest.toml}"
export GANNYU_REGION_ID="${GANNYU_REGION_ID:-}"
export GANNYU_MACOS_SMOKE_RETRIEVE="${GANNYU_MACOS_SMOKE_RETRIEVE:-gau}"
export GANNYU_MACOS_SMOKE_COMPOSE="${GANNYU_MACOS_SMOKE_COMPOSE:-吹牛}"

if [[ "$skip_build" -eq 0 ]]; then
  bash "$script_dir/build.sh"
fi

args=(
  --manifest "$GANNYU_MANIFEST"
  --retrieve "$GANNYU_MACOS_SMOKE_RETRIEVE"
  --compose "$GANNYU_MACOS_SMOKE_COMPOSE"
)

if [[ -n "$GANNYU_REGION_ID" ]]; then
  args+=(--region "$GANNYU_REGION_ID")
fi

swift run --package-path "$script_dir" -c release GannyuMacOSSmoke "${args[@]}"
