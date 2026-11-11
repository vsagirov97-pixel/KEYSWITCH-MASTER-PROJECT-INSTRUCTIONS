#!/usr/bin/env bash
# Runs every check that can run on this host and prints a PASS/FAIL/SKIP summary.
# Usage: scripts/check.sh [--quick]   (--quick skips the Windows cross-check and supply chain)
set -u
cd "$(dirname "$0")/.."
QUICK="${1:-}"
declare -a RESULTS=()
run() {
  local name="$1"; shift
  if "$@" >/tmp/ks-check-$$.log 2>&1; then RESULTS+=("PASS  $name")
  else RESULTS+=("FAIL  $name"); echo "---- $name failed:"; tail -40 /tmp/ks-check-$$.log; fi
}
skip() { RESULTS+=("SKIP  $1 ($2)"); }

run "rustfmt"                cargo fmt --all --check
run "clippy ks-core"         cargo clippy -p ks-core --all-targets -- -D warnings
run "tests ks-core"          cargo test -p ks-core
run "clippy sandbox"         cargo clippy -p ks-sandbox-proto -p ks-sandbox --all-targets -- -D warnings
run "tests sandbox (hostile)" cargo test -p ks-sandbox-proto -p ks-sandbox
if [ -d app/node_modules ]; then
  run "frontend typecheck"   npm --prefix app run typecheck
  run "frontend build"       npm --prefix app run build
else
  skip "frontend" "run: npm --prefix app ci"
fi
if [ "$QUICK" != "--quick" ]; then
  if rustup target list --installed 2>/dev/null | grep -q x86_64-pc-windows-msvc && [ -d app/dist ]; then
    run "clippy app (windows target, check only)" cargo clippy -p keyswitch --target x86_64-pc-windows-msvc --all-targets -- -D warnings
  else
    skip "windows cross-check" "rustup target add x86_64-pc-windows-msvc; build frontend first"
  fi
  if rustup target list --installed 2>/dev/null | grep -q x86_64-pc-windows-msvc; then
    run "clippy sandbox (windows target, check only)" cargo clippy -p ks-sandbox-proto -p ks-sandbox --target x86_64-pc-windows-msvc --all-targets -- -D warnings
  else
    skip "sandbox windows cross-check" "rustup target add x86_64-pc-windows-msvc"
  fi
  # wasmtime's build script compiles a C helper; cross-checking macOS needs the macOS SDK.
  # On macOS the native "clippy sandbox" above already covers it.
  [ "$(uname -s)" != "Darwin" ] && skip "sandbox macOS cross-check" "needs macOS SDK; native clippy runs in CI job sandbox (macos-15)"
  if command -v cargo-deny >/dev/null; then run "cargo deny" cargo deny --all-features check
  else skip "cargo deny" "cargo install cargo-deny --version 0.19.9 --locked"; fi
  [ -d app/node_modules ] && run "npm audit" npm --prefix app audit --audit-level=high
fi
rm -f /tmp/ks-check-$$.log
echo; echo "==== Summary (this host: $(uname -s)) ===="; printf '%s\n' "${RESULTS[@]}"
echo "NOT RUN HERE: Windows runtime tests, installer build, real-app UI behaviour -> Windows CI / manual acceptance."
printf '%s\n' "${RESULTS[@]}" | grep -q '^FAIL' && exit 1 || exit 0
