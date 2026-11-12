// Design-system primitives (docs/research/design-system.md §9).
// No inline `style` attributes anywhere: the app CSP is `style-src 'self'`.
import {
  useId,
  useLayoutEffect,
  useRef,
  type ComponentPropsWithRef,
  type KeyboardEvent,
  type ReactNode,
} from "react";

export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

/* ---------- Icons (own minimal geometry, 16 px, currentColor) ---------- */

function Svg({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      className={cx("size-4 shrink-0", className)}
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

export function IconX({ className }: { className?: string }) {
  return (
    <Svg className={className}>
      <path d="M4 4l8 8M12 4l-8 8" />
    </Svg>
  );
}

export function IconLock({ className }: { className?: string }) {
  return (
    <Svg className={className}>
      <rect x="3.5" y="7" width="9" height="6.5" rx="1.5" />
      <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
    </Svg>
  );
}

export function IconAlert({ className }: { className?: string }) {
  return (
    <Svg className={className}>
      <circle cx="8" cy="8" r="6" />
      <path d="M8 5v3.5M8 11h.01" />
    </Svg>
  );
}

export function Spinner() {
  return (
    <span
      aria-hidden="true"
      className="inline-block size-3.5 shrink-0 animate-[ks-spin_0.8s_linear_infinite] rounded-full border-2 border-border-strong border-t-accent-fg"
    />
  );
}

/* ---------- Button ---------- */

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

const BUTTON_VARIANTS: Record<ButtonVariant, string> = {
  primary:
    "bg-accent text-on-accent enabled:hover:bg-accent-hover enabled:active:bg-accent-pressed disabled:bg-hover disabled:text-fg-disabled",
  secondary:
    "border border-border-strong bg-raised text-fg enabled:hover:bg-hover enabled:active:bg-overlay disabled:border-border disabled:text-fg-disabled",
  ghost:
    "text-fg-secondary enabled:hover:bg-hover enabled:hover:text-fg enabled:active:bg-overlay disabled:text-fg-disabled",
  danger:
    "bg-danger-solid text-on-danger enabled:hover:brightness-[0.94] enabled:active:brightness-[0.88] disabled:bg-hover disabled:text-fg-disabled",
};

interface ButtonProps extends ComponentPropsWithRef<"button"> {
  variant?: ButtonVariant;
}

export function Button({ variant = "secondary", className, type = "button", ...rest }: ButtonProps) {
  return (
    <button
      type={type}
      className={cx(
        "inline-flex h-8 min-w-18 items-center justify-center gap-2 rounded-md px-3 text-base font-medium whitespace-nowrap transition-colors duration-(--ks-dur-fast) disabled:cursor-not-allowed",
        BUTTON_VARIANTS[variant],
        className,
      )}
      {...rest}
    />
  );
}

/* ---------- Key chip ---------- */

export function Kbd({ children, muted = false }: { children: ReactNode; muted?: boolean }) {
  return (
    <kbd
      className={cx(
        "inline-flex min-w-5.5 items-center justify-center rounded-sm border border-b-2 border-border bg-overlay px-1.5 py-px font-mono text-xs font-medium",
        muted ? "text-fg-disabled" : "text-fg",
      )}
    >
      {children}
    </kbd>
  );
}

/* ---------- Toggle ---------- */

interface ToggleProps {
  id: string;
  checked: boolean;
  pending?: boolean;
  onChange: (next: boolean) => void;
  describedBy?: string;
}

export function Toggle({ id, checked, pending = false, onChange, describedBy }: ToggleProps) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      aria-busy={pending}
      aria-describedby={describedBy}
      onClick={() => {
        if (!pending) onChange(!checked);
      }}
      className={cx(
        "inline-flex h-5 w-9 shrink-0 items-center rounded-full border px-0.5 transition-colors duration-(--ks-dur-base)",
        checked ? "border-accent bg-accent hover:bg-accent-hover" : "border-border-strong hover:bg-hover",
      )}
    >
      <span
        className={cx(
          "size-3.5 rounded-full transition-transform duration-(--ks-dur-base) ease-standard",
          checked ? "translate-x-4 bg-on-accent" : "bg-fg-secondary",
        )}
      />
    </button>
  );
}

/* ---------- Segmented control ---------- */

interface SegmentedProps<T extends string> {
  label: string;
  value: T;
  options: ReadonlyArray<{ value: T; label: string }>;
  onChange: (next: T) => void;
}

