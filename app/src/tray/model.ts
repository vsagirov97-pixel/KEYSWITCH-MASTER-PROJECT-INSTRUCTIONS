// Tray menu model (design-concept.md §4.8, Beta 1 subset: no Macro Studio / Diagnostics items).
// Pure data, JSON-serialisable: the Rust side can build the native menu from `TrayMenuModel`
// and send back `TrayCommand` ids. Labels come from i18n so the menu follows the UI language.

import type { Hotkey } from "../api";
import { formatHotkey } from "../hotkey";
import type { Translate } from "../i18n";

export type TrayStatus = "active" | "paused" | "attention";

export type TrayCommand =
  | "open"
  | "pause.15m"
  | "pause.1h"
  | "pause.untilRestart"
  | "resume"
  | "settings"
  | "quit";

export interface TrayState {
  status: TrayStatus;
  /** Locale-formatted end of the pause ("14:30"); null = until restart. Ignored unless paused. */
  pausedUntil: string | null;
  hotkey: Hotkey;
}

export type TrayItem =
  | { kind: "item"; id: TrayCommand; label: string; enabled: boolean; isDefault: boolean }
  | { kind: "submenu"; label: string; items: TrayItem[] }
  | { kind: "info"; label: string }
  | { kind: "separator" };

export interface TrayMenuModel {
  /** Which of the three tray icons to show; the icons differ by shape, not only colour. */
  icon: TrayStatus;
  tooltip: string;
  items: TrayItem[];
}

function item(id: TrayCommand, label: string, isDefault = false): TrayItem {
  return { kind: "item", id, label, enabled: true, isDefault };
}

export function buildTrayMenu(state: TrayState, t: Translate): TrayMenuModel {
  const statusLabel = t(`tray.status.${state.status}`);
  const pauseBlock: TrayItem[] =
    state.status === "paused"
      ? [
          {
            kind: "info",
            label:
              state.pausedUntil === null
                ? t("tray.pausedUntilRestart")
                : t("tray.pausedUntil", { time: state.pausedUntil }),
          },
          item("resume", t("tray.resume")),
        ]
      : [
          {
            kind: "submenu",
            label: t("tray.pause"),
            items: [
              item("pause.15m", t("tray.pause.15m")),
              item("pause.1h", t("tray.pause.1h")),
              item("pause.untilRestart", t("tray.pause.untilRestart")),
            ],
          },
        ];

  return {
    icon: state.status,
    tooltip: t("tray.tooltip", { status: statusLabel, hotkey: formatHotkey(state.hotkey) }),
    items: [
      item("open", t("tray.open"), true),
      { kind: "separator" },
      ...pauseBlock,
      { kind: "separator" },
      item("settings", t("tray.settings")),
      { kind: "separator" },
      item("quit", t("tray.quit")),
    ],
  };
}
