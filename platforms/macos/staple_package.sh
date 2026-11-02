#!/usr/bin/env bash
set -euo pipefail

package="${1:?usage: staple_package.sh package.pkg}"
for attempt in 1 2 3; do
  if xcrun stapler staple "$package"; then
    exit 0
  else
    status=$?
  fi
  if [[ "$status" != "68" || "$attempt" == "3" ]]; then
    exit "$status"
  fi
  echo "Apple ticket service unavailable; retrying stapling ($attempt/3)" >&2
  sleep "$((attempt * 5))"
done
