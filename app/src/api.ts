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
  const mock = createMockApi();
  if (!isTauri()) return mock;
  // Beta 1: the backend exposes only `core_version` so far. Remaining calls stay on the mock
  // until the matching commands land (contract: docs/ui/beta1-ui-notes.md).
  return {
    ...mock,
    async getAppInfo() {
      const info = await mock.getAppInfo();
      const [version, coreVersion] = await Promise.all([
        getVersion().catch(() => info.version),
        invoke<string>("core_version").catch(() => info.coreVersion),
      ]);
      return { ...info, version, coreVersion };
    },
  };
}

export const api: KeySwitchApi = createApi();
