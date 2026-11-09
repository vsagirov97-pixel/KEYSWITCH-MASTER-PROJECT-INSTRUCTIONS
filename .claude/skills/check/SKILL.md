---
name: check
description: Run all KeySwitch checks available on this host (fmt, clippy, tests, frontend, Windows cross-check, cargo-deny, npm audit) and report results honestly. Use before every commit/PR and whenever asked whether something works.
---
# /check

1. If `app/node_modules` is missing, run `npm --prefix app ci`.
2. Run `scripts/check.sh` (or `scripts/check.sh --quick` when only Rust core / frontend changed and time matters).
3. Report the **Summary block verbatim**. For every FAIL, show the relevant error lines and either fix it or state why not.
4. Always add the "not verified here" line: Windows runtime behaviour, installer, real applications → Windows CI (give job name) or owner manual acceptance.
5. Never say "all tests pass" if anything was SKIP — name the skipped checks.
