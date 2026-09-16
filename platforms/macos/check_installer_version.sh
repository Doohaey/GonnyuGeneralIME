#!/usr/bin/env bash
set -euo pipefail

incoming="${1:?incoming version required}"
installed_plist="${2:?installed Info.plist path required}"

parse_version() {
  local value="$1"
  if [[ "$value" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)-pre\.([0-9]+)$ ]]; then
    if (( 10#${BASH_REMATCH[4]} >= 1000 )); then
      echo "pre-release sequence must be below 1000: $value" >&2
      return 2
    fi
    parsed=("${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}" 0 "${BASH_REMATCH[4]}")
  elif [[ "$value" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    parsed=("${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}" 1 0)
  else
    echo "invalid installer version: $value" >&2
    return 2
  fi
}

parse_version "$incoming"
incoming_parts=("${parsed[@]}")

read_plist_version() {
  local key="$1"
  if [[ -x /usr/libexec/PlistBuddy ]]; then
    /usr/libexec/PlistBuddy -c "Print :$key" "$installed_plist" 2>/dev/null
    return
  fi
  python3 - "$installed_plist" "$key" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "rb") as source:
    value = plistlib.load(source).get(sys.argv[2])
if not isinstance(value, str):
    raise SystemExit(1)
print(value)
PY
}

if [[ ! -f "$installed_plist" ]]; then
  bundle_root="${installed_plist%/Contents/Info.plist}"
  if [[ -e "$bundle_root" ]]; then
    echo "installed input method has no readable Info.plist: $installed_plist" >&2
    exit 2
  fi
  exit 0
fi
installed="$(read_plist_version GannyuVersion)" || \
  installed="$(read_plist_version CFBundleShortVersionString)" || {
  echo "installed input method has no readable version: $installed_plist" >&2
  exit 2
}
parse_version "$installed"
installed_parts=("${parsed[@]}")

for index in 0 1 2 3 4; do
  incoming_number=$((10#${incoming_parts[$index]}))
  installed_number=$((10#${installed_parts[$index]}))
  if (( incoming_number > installed_number )); then exit 0; fi
  if (( incoming_number < installed_number )); then
    echo "installed Gonnyu version $installed is newer than package $incoming" >&2
    exit 1
  fi
done

if [[ "${GANNYU_ALLOW_SAME_VERSION:-0}" == "1" ]]; then exit 0; fi
echo "Gonnyu version $incoming is already installed" >&2
exit 1
