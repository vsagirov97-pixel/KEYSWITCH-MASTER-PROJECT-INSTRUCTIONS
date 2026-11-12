#!/usr/bin/env bash
# WS-08 (S-2a Windows security review): fail if forbidden Win32 APIs or probe-only helpers
# appear in code outside crates/ks-winprobe, or if any other crate depends on ks-winprobe.
# Documentation (*.md) and this script are excluded: they name the APIs to forbid them.
set -u
cd "$(dirname "$0")/.."
PATTERN='SetWindowsHookEx|AttachThreadInput|CreateRemoteThread|WriteProcessMemory|send_vk_chord|send_vk_state'
status=0
if git grep -nE "$PATTERN" -- . ':!crates/ks-winprobe' ':!*.md' ':!scripts/check-forbidden-apis.sh'; then
  echo "FAIL: forbidden API / probe-only helper outside crates/ks-winprobe (see lines above)"
  status=1
fi
if git grep -nE '^\s*ks-winprobe\s*=|ks-winprobe\s*=\s*\{' -- '*Cargo.toml' ':!crates/ks-winprobe/Cargo.toml'; then
  echo "FAIL: a crate depends on ks-winprobe (probe code must not reach the product)"
  status=1
fi
[ "$status" -eq 0 ] && echo "OK: no forbidden APIs outside crates/ks-winprobe; no dependency on ks-winprobe"
exit "$status"