export function Segmented<T extends string>({ label, value, options, onChange }: SegmentedProps<T>) {
  const refs = useRef<Array<HTMLButtonElement | null>>([]);

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const index = options.findIndex((option) => option.value === value);
    let next = index;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") next = (index + 1) % options.length;
    else if (event.key === "ArrowLeft" || event.key === "ArrowUp") next = (index - 1 + options.length) % options.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = options.length - 1;
    else return;
    event.preventDefault();
    const option = options[next];
    if (!option) return;
    onChange(option.value);
    refs.current[next]?.focus();
  }

  return (
    <div
      role="radiogroup"
      aria-label={label}
      onKeyDown={onKeyDown}
      className="inline-flex h-8 items-center gap-0.5 rounded-md border border-border bg-surface p-0.5"
    >
      {options.map((option, index) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            ref={(element) => {
              refs.current[index] = element;
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(option.value)}
            className={cx(
              "h-6.5 rounded-sm border px-3 text-base font-medium whitespace-nowrap transition-colors duration-(--ks-dur-fast)",
              selected
                ? "border-border-strong bg-selected text-fg shadow-1 dark:bg-raised"
                : "border-transparent text-fg-secondary hover:bg-hover hover:text-fg",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

/* ---------- Status pill ---------- */

export type Tone = "success" | "warning" | "danger" | "info" | "neutral";

const PILL_TONES: Record<Tone, string> = {
  success: "bg-success-bg text-success",
  warning: "bg-warning-bg text-warning",
  danger: "bg-danger-bg text-danger",
  info: "bg-info-bg text-info",
  neutral: "bg-hover text-fg-secondary",
};

/** Marker shape differs per tone so the status is never conveyed by colour alone. */
function PillMarker({ tone }: { tone: Tone }) {
  const common = { viewBox: "0 0 10 10", className: "size-2.5 shrink-0", "aria-hidden": true } as const;
  switch (tone) {
    case "success":
      return (
        <svg {...common}>
          <circle cx="5" cy="5" r="4" fill="currentColor" />
        </svg>
      );
    case "warning":
      return (
        <svg {...common}>
          <path d="M5 1l4.5 8h-9z" fill="currentColor" />
        </svg>
      );
    case "danger":
      return (
        <svg {...common}>
          <path d="M2 2l6 6M8 2l-6 6" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" />
        </svg>
      );
    case "info":
      return (
        <svg {...common}>
          <path d="M5 4.5V8.5M5 1.75v.5" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" />
        </svg>
      );
    case "neutral":
      return (
        <svg {...common}>
          <circle cx="5" cy="5" r="3.5" fill="none" stroke="currentColor" strokeWidth="1.25" strokeDasharray="2 1.6" />
        </svg>
      );
  }
}

export function Pill({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <span
      className={cx(
        "inline-flex h-5 items-center gap-1 rounded-full px-2 text-xs font-semibold whitespace-nowrap",
        PILL_TONES[tone],
      )}
    >
      <PillMarker tone={tone} />
      {children}
    </span>
  );
}

/* ---------- Inline alert (no success variant by design) ---------- */

type AlertTone = "info" | "warning" | "danger";

const ALERT_TONES: Record<AlertTone, string> = {
  info: "border-info bg-info-bg",
  warning: "border-warning bg-warning-bg",
  danger: "border-danger bg-danger-bg",
};
const ALERT_ICON: Record<AlertTone, string> = { info: "text-info", warning: "text-warning", danger: "text-danger" };

interface InlineAlertProps {
  tone: AlertTone;
  title?: string;
  children?: ReactNode;
  /** `alert` only for a danger that answers a user action; otherwise `status`. */
  role?: "alert" | "status";
  id?: string;
}

export function InlineAlert({ tone, title, children, role = "status", id }: InlineAlertProps) {
  return (
    <div
      id={id}
      role={role}
      className={cx("flex gap-2 rounded-md border-l-[3px] px-4 py-3 text-base text-fg", ALERT_TONES[tone])}
    >
      <IconAlert className={cx("mt-0.5", ALERT_ICON[tone])} />
      <div className="min-w-0">
        {title ? <p className="font-semibold">{title}</p> : null}
        {children}
      </div>
    </div>
  );
}

/* ---------- Modal (native <dialog>: focus trap, Esc, focus return) ---------- */

interface ModalProps {
  title: string;
  children: ReactNode;
  cancelLabel: string;
  actionLabel: string;
  actionVariant?: "primary" | "danger";
  onCancel: () => void;
  onAction: () => void;
}

export function Modal({
  title,
  children,
  cancelLabel,
  actionLabel,
  actionVariant = "primary",
  onCancel,
  onAction,
}: ModalProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const titleId = useId();

  useLayoutEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    dialog.showModal();
    cancelRef.current?.focus(); // initial focus on the safe button
    return () => dialog.close();
  }, []);

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby={titleId}
      onCancel={(event) => {
        event.preventDefault();
        onCancel();
      }}
      className="m-auto w-110 max-w-[90vw] rounded-xl bg-overlay p-5 text-fg shadow-3"
    >
      <h2 id={titleId} className="text-lg font-semibold">
        {title}
      </h2>
      <div className="mt-2 text-base text-fg-secondary">{children}</div>
      <div className="mt-5 flex justify-end gap-2">
        <Button ref={cancelRef} onClick={onCancel}>
          {cancelLabel}
        </Button>
        <Button variant={actionVariant} onClick={onAction}>
          {actionLabel}
        </Button>
      </div>
    </dialog>
  );
}

/* ---------- Page layout helpers ---------- */

export function PageTitle({ children }: { children: ReactNode }) {
  return <h1 className="font-display text-xl font-semibold text-fg">{children}</h1>;
}

export function Card({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <section className="rounded-lg border border-border bg-surface p-4 shadow-1">
      {title ? <h2 className="mb-3 text-lg font-semibold text-fg">{title}</h2> : null}
      {children}
    </section>
  );
}

interface SettingRowProps {
  label: ReactNode;
  hint?: ReactNode;
  control: ReactNode;
  below?: ReactNode;
}

export function SettingRow({ label, hint, control, below }: SettingRowProps) {
  return (
    <div className="border-b border-border-subtle py-3 first:pt-0 last:border-b-0 last:pb-0">
      <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2">
        <div className="min-w-0 flex-1 basis-60">
          <div className="text-md font-medium text-fg">{label}</div>
          {hint ? <div className="mt-0.5 text-sm text-fg-muted">{hint}</div> : null}
        </div>
        <div className="shrink-0">{control}</div>
      </div>
      {below ? <div className="mt-3">{below}</div> : null}
    </div>
  );
}
