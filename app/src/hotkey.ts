// Hotkey helpers shared by the capture control, the tray model and the mock backend.
// Rule D-14: 2-3 keys, exactly one non-modifier key.

import type { Hotkey, Modifier } from "./api";

export const MIN_MODIFIERS = 1;
export const MAX_MODIFIERS = 2;

const MODIFIER_CODES = /^(Control|Shift|Alt|Meta|OS)(Left|Right)?$/;

export function isModifierCode(code: string): boolean {
  return MODIFIER_CODES.test(code);
}

interface ModifierFlags {
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}

export function modifiersOf(event: ModifierFlags): Modifier[] {
  const out: Modifier[] = [];
  if (event.ctrlKey) out.push("Ctrl");
  if (event.shiftKey) out.push("Shift");
  if (event.altKey) out.push("Alt");
  if (event.metaKey) out.push("Win");
  return out;
}

/** Layout-independent key name from `KeyboardEvent.code`: "KeyK" -> "K", "Digit1" -> "1". */
export function keyNameFromCode(code: string): string {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return code;
}

export function hotkeyParts(hotkey: Hotkey): string[] {
  return [...hotkey.modifiers, hotkey.key];
}

/** "Ctrl+Shift+Space" — for tooltips and prose ("+" is never used between key chips). */
export function formatHotkey(hotkey: Hotkey): string {
  return hotkeyParts(hotkey).join("+");
}
