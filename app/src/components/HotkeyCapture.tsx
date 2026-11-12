// Hotkey capture control (design-system.md §9.6). The field itself is the "change" control.
// Capture uses ordinary keydown events of the focused element only — no global hooks.
import { useId, useState, type KeyboardEvent } from "react";
import type { Hotkey, HotkeyError, HotkeyResult } from "../api";
import { MAX_MODIFIERS, MIN_MODIFIERS, hotkeyParts, isModifierCode, keyNameFromCode, modifiersOf } from "../hotkey";
import { useT } from "../i18n";
import { Kbd, Pill, Spinner, cx } from "./ui";

type Phase = "idle" | "recording" | "validating";
type CaptureError = HotkeyError | "failed";

interface HotkeyCaptureProps {
  id: string;
  actionLabel: string;
  value: Hotkey;
  /** Registers the hotkey with the backend; on failure the previous one stays active. */
  onCommit: (hotkey: Hotkey) => Promise<HotkeyResult>;
}

export function HotkeyCapture({ id, actionLabel, value, onCommit }: HotkeyCaptureProps) {
  const t = useT();
  const errorId = useId();
  const [phase, setPhase] = useState<Phase>("idle");
  const [held, setHeld] = useState<string[]>([]);
  const [error, setError] = useState<CaptureError | null>(null);

  function startRecording() {
    setError(null);
    setHeld([]);
    setPhase("recording");
  }

  async function commit(hotkey: Hotkey) {
    setPhase("validating");
    try {
      const result = await onCommit(hotkey);
      if (!result.ok) setError(result.error);
    } catch {
      setError("failed");
    }
    setHeld([]);
    setPhase("idle");
  }

  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (phase !== "recording") return;
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
      setError("invalid");
      setPhase("idle");
      return;
    }
    void commit({ modifiers, key: keyNameFromCode(event.code) });
  }

  const recording = phase === "recording";
  const parts = recording ? held : hotkeyParts(value);

  return (
    <div className="flex flex-col items-end gap-2">
      <div className="flex flex-wrap items-center justify-end gap-2">
        {phase === "idle" && error === null ? <Pill tone="success">{t("hotkey.registered")}</Pill> : null}
        <button
          id={id}
          type="button"
          aria-label={t("hotkey.aria", { action: actionLabel, keys: hotkeyParts(value).join(" + ") })}
          aria-describedby={error ? errorId : undefined}
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
          {recording && held.length === 0 ? <span className="text-sm text-fg-muted">{t("hotkey.recording")}</span> : null}
          {phase === "validating" ? (
            <span className="ml-auto inline-flex items-center gap-2 text-sm text-fg-muted">
              <Spinner />
              {t("hotkey.validating")}
            </span>
          ) : null}
        </button>
      </div>
      <span aria-live="polite" className="sr-only">
        {recording ? t("hotkey.recording") : ""}
      </span>
      {error ? (
        <p id={errorId} role="alert" className="max-w-90 text-right text-sm text-danger">
          {t(`hotkey.error.${error}`)}
        </p>
      ) : null}
    </div>
  );
}
