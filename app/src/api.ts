// The single boundary between the UI and the Rust backend.
// Every backend call goes through `api`; components never import `@tauri-apps/api` themselves.
//
// By design no type here carries text from the user's applications: outcomes are metadata only
// (kind + reason code). The one exception is the Macro Studio slot's `content`, which is the
// user's own macro text typed in the editor (stored locally, never logged).

import { getVersion } from "@tauri-apps/api/app";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { createMockApi } from "./api.mock";

export type UiLanguage = "ru" | "en";
export type Theme = "dark" | "light" | "system";
export type ConversionLanguage = "ru" | "en";

export type Modifier = "Ctrl" | "Shift" | "Alt" | "Win";

/** `key` is a layout-independent key name derived from `KeyboardEvent.code` (e.g. "Space", "K", "F9"). */
export interface Hotkey {
  modifiers: Modifier[];
  key: string;
}

export interface Settings {
  hotkey: Hotkey;
  autostart: boolean;
  uiLanguage: UiLanguage;
  theme: Theme;
}

export type HotkeyError = "invalid" | "reserved" | "conflict";
export type HotkeyResult = { ok: true } | { ok: false; error: HotkeyError };

export type ExcludedAppSource = "default" | "user";
export interface ExcludedApp {
  exe: string;
  source: ExcludedAppSource;
}

export interface AppInfo {
  version: string;
  coreVersion: string;
  signed: boolean;
}

/**
 * Reason codes from pa-prd.md (US-05, US-06, FR-SCOPE-7) plus the codes of the Windows
 * backend (`ks_engine::Refusal`). Every code has a `status.reason.<CODE>` message.
 * `MIXED_CANCELLED` is UI-only: the backend reports a dismissed language choice as the
 * outcome `CANCELLED`, which is not a refusal and produces no notice.
 */
export type RefusalReason =
  | "PASSWORD"
  | "ELEVATED"
  | "SECURE_DESKTOP"
  | "EXCLUDED_APP"
  | "READ_ONLY"
  | "TOO_LONG"
  | "LOW_CONFIDENCE"
  | "MIXED_CANCELLED"
  | "SELF_TARGET"
  | "CONSOLE"
  | "MIXED"
  | "UNSUPPORTED"
  | "FORBIDDEN_CHARS"
  | "CLIPBOARD_UNAVAILABLE"
  | "CLIPBOARD_LISTENER"
  | "NOT_AVAILABLE";

/** A started operation stopped (`ks_engine::Failure`). Messages: `status.reason.<CODE>`. */
export type FailureReason = "TARGET_CHANGED" | "MODIFIERS_HELD" | "READ_FAILED" | "WRITE_FAILED";

/**
 * Result of the last hotkey operation as the backend reports it (command `last_operation`).
 * Codes and flags only. `code` is `CONVERTED`, `CONVERTED_UNVERIFIED`, `MACRO_INSERTED`,
 * `MACRO_INSERTED_UNVERIFIED` (kind `inserted`), `NOTHING`, `CANCELLED`,
 * a `RefusalReason` (kind `refused`) or a `FailureReason` (kind `failed`).
 */
export interface LastOperation {
  kind: "converted" | "inserted" | "nothing" | "cancelled" | "refused" | "failed";
  code: string;
  /** The clipboard from before the operation can still be restored from the tray menu. */
  clipboardHeld: boolean;
}

export type OperationOutcome =
  | { kind: "converted" }
  | { kind: "nothing" }
  | { kind: "refused"; reason: RefusalReason };

/** Macro Studio (Beta 2 step 1): one slot, text macros only. */
export type MacroType = "text";

/** Upper bound of `MacroSlot.content`, in characters (backend: `ks_engine::MACRO_MAX_CHARS`). */
export const MACRO_MAX_CHARS = 5000;

/** The slot as the editor edits it (command `set_macro_slot`, argument `slot`). */
export interface MacroSlot {
  /** At most 64 characters, no control characters. */
  name: string;
  enabled: boolean;
  /** `null` = no hotkey; same rules as the conversion hotkey and never the same combination. */
  hotkey: Hotkey | null;
  type: MacroType;
  /**
   * At most `MACRO_MAX_CHARS` characters. No control characters; `\n` and `\t` only with
   * `allowEnterTab` (D-18). They are inserted as characters, never pressed as keys.
   */
  content: string;
  /** D-18: off by default. */
  allowEnterTab: boolean;
}

/** `get_macro_slot`: the slot plus backend facts. */
export interface MacroSlotState extends MacroSlot {
  /** The slot's hotkey is registered with Windows right now. */
  registered: boolean;
  /** Test run from the editor; `false` in this step (show the button disabled). */
  testRunAvailable: boolean;
}

export type MacroSlotError =
  | HotkeyError
  | "name_invalid"
  | "type_unsupported"
  | "content_too_long"
  | "content_forbidden_chars"
  | "content_enter_tab_not_permitted"
  /** The slot is active but could not be written to disk. */
  | "save_failed";
export type MacroSlotResult = { ok: true } | { ok: false; error: MacroSlotError };

export interface KeySwitchApi {
  getSettings(): Promise<Settings>;
  /** Rejects when the OS autostart entry cannot be written. */
  setAutostart(enabled: boolean): Promise<void>;
  setUiLanguage(language: UiLanguage): Promise<void>;
  setTheme(theme: Theme): Promise<void>;
  /** On failure the previous hotkey stays registered. */
  setHotkey(hotkey: Hotkey): Promise<HotkeyResult>;
  listExcludedApps(): Promise<ExcludedApp[]>;
  addExcludedApp(exe: string): Promise<ExcludedApp[]>;
  removeExcludedApp(exe: string): Promise<ExcludedApp[]>;
  getAppInfo(): Promise<AppInfo>;
  /** `null` until the first operation since start. */
  getLastOperation(): Promise<LastOperation | null>;
  getMacroSlot(): Promise<MacroSlotState>;
  /** On failure the previous slot and its hotkey stay (except `save_failed`). */
  setMacroSlot(slot: MacroSlot): Promise<MacroSlotResult>;
}

function createApi(): KeySwitchApi {
  // Outside the Tauri shell (`npm run dev`, screenshots) the in-memory mock serves the UI.
  if (!isTauri()) return createMockApi();
  // Command names and shapes: docs/ui/beta1-ui-notes.md; Rust side: app/src-tauri/src/main.rs.
  return {
    getSettings: () => invoke<Settings>("get_settings"),
    setAutostart: (enabled) => invoke<void>("set_autostart", { enabled }),
    setUiLanguage: (language) => invoke<void>("set_ui_language", { language }),
    setTheme: (theme) => invoke<void>("set_theme", { theme }),
    setHotkey: (hotkey) => invoke<HotkeyResult>("set_hotkey", { hotkey }),
    listExcludedApps: () => invoke<ExcludedApp[]>("list_excluded_apps"),
    addExcludedApp: (exe) => invoke<ExcludedApp[]>("add_excluded_app", { exe }),
    removeExcludedApp: (exe) => invoke<ExcludedApp[]>("remove_excluded_app", { exe }),
    async getAppInfo() {
      const [version, coreVersion, signed] = await Promise.all([
        getVersion(),
        invoke<string>("core_version"),
        invoke<boolean>("build_signed"),
      ]);
      return { version, coreVersion, signed };
    },
    getLastOperation: () => invoke<LastOperation | null>("last_operation"),
    getMacroSlot: () => invoke<MacroSlotState>("get_macro_slot"),
    setMacroSlot: (slot) => invoke<MacroSlotResult>("set_macro_slot", { slot }),
  };
}

export const api: KeySwitchApi = createApi();
