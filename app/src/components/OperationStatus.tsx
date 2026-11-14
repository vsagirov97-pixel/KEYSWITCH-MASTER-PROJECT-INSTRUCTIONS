// Result of the last operation: converted / nothing to convert / refused (+ reason).
// Metadata only — `OperationOutcome` cannot carry user text, so nothing here can render it.
import { useEffect, useState } from "react";
import { api, type LastOperation, type OperationOutcome } from "../api";
import { useT, type MessageKey } from "../i18n";
import { ru } from "../i18n/ru";
import { IconX, Pill, type Tone } from "./ui";

const TONE: Record<OperationOutcome["kind"], Tone> = {
  converted: "success",
  nothing: "neutral",
  refused: "warning",
};

export function OperationStatus({ outcome }: { outcome: OperationOutcome }) {
  const t = useT();
  return (
    <div role="status" className="flex flex-wrap items-center gap-2 text-base text-fg-secondary">
      <Pill tone={TONE[outcome.kind]}>{t(`status.${outcome.kind}`)}</Pill>
      {outcome.kind === "refused" ? <span>{t(`status.reason.${outcome.reason}`)}</span> : null}
    </div>
  );
}

const LAST_TONE: Record<LastOperation["kind"], Tone> = {
  converted: "success",
  inserted: "success",
  nothing: "neutral",
  cancelled: "neutral",
  refused: "warning",
  failed: "danger",
};

/** `status.reason.<code>` when such a message exists (refusal and failure codes). */
function reasonKey(code: string): MessageKey | null {
  const key = `status.reason.${code}`;
  // Every dictionary has the same keys (`Record<MessageKey, string>`), so `ru` decides.
  return Object.hasOwn(ru, key) ? (key as MessageKey) : null;
}

/**
 * Result of the last hotkey operation (command `last_operation`), refreshed when the window
 * regains focus. Codes only: the message of a refusal / failure code and the code itself.
 */
export function LastOperationStatus() {
  const t = useT();
  const [last, setLast] = useState<LastOperation | null>(null);

  useEffect(() => {
    let alive = true;
    const refresh = () => {
      api
        .getLastOperation()
        .then((value) => {
          if (alive) setLast(value);
        })
        .catch(() => undefined);
    };
    refresh();
    window.addEventListener("focus", refresh);
    return () => {
      alive = false;
      window.removeEventListener("focus", refresh);
    };
  }, []);

  if (!last) {
    return <p className="text-base text-fg-secondary">{t("lastOp.none")}</p>;
  }
  const reason = reasonKey(last.code);
  return (
    <div role="status" className="flex flex-wrap items-center gap-2 text-base text-fg-secondary">
      <Pill tone={LAST_TONE[last.kind]}>{t(`status.${last.kind}`)}</Pill>
      {reason ? <span>{t(reason)}</span> : null}
      <code className="font-mono text-xs text-fg-muted">{last.code}</code>
      {last.clipboardHeld ? <span>{t("status.reason.CLIPBOARD_HELD")}</span> : null}
    </div>
  );
}

interface ToastProps {
  title: string;
  children: string;
  onClose: () => void;
}

/**
 * Toast (design-system.md §9.10): only for async errors not tied to a visible element.
 * No auto-hide; the caller shows at most one at a time. There is no success/info variant.
 */
export function Toast({ title, children, onClose }: ToastProps) {
  const t = useT();
  return (
    <div
      role="alert"
      className="fixed right-4 bottom-4 z-[1100] flex w-90 max-w-[calc(100vw-2rem)] items-start gap-3 rounded-lg border-l-[3px] border-danger bg-overlay py-3 pr-2 pl-4 text-fg shadow-2"
    >
      <div className="min-w-0 flex-1">
        <p className="text-base font-semibold">{title}</p>
        <p className="text-base text-fg-secondary">{children}</p>
      </div>
      <button
        type="button"
        aria-label={t("common.close")}
        title={t("common.close")}
        onClick={onClose}
        className="inline-flex size-6 shrink-0 items-center justify-center rounded-md text-fg-secondary transition-colors duration-(--ks-dur-fast) hover:bg-hover hover:text-fg"
      >
        <IconX />
      </button>
    </div>
  );
}
