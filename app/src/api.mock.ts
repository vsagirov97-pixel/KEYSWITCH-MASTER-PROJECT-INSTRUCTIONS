// In-memory backend so the UI runs under `npm run dev` / `vite build` without the Rust side.
// No storage, no network, no user text.

import type { ExcludedApp, Hotkey, HotkeyResult, KeySwitchApi, Settings, Theme, UiLanguage } from "./api";
import { version as appVersion } from "../package.json";
import { MAX_MODIFIERS, MIN_MODIFIERS } from "./hotkey";

const RESERVED: readonly string[] = ["Ctrl+Alt+Delete", "Alt+Tab", "Alt+F4", "Win+L", "Win+D"];
const TAKEN: readonly string[] = ["Ctrl+Shift+Escape"];

function hotkeyId(hotkey: Hotkey): string {
  return [...hotkey.modifiers, hotkey.key].join("+");
}

/** Dev-only overrides (`?theme=light&lang=en`), used for screenshots. */
function initialOverrides(): Partial<Settings> {
  if (typeof window === "undefined") return {};
  const params = new URLSearchParams(window.location.search);
  const out: Partial<Settings> = {};
  const theme = params.get("theme");
  if (theme === "dark" || theme === "light" || theme === "system") out.theme = theme;
  const lang = params.get("lang");
  if (lang === "ru" || lang === "en") out.uiLanguage = lang;
  return out;
}

export function createMockApi(): KeySwitchApi {
  const settings: Settings = {
    hotkey: { modifiers: ["Ctrl", "Shift"], key: "Space" },
    autostart: true,
    uiLanguage: "ru",
    theme: "system",
    ...initialOverrides(),
  };
  let excluded: ExcludedApp[] = [
    { exe: "keepass.exe", source: "default" },
    { exe: "1password.exe", source: "default" },
    { exe: "windowsterminal.exe", source: "default" },
    { exe: "cmd.exe", source: "default" },
  ];
  const snapshot = (): ExcludedApp[] => excluded.map((app) => ({ ...app }));

  return {
    async getSettings() {
      return { ...settings, hotkey: { ...settings.hotkey, modifiers: [...settings.hotkey.modifiers] } };
    },
    async setAutostart(enabled: boolean) {
      settings.autostart = enabled;
    },
    async setUiLanguage(language: UiLanguage) {
      settings.uiLanguage = language;
    },
    async setTheme(theme: Theme) {
      settings.theme = theme;
    },
    async setHotkey(hotkey: Hotkey): Promise<HotkeyResult> {
      const count = hotkey.modifiers.length;
      if (count < MIN_MODIFIERS || count > MAX_MODIFIERS || hotkey.key === "") {
        return { ok: false, error: "invalid" };
      }
      const id = hotkeyId(hotkey);
      if (RESERVED.includes(id)) return { ok: false, error: "reserved" };
      if (TAKEN.includes(id)) return { ok: false, error: "conflict" };
      settings.hotkey = { modifiers: [...hotkey.modifiers], key: hotkey.key };
      return { ok: true };
    },
    async listExcludedApps() {
      return snapshot();
    },
    async addExcludedApp(exe: string) {
      const name = exe.trim().toLowerCase();
      if (!excluded.some((app) => app.exe === name)) excluded = [...excluded, { exe: name, source: "user" }];
      return snapshot();
    },
    async removeExcludedApp(exe: string) {
      excluded = excluded.filter((app) => !(app.exe === exe && app.source === "user"));
      return snapshot();
    },
    async getAppInfo() {
      return { version: appVersion, coreVersion: "mock", signed: false };
    },
    async getLastOperation() {
      return null;
    },
  };
}
