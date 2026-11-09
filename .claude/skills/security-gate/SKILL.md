---
name: security-gate
description: Independent KeySwitch security review of a branch/PR/diff against docs/03-threat-model.md (T-01..T-16) and CLAUDE.md rules, producing findings with severities and a PASS/BLOCK verdict. Use before any merge touching Rust platform code, the sandbox, Tauri config, clipboard, dependencies or CI.
---
# /security-gate [branch or base..head]

Delegate to the `security-analyst` subagent (read-only) with the diff range; the author of a change must not be its security reviewer.

The analyst must check at least:
- [ ] No user text in logs/panics/`Debug`/`Display`/error strings; `UserText` used (T-09).
- [ ] No new network-capable crate or npm package in the runtime graph (`cargo tree -e normal`, imports) (T-11).
- [ ] Tauri capabilities still minimal; CSP unchanged or stricter; no fs/shell/http plugins (T-10).
- [ ] Every `unsafe` justified and confined to `ks-platform-win`.
- [ ] Clipboard: snapshot/restore-if-unchanged, exclusion formats present (T-04, T-05).
- [ ] Focus re-verification before writing; password / elevated / excluded refusals (T-06, T-08, T-15).
- [ ] Sandbox: no WASI preopens/sockets/env; limits set; actions validated by the engine; `AllowedKey` closed (T-01, T-02, T-12, T-13).
- [ ] Dependencies: justification present, `cargo deny` + `npm audit` pass, actions pinned by SHA (T-14).

Output: findings table (ID, severity, file:line, scenario, fix, verification), verdict, unverified items. Any open Critical/High ⇒ BLOCK; accepting residual risk requires the owner.
