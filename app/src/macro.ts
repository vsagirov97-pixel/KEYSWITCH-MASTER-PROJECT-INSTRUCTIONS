// Client-side mirror of the backend's save-time rules for the Macro Studio slot, so the editor
// can explain a problem before Save. The backend stays authoritative and re-checks everything:
// - content: `ks_engine::check_content` (crates/ks-engine/src/macros.rs)
// - name: `validate` in app/src-tauri/src/macro_slot.rs
// Lengths are counted in Unicode scalar values, like Rust's `str::chars().count()`.
// Nothing here logs or stores the text.

import { MACRO_MAX_CHARS, type MacroSlot } from "./api";
import { sameHotkey } from "./hotkey";

export const MACRO_NAME_MAX_CHARS = 64;

/** Rust `char::is_control`: general category Cc. */
function isControl(codePoint: number): boolean {
  return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f);
}

/** `always_forbidden` of `ks_engine::macros`. */
function isAlwaysForbidden(codePoint: number): boolean {
  if (isControl(codePoint)) return codePoint !== 0x0a && codePoint !== 0x09;
  return (
    codePoint === 0x2028 ||
    codePoint === 0x2029 ||
    codePoint === 0x061c ||
    codePoint === 0x200e ||
    codePoint === 0x200f ||
    (codePoint >= 0x202a && codePoint <= 0x202e) ||
    (codePoint >= 0x2066 && codePoint <= 0x2069)
  );
}

export function charCount(text: string): number {
  return Array.from(text).length;
}

export type ContentProblem =
  | { kind: "too_long" }
  /** `position` is 1-based, in characters; `codePoint` is shown as U+XXXX. */
  | { kind: "forbidden"; position: number; codePoint: number }
  | { kind: "enter_tab" };

/** Same order of checks as `check_content`: the first problem wins. */
export function contentProblem(text: string, allowEnterTab: boolean): ContentProblem | null {
  let position = 0;
  for (const char of text) {
    position += 1;
    if (position > MACRO_MAX_CHARS) return { kind: "too_long" };
    const codePoint = char.codePointAt(0) ?? 0;
    if (isAlwaysForbidden(codePoint)) return { kind: "forbidden", position, codePoint };
    if ((codePoint === 0x0a || codePoint === 0x09) && !allowEnterTab) return { kind: "enter_tab" };
  }
  return null;
}

export function nameInvalid(name: string): boolean {
  if (charCount(name) > MACRO_NAME_MAX_CHARS) return true;
  for (const char of name) {
    if (isControl(char.codePointAt(0) ?? 0)) return true;
  }
  return false;
}

export function formatCodePoint(codePoint: number): string {
  return `U+${codePoint.toString(16).toUpperCase().padStart(4, "0")}`;
}

export function sameSlot(a: MacroSlot, b: MacroSlot): boolean {
  const sameKeys = a.hotkey === null || b.hotkey === null ? a.hotkey === b.hotkey : sameHotkey(a.hotkey, b.hotkey);
  return (
    a.name === b.name &&
    a.enabled === b.enabled &&
    a.type === b.type &&
    a.content === b.content &&
    a.allowEnterTab === b.allowEnterTab &&
    sameKeys
  );
}

/** The editable part of `MacroSlotState` (drops the backend facts). */
export function slotOf(state: MacroSlot): MacroSlot {
  return {
    name: state.name,
    enabled: state.enabled,
    hotkey: state.hotkey ? { modifiers: [...state.hotkey.modifiers], key: state.hotkey.key } : null,
    type: state.type,
    content: state.content,
    allowEnterTab: state.allowEnterTab,
  };
}
