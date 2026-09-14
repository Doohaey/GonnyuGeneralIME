#!/usr/bin/env bash
# Gonnyu Android input method build.  Mobile builds use the native Rime engine.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
variant="${GANNYU_ANDROID_BUILD_VARIANT:-debug}"
apk_dir="$script_dir/app/build/outputs/apk/$variant"

case "$variant" in
  debug|release) ;;
  *)
    echo "GANNYU_ANDROID_BUILD_VARIANT must be debug or release." >&2
    exit 2
    ;;
esac

if [[ "$variant" == "release" ]]; then
  for signing_var in ANDROID_KEYSTORE_PASSWORD ANDROID_KEY_ALIAS ANDROID_KEY_PASSWORD; do
    if [[ -z "${!signing_var:-}" ]]; then
      echo "$signing_var is required for a signed Android release." >&2
      exit 2
    fi
  done
  if [[ -z "${ANDROID_KEYSTORE_BASE64:-}" && -z "${ANDROID_KEYSTORE_PATH:-}" ]]; then
    echo "ANDROID_KEYSTORE_BASE64 (CI) or ANDROID_KEYSTORE_PATH (local) is required for a signed Android release." >&2
    exit 2
  fi
fi

bash "$repo_root/platforms/rime/mobile/build_android_jni.sh"

if [[ "$variant" == "debug" ]]; then
  gradle_task="assembleDebug"
else
  gradle_task="assembleRelease"
fi
(
  cd "$script_dir"
  ./gradlew --no-daemon "$gradle_task"
)

apk="$apk_dir/app-$variant.apk"
if [[ ! -f "$apk" ]]; then
  echo "APK was not generated: $apk" >&2
  exit 1
fi

product_version="$(awk -F '"' '/^version[[:space:]]*=/ { print $2; exit }' "$repo_root/Cargo.toml")"
output_dir="$repo_root/build/android"
mkdir -p "$output_dir"
output="$output_dir/GonnyuGeneralIME-${product_version}-android-${variant}.apk"
cp "$apk" "$output"
echo "APK generated: $output"
