# Beta 1 (Windows) UI notes — BL-UI-B1

Scope: conversion only. No macros, no updater UI, no payments. Source of look and wording:
`docs/research/design-system.md` (tokens copied 1:1 into `app/src/index.css`), `design-concept.md`, `pa-prd.md`.

Screenshots (mock backend, Chromium on Linux, 960x640 @2x — fonts differ from Segoe UI on Windows):
`beta1-settings-light.png`, `beta1-settings-dark.png`.

## Layout of `app/src`

| Path | What |
|---|---|
| `api.ts` | The only UI↔Rust boundary: types + `KeySwitchApi` + `api` instance |
| `api.mock.ts` | In-memory backend (no storage, no network) |
| `hotkey.ts` | Hotkey helpers (D-14 rule: 1–2 modifiers + one regular key) |
| `i18n/` | `ru.ts` (source of keys), `en.ts` (`Record<MessageKey, string>`: missing key = compile error) |
| `tray/model.ts` | `buildTrayMenu(state, t)` → JSON-serialisable `TrayMenuModel`; command ids `TrayCommand` |
| `components/` | `ui.tsx` primitives, `HotkeyCapture`, `LanguagePicker`, `OperationStatus` (+ `Toast`) |
| `settings/` | Settings window: General, Conversion, Excluded apps, About |
| `PreviewPage.tsx` | Dev-only gallery: `npm run dev` → `/?view=preview` (not in the production bundle) |

Dev-only mock overrides: `?theme=dark|light|system&lang=ru|en`.

## Backend contract (Rust)

Implemented in `app/src-tauri/src/main.rs` (BL-WIN-B1). Inside the Tauri shell `api.ts` calls these
commands; the mock serves only `npm run dev`. Settings live in `%APPDATA%\KeySwitch\settings.json`;
autostart state is read from the `HKCU` `Run` key (not stored in the file).

| `KeySwitchApi` method | Proposed command | Notes |
|---|---|---|
| `getSettings` | `get_settings` | `{ hotkey, autostart, uiLanguage, theme }` |
| `setAutostart(bool)` | `set_autostart` | error → UI rolls the toggle back and shows an inline alert |
| `setUiLanguage`, `setTheme` | `set_ui_language`, `set_theme` | tray menu must be rebuilt on language change |
| `setHotkey(hotkey)` | `set_hotkey` | returns `{ok:true}` or `{ok:false,error:"invalid"\|"reserved"\|"conflict"}`; previous hotkey stays registered on failure |
| `listExcludedApps` / `addExcludedApp` / `removeExcludedApp` | `list_/add_/remove_excluded_app` | backend re-validates the exe name (invalid name → rejected promise); `default` entries are not removable |
| `getAppInfo` | `core_version` + `getVersion()` + `build_signed` | `build_signed` returns `false` in Beta 1 (unsigned) |

`OperationOutcome` / `RefusalReason` carry reason codes only — no type on the boundary can hold user text.

## Deviations and simplest-option choices (need designer/owner confirmation)

1. **Result toast vs design rule "no success notifications"** (design-concept §5.1, design-system §9.10:
   toast = async errors only, never success/info). Built: `OperationStatus` (status pill + reason, usable
   in-window) and an error-only `Toast`. Neither is wired to real operations; where/if results are shown is open.
2. Navigation: 4 sidebar items (General / Conversion / Excluded apps / About) instead of the full design
   sidebar and the single scrolling Settings page; Overview, Diagnostics, Macro Studio, onboarding are out of scope.
3. Theme control (dark / light / system) added to General as in the design; default in the mock is `system`.
4. Hotkey: the capture field itself is the "change" control (§9.6). No "clear" button and no "restore previous"
   button — the hotkey can not be empty in Beta 1 and the previous one stays active on error.
5. Excluded apps: exe name typed by hand (no file dialog — no `fs` plugin). Default list in the mock is a placeholder.
6. Icons: four own minimal SVGs instead of Lucide (no new dependency); sidebar items have no icons.
7. Privacy block from the design is not shown (wording about network calls needs CLO after ADR-0011).
8. About: licence and third-party texts are derived from `LICENSE` / `THIRD_PARTY.md`; no clickable link
   (no `shell`/opener plugin) — plain text pointing to the bundled file. Wording needs CLO review.
9. Language picker: built as a component (keys 1 / 2 / Esc, arrows, 10 s timeout); the separate always-on-top
   OS window, its positioning and focus mode (a/b) are backend/architect work.
10. Key names on chips are not localised (`Space`, not `Пробел`).

## Not verified here

Real WebView2 rendering and the Tauri CSP (`style-src 'self'` — the code uses no inline styles, but this was
only checked in Chromium via the Vite dev server), Narrator/NVDA, high-contrast mode, 125–200 % Windows scaling.
No frontend test runner exists, so there are no component tests.
