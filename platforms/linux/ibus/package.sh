#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
version="$(awk -F '"' '/^version[[:space:]]*=/ { print $2; exit }' "$repo_root/Cargo.toml")"
artifact="${1:-$repo_root/build/GonnyuGeneralIME-${version}-ibus.tar.gz}"
stage_root="$(mktemp -d)"
trap 'rm -rf "$stage_root"' EXIT

bash "$script_dir/build.sh"
install -Dm755 "$repo_root/build/linux-ibus/ibus-engine-gannyu" "$stage_root/payload/ibus-engine-gannyu"
install -Dm644 "$repo_root/build/linux-ibus/gannyu.xml" "$stage_root/payload/gannyu.xml"
install -Dm755 "$script_dir/install.sh" "$stage_root/install.sh"
mkdir -p "$(dirname "$artifact")"
tar -C "$stage_root" -czf "$artifact" .
tar -tzf "$artifact" >/dev/null
