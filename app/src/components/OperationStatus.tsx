// Result of the last operation: converted / nothing to convert / refused (+ reason).
// Metadata only — `OperationOutcome` cannot carry user text, so nothing here can render it.
import type { OperationOutcome } from "../api";
import { useT } from "../i18n";
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
