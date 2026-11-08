#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
version="$(awk -F '"' '/^version[[:space:]]*=/ { print $2; exit }' "$repo_root/Cargo.toml")"
artifact_root="${1:-$repo_root/build}"

bash "$script_dir/build.sh"

stage_addon() {
  local addon="$1"
  local artifact="$2"
  local stage_root
  stage_root="$(mktemp -d)"
  trap 'rm -rf "$stage_root"' RETURN
  install -Dm755 "$repo_root/build/linux-fcitx5/libfcitx5-$addon.so" "$stage_root/payload/libfcitx5-$addon.so"
  install -Dm644 "$repo_root/build/linux-fcitx5/$addon.conf" "$stage_root/payload/$addon.conf"
  install -Dm644 "$repo_root/build/linux-fcitx5/$addon-addon.conf" "$stage_root/payload/$addon-addon.conf"
  install -Dm755 "$script_dir/install.sh" "$stage_root/install.sh"
  mkdir -p "$(dirname "$artifact")"
  tar -C "$stage_root" -czf "$artifact" .
  tar -tzf "$artifact" >/dev/null
  rm -rf "$stage_root"
  trap - RETURN
}

if [ "$#" -gt 0 ]; then
  artifact_root="$1"
fi
mkdir -p "$artifact_root"
stage_addon gonnyu "$artifact_root/GonnyuGeneralIME-${version}-fcitx5-gonnyu.tar.gz"
stage_addon gonnyu_II "$artifact_root/GonnyuGeneralIME-${version}-fcitx5-gonnyu_II.tar.gz"
