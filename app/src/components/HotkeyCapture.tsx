// Hotkey capture control (design-system.md §9.6). The field itself is the "change" control.
// Capture uses ordinary keydown events of the focused element only — no global hooks.
import { useId, useState, type KeyboardEvent, type ReactNode } from "react";
import type { Hotkey, HotkeyError } from "../api";
import { MAX_MODIFIERS, MIN_MODIFIERS, hotkeyParts, isModifierCode, keyNameFromCode, modifiersOf } from "../hotkey";
import { useT } from "../i18n";
import { IconX, Kbd, Pill, Spinner, cx } from "./ui";

type Phase = "idle" | "recording" | "validating";

/**
 * Error states of the field. `duplicate` is UI-only: the combination is already used by the
 * other KeySwitch action (the backend reports this case as `conflict`).
 */
export type CaptureError = HotkeyError | "duplicate" | "failed";
export type CaptureResult = { ok: true } | { ok: false; error: CaptureError };

interface HotkeyCaptureProps {
  id: string;
  actionLabel: string;
  /** `null` = no hotkey (only where `onClear` allows an empty value). */
  value: Hotkey | null;
  /** Registers (or, in a form, accepts) the hotkey; on failure the previous one stays. */
  onCommit: (hotkey: Hotkey) => Promise<CaptureResult>;
  /** Shows the "Clear" icon button and lets Backspace / Delete clear the value. */
  onClear?: () => void;
  /** An error reported from outside (e.g. by the backend on save); local errors take priority. */
  error?: CaptureError | null;
  /** Called when the user starts a new recording or clears the value, so the owner can drop `error`. */
  onErrorReset?: () => void;
  /** Label of the other action for the `duplicate` message. */
  otherActionLabel?: string;
  /** Status shown next to the field when idle without error; default — the "Registered" pill. */
  status?: ReactNode;
  describedBy?: string;
}

export function HotkeyCapture({
  id,
  actionLabel,
  value,
  onCommit,
  onClear,
  error: externalError = null,
  onErrorReset,
  otherActionLabel = "",
  status,
  describedBy,
}: HotkeyCaptureProps) {
  const t = useT();
  const errorId = useId();
  const [phase, setPhase] = useState<Phase>("idle");
  const [held, setHeld] = useState<string[]>([]);
  const [localError, setLocalError] = useState<CaptureError | null>(null);
  const error = localError ?? externalError;

  function resetErrors() {
    setLocalError(null);
    onErrorReset?.();
  }

  function startRecording() {
    resetErrors();
    setHeld([]);
    setPhase("recording");
  }

  async function commit(hotkey: Hotkey) {
    setPhase("validating");
    try {
      const result = await onCommit(hotkey);
      if (!result.ok) setLocalError(result.error);
    } catch {
      setLocalError("failed");
    }
    setHeld([]);
    setPhase("idle");
  }

  function clear() {
    resetErrors();
    onClear?.();
  }

  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (phase !== "recording") {
      if (onClear && value !== null && (event.key === "Backspace" || event.key === "Delete")) {
        event.preventDefault();
        clear();
      }
      return;
    }
    if (event.key === "Tab") {
      setPhase("idle"); // leave recording without changes; focus moves on
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      setPhase("idle");
      return;
    }
    const modifiers = modifiersOf(event);
    if (isModifierCode(event.code)) {
      setHeld(modifiers);
      return;
    }
    if (event.repeat) return;
    if (modifiers.length < MIN_MODIFIERS || modifiers.length > MAX_MODIFIERS) {
      setLocalError("invalid");
      setPhase("idle");
      return;
    }
    void commit({ modifiers, key: keyNameFromCode(event.code) });
  }

  const recording = phase === "recording";
  const parts = recording ? held : value ? hotkeyParts(value) : [];
  const keysLabel = value ? hotkeyParts(value).join(" + ") : t("hotkey.empty");
  const describedByIds = [describedBy, error ? errorId : undefined].filter(Boolean).join(" ") || undefined;
  const idleStatus = status === undefined ? <Pill tone="success">{t("hotkey.registered")}</Pill> : status;

  return (
    <div className="flex flex-col items-end gap-2">
      <div className="flex flex-wrap items-center justify-end gap-2">
        {phase === "idle" && error === null ? idleStatus : null}
        <div className="relative">
          <button
            id={id}
            type="button"
            aria-label={t("hotkey.aria", { action: actionLabel, keys: keysLabel })}
            aria-describedby={describedByIds}
            aria-invalid={error !== null}
            aria-busy={phase === "validating"}
            disabled={phase === "validating"}
            onClick={() => {
              if (!recording) startRecording();
            }}
            onKeyDown={onKeyDown}
            onBlur={() => {
              if (recording) setPhase("idle");
            }}
            className={cx(
              "inline-flex h-8 min-w-55 items-center gap-0.5 rounded-md border bg-raised px-3 text-left transition-colors duration-(--ks-dur-fast)",
              onClear && value !== null && !recording ? "pr-9" : null,
              recording
                ? "border-accent outline-2 outline-offset-2 outline-focus"
                : error
                  ? "border-danger"
                  : "border-border-strong hover:border-fg-muted",
            )}
          >
            {recording ? (
              <span className="mr-1.5 size-1.5 shrink-0 animate-[ks-pulse_1.2s_ease-in-out_infinite] rounded-full bg-danger" />
            ) : null}
            {parts.map((part) => (
              <Kbd key={part}>{part}</Kbd>
            ))}
            {!recording && value === null && phase === "idle" ? (
              <span className="text-base text-fg-muted">{t("hotkey.empty")}</span>
            ) : null}
            {recording && held.length === 0 ? <span className="text-sm text-fg-muted">{t("hotkey.recording")}</span> : null}
            {phase === "validating" ? (
              <span className="ml-auto inline-flex items-center gap-2 text-sm text-fg-muted">
                <Spinner />
                {t("hotkey.validating")}
              </span>
            ) : null}
          </button>
          {onClear && value !== null && !recording && phase === "idle" ? (
            <button
              type="button"
              aria-label={t("hotkey.clear.aria", { action: actionLabel })}
              title={t("hotkey.clear")}
              onClick={clear}
              className="absolute top-1 right-1 inline-flex size-6 items-center justify-center rounded-md text-fg-secondary transition-colors duration-(--ks-dur-fast) hover:bg-hover hover:text-fg"
            >
              <IconX />
            </button>
          ) : null}
        </div>
      </div>
      <span aria-live="polite" className="sr-only">
        {recording ? t("hotkey.recording") : ""}
      </span>
      {error ? (
        <p id={errorId} role="alert" className="max-w-90 text-right text-sm text-danger">
          {error === "duplicate" ? t("hotkey.error.duplicate", { action: otherActionLabel }) : t(`hotkey.error.${error}`)}
        </p>
      ) : null}
    </div>
  );
}
