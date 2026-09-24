#!/usr/bin/env bash

gonnyu_log_file="/Users/Shared/GonnyuInputMethod/installer.log"

gonnyu_log() {
  printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" >> "$gonnyu_log_file" 2>/dev/null || true
}

gonnyu_init_logging() {
  mkdir -p "${gonnyu_log_file%/*}" 2>/dev/null || true
  if ! touch "$gonnyu_log_file" 2>/dev/null; then
    return 0
  fi
  exec > >(/usr/bin/tee -a "$gonnyu_log_file") 2>&1 || true
  gonnyu_log "started ${0##*/}"
}

gonnyu_fail() {
  status="$?"
  gonnyu_log "failed ${0##*/} with exit status $status"
  printf 'Gonnyu installer failed. Detailed log: %s\n' "$gonnyu_log_file" >&2
  exit "$status"
}

gonnyu_console_home() {
  console_user="$(stat -f '%Su' /dev/console 2>/dev/null || true)"
  if [[ -n "$console_user" && "$console_user" != root && "$console_user" != loginwindow && -d "/Users/$console_user" ]]; then
    printf '/Users/%s\n' "$console_user"
  fi
}

gonnyu_resolve_target() {
  gonnyu_system_target='/Library/Input Methods/GonnyuInputMethod.app'
  gonnyu_user_home="$(gonnyu_console_home)"
  gonnyu_user_target=''
  if [[ -n "$gonnyu_user_home" ]]; then
    gonnyu_user_target="$gonnyu_user_home/Library/Input Methods/GonnyuInputMethod.app"
  fi
  if [[ -n "$gonnyu_user_target" && -d "$gonnyu_user_target" ]]; then
    gonnyu_target="$gonnyu_user_target"
  else
    gonnyu_target="$gonnyu_system_target"
  fi
  gonnyu_backup="${gonnyu_target%/*}/.GonnyuInputMethod.previous.app"
  gonnyu_failed="$gonnyu_target.failed"
}

gonnyu_recovery_paths() {
  printf '%s\n' '/Library/Input Methods/.GonnyuInputMethod.previous.app'
  if [[ -n "${gonnyu_user_home:-}" ]]; then
    printf '%s\n' "$gonnyu_user_home/Library/Input Methods/.GonnyuInputMethod.previous.app"
  fi
}

gonnyu_failed_paths() {
  printf '%s\n' '/Library/Input Methods/GonnyuInputMethod.app.failed'
  if [[ -n "${gonnyu_user_home:-}" ]]; then
    printf '%s\n' "$gonnyu_user_home/Library/Input Methods/GonnyuInputMethod.app.failed"
  fi
}
