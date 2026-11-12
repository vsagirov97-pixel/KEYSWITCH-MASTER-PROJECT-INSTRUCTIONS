// The single boundary between the UI and the Rust backend.
// Every backend call goes through `api`; components never import `@tauri-apps/api` themselves.
//
// By design no type here carries user text: outcomes are metadata only (kind + reason code).

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

/** Reason codes from pa-prd.md (US-05, US-06, FR-SCOPE-7). */
export type RefusalReason =
  | "PASSWORD"
  | "ELEVATED"
  | "SECURE_DESKTOP"
  | "EXCLUDED_APP"
  | "READ_ONLY"
  | "TOO_LONG"
  | "LOW_CONFIDENCE"
  | "MIXED_CANCELLED";

export type OperationOutcome =
  | { kind: "converted" }
  | { kind: "nothing" }
  | { kind: "refused"; reason: RefusalReason };

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
  };
}

export const api: KeySwitchApi = createApi();
