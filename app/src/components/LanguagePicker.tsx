// "Choose language" popup for mixed text (design-system.md §9.12, design-concept.md §4.9).
// Never shows the user's text. Option order is fixed: 1 = Russian, 2 = English, Esc = cancel.
import { useEffect, useId, useRef, type KeyboardEvent } from "react";
import type { ConversionLanguage } from "../api";
import { useT, type MessageKey } from "../i18n";
import { Kbd } from "./ui";

/** Must match the `ks-timeout_10s` animation below. */
export const PICKER_TIMEOUT_MS = 10_000;

const OPTIONS: ReadonlyArray<{ language: ConversionLanguage; shortcut: string; label: MessageKey; tag: string }> = [
  { language: "ru", shortcut: "1", label: "picker.ru", tag: "RU" },
  { language: "en", shortcut: "2", label: "picker.en", tag: "EN" },
];

interface LanguagePickerProps {
  onChoose: (language: ConversionLanguage) => void;
  /** Called on Esc, on the cancel button and on timeout. Nothing must change in that case. */
  onCancel: () => void;
}

export function LanguagePicker({ onChoose, onCancel }: LanguagePickerProps) {
  const t = useT();
  const titleId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const cancelRef = useRef(onCancel);
  cancelRef.current = onCancel;

  useEffect(() => {
    rootRef.current?.focus(); // neutral initial focus: neither option is pre-selected
    const timer = window.setTimeout(() => cancelRef.current(), PICKER_TIMEOUT_MS);
    return () => window.clearTimeout(timer);
  }, []);

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.ctrlKey || event.altKey || event.metaKey) return;
    if (event.key === "Escape") {
      event.preventDefault();
      onCancel();
      return;
    }
    // `code` keeps 1/2 working regardless of the active keyboard layout.
    const option = OPTIONS.find((o) => event.code === `Digit${o.shortcut}` || event.code === `Numpad${o.shortcut}`);
    if (option) {
      event.preventDefault();
      onChoose(option.language);
      return;
    }
    if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
      event.preventDefault();
      const current = optionRefs.current.findIndex((element) => element === document.activeElement);
      const step = event.key === "ArrowRight" ? 1 : -1;
      const next = current === -1 ? (step === 1 ? 0 : OPTIONS.length - 1) : (current + step + OPTIONS.length) % OPTIONS.length;
      optionRefs.current[next]?.focus();
    }
  }

  return (
    <div
      ref={rootRef}
      role="dialog"
      aria-labelledby={titleId}
      tabIndex={-1}
      onKeyDown={onKeyDown}
      className="relative w-80 outline-none animate-[ks-picker-in_100ms_var(--ease-standard)] overflow-hidden rounded-xl bg-overlay p-4 text-fg shadow-3"
    >
      <h2 id={titleId} className="text-base font-semibold">
        {t("picker.title")}
      </h2>
      <div className="mt-3 grid grid-cols-2 gap-2">
        {OPTIONS.map((option, index) => (
          <button
            key={option.language}
            ref={(element) => {
              optionRefs.current[index] = element;
            }}
            type="button"
            aria-keyshortcuts={option.shortcut}
            onClick={() => onChoose(option.language)}
            className="inline-flex h-10 items-center gap-2 rounded-md border border-border-strong bg-raised px-3 text-left transition-colors duration-(--ks-dur-fast) hover:bg-hover active:bg-selected"
          >
            <Kbd>{option.shortcut}</Kbd>
            <span className="min-w-0 flex-1 truncate text-md font-medium">{t(option.label)}</span>
            <span className="font-mono text-xs text-fg-muted">{option.tag}</span>
          </button>
        ))}
      </div>
      <button
        type="button"
        aria-keyshortcuts="Escape"
        onClick={onCancel}
        className="mt-2 inline-flex h-8 items-center gap-2 rounded-md px-1 text-base text-fg-secondary transition-colors duration-(--ks-dur-fast) hover:bg-hover hover:text-fg active:bg-overlay"
      >
        <Kbd>Esc</Kbd>
        {t("picker.cancel")}
      </button>
      <p className="mt-1 hidden text-sm text-fg-muted motion-reduce:block">
        {t("picker.timeout", { seconds: PICKER_TIMEOUT_MS / 1000 })}
      </p>
      <div
        aria-hidden="true"
        className="absolute inset-x-0 bottom-0 h-0.5 origin-left animate-[ks-timeout_10s_linear_forwards] bg-accent motion-reduce:hidden"
      />
    </div>
  );
}
