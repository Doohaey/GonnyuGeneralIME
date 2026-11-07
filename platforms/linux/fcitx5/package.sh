#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
version="$(awk -F '"' '/^version[[:space:]]*=/ { print $2; exit }' "$repo_root/Cargo.toml")"
artifact="${1:-$repo_root/build/GonnyuGeneralIME-${version}-fcitx5.tar.gz}"
stage_root="$(mktemp -d)"
trap 'rm -rf "$stage_root"' EXIT

bash "$script_dir/build.sh"
for addon in gonnyu gonnyu-ipa; do
  install -Dm755 "$repo_root/build/linux-fcitx5/libfcitx5-$addon.so" "$stage_root/payload/libfcitx5-$addon.so"
  install -Dm644 "$repo_root/build/linux-fcitx5/$addon.conf" "$stage_root/payload/$addon.conf"
  install -Dm644 "$repo_root/build/linux-fcitx5/$addon-addon.conf" "$stage_root/payload/$addon-addon.conf"
done
install -Dm755 "$script_dir/install.sh" "$stage_root/install.sh"
mkdir -p "$(dirname "$artifact")"
tar -C "$stage_root" -czf "$artifact" .
tar -tzf "$artifact" >/dev/null
