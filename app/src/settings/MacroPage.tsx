// Macro Studio, Beta 2 step 1: one slot, text macros only (D-32: no code editor, no JS / Python).
// The macro text is the user's own data: it lives only in component state and goes only to
// `api.setMacroSlot`. Never log it, never put it into an error message or a URL.
import { useEffect, useId, useRef, useState, type FormEvent, type KeyboardEvent, type ReactNode } from "react";
import { MACRO_MAX_CHARS, api, type Hotkey, type MacroSlot, type MacroSlotError, type MacroSlotState } from "../api";
import { HotkeyCapture, type CaptureError, type CaptureResult } from "../components/HotkeyCapture";
import { Button, Card, IconAlert, InlineAlert, Modal, Pill, SettingRow, Spinner, Toggle, cx } from "../components/ui";
import { sameHotkey } from "../hotkey";
import { useT } from "../i18n";
import { charCount, contentProblem, formatCodePoint, nameInvalid, sameSlot, slotOf } from "../macro";

type ContentBackendError = "content_too_long" | "content_forbidden_chars" | "content_enter_tab_not_permitted";
type FormError = "type_unsupported" | "save_failed" | "failed" | "fix";

interface MacroPageProps {
  /** The conversion hotkey: the macro hotkey must differ from it. */
  conversionHotkey: Hotkey;
  onDirtyChange: (dirty: boolean) => void;
  /** The user picked another section while the form has unsaved changes. */
  leaveRequested: boolean;
  onLeave: () => void;
  onStay: () => void;
}

function isContentError(error: MacroSlotError): error is ContentBackendError {
  return (
    error === "content_too_long" || error === "content_forbidden_chars" || error === "content_enter_tab_not_permitted"
  );
}

export function MacroPage({ conversionHotkey, onDirtyChange, leaveRequested, onLeave, onStay }: MacroPageProps) {
  const t = useT();
  const nameHelpId = useId();
  const contentHelpId = useId();
  const counterId = useId();
  const enterTabHintId = useId();
  const nameRef = useRef<HTMLInputElement>(null);
  const contentRef = useRef<HTMLTextAreaElement>(null);

  const [saved, setSaved] = useState<MacroSlotState | null>(null);
  const [draft, setDraft] = useState<MacroSlot | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [saving, setSaving] = useState(false);
  const [hotkeyError, setHotkeyError] = useState<CaptureError | null>(null);
  const [nameBackendError, setNameBackendError] = useState(false);
  const [contentBackendError, setContentBackendError] = useState<ContentBackendError | null>(null);
  const [formError, setFormError] = useState<FormError | null>(null);
  // Errors of the live checks are shown once the field was edited or a save was attempted.
  const [showChecks, setShowChecks] = useState(false);

  useEffect(() => {
    let alive = true;
    api
      .getMacroSlot()
      .then((state) => {
        if (!alive) return;
        setSaved(state);
        setDraft(slotOf(state));
      })
      .catch(() => {
        if (alive) setLoadFailed(true);
      });
    return () => {
      alive = false;
    };
  }, []);

  const dirty = saved !== null && draft !== null && !sameSlot(draft, slotOf(saved));
  useEffect(() => {
    onDirtyChange(dirty);
  }, [dirty, onDirtyChange]);

  if (loadFailed) {
    return (
      <div className="flex flex-col gap-6">
        <Header title={t("macro.title")} />
        <InlineAlert tone="danger" role="alert">
          {t("macro.loadError")}
        </InlineAlert>
      </div>
    );
  }
  if (saved === null || draft === null) {
    return (
      <div className="flex flex-col gap-6">
        <Header title={t("macro.title")} />
        <p role="status" className="text-base text-fg-muted">
          {t("app.loading")}
        </p>
      </div>
    );
  }

  const current = draft;
  const baseline = saved;
  const nameError = nameBackendError || nameInvalid(current.name);
  const problem = contentProblem(current.content, current.allowEnterTab);
  const count = charCount(current.content);
  const contentErrorText: string | null = problem
    ? problem.kind === "forbidden"
      ? t("macro.content.error.forbidden", { code: formatCodePoint(problem.codePoint), position: problem.position })
      : t(`macro.content.error.${problem.kind}`, { max: MACRO_MAX_CHARS })
    : contentBackendError === "content_too_long"
      ? t("macro.content.error.too_long", { max: MACRO_MAX_CHARS })
      : contentBackendError === "content_forbidden_chars"
        ? t("macro.content.error.forbidden_backend")
        : contentBackendError === "content_enter_tab_not_permitted"
          ? t("macro.content.error.enter_tab")
          : null;
  const showNameError = nameError && (showChecks || nameBackendError);
  const showContentError = contentErrorText !== null && (showChecks || contentBackendError !== null);

  function edit(patch: Partial<MacroSlot>) {
    setDraft((previous) => (previous ? { ...previous, ...patch } : previous));
    setShowChecks(true);
    if (formError === "fix" || formError === "type_unsupported" || formError === "failed") setFormError(null);
  }

  async function reload() {
    try {
      const state = await api.getMacroSlot();
      setSaved(state);
      setDraft(slotOf(state));
    } catch {
      setLoadFailed(true);
    }
  }

  /** Returns `true` when the slot is active in the backend (saved, or active but not persisted). */
  async function save(): Promise<boolean> {
    setShowChecks(true);
    if (nameError || problem !== null || contentBackendError !== null || hotkeyError !== null) {
      setFormError("fix");
      if (nameError) nameRef.current?.focus();
      else if (problem !== null || contentBackendError !== null) contentRef.current?.focus();
      else document.getElementById("ks-macro-hotkey")?.focus();
      return false;
    }
    setSaving(true);
    setFormError(null);
    try {
      const result = await api.setMacroSlot(current);
      if (result.ok) {
        await reload();
        return true;
      }
      const error = result.error;
      if (error === "save_failed") {
        await reload();
        setFormError("save_failed");
        return true;
      }
      if (error === "invalid" || error === "reserved" || error === "conflict") {
        const duplicate = error === "conflict" && current.hotkey !== null && sameHotkey(current.hotkey, conversionHotkey);
        setHotkeyError(duplicate ? "duplicate" : error);
        document.getElementById("ks-macro-hotkey")?.focus();
      } else if (error === "name_invalid") {
        setNameBackendError(true);
        nameRef.current?.focus();
      } else if (isContentError(error)) {
        setContentBackendError(error);
        contentRef.current?.focus();
      } else {
        setFormError("type_unsupported");
      }
      return false;
    } catch {
      setFormError("failed");
      return false;
    } finally {
      setSaving(false);
    }
  }

  function discard() {
    setDraft(slotOf(baseline));
    setHotkeyError(null);
    setNameBackendError(false);
    setContentBackendError(null);
    setFormError(null);
    setShowChecks(false);
  }

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (dirty && !saving) await save();
  }

  function onFormKeyDown(event: KeyboardEvent<HTMLFormElement>) {
    if ((event.ctrlKey || event.metaKey) && !event.altKey && event.code === "KeyS") {
      event.preventDefault();
      if (dirty && !saving) void save();
    }
  }

  async function commitHotkey(hotkey: Hotkey): Promise<CaptureResult> {
    if (sameHotkey(hotkey, conversionHotkey)) return { ok: false, error: "duplicate" };
    edit({ hotkey });
    return { ok: true };
  }

  // Status next to the hotkey field (design-concept §4.3 states).
  const hotkeyDirty =
    current.hotkey === null || saved.hotkey === null
      ? current.hotkey !== saved.hotkey
      : !sameHotkey(current.hotkey, saved.hotkey);
  let hotkeyStatus: ReactNode = null;
  if (hotkeyDirty) hotkeyStatus = <Pill tone="neutral">{t("macro.hotkey.unsaved")}</Pill>;
  else if (saved.hotkey === null) hotkeyStatus = null;
  else if (!saved.enabled) hotkeyStatus = <Pill tone="neutral">{t("macro.hotkey.disabled")}</Pill>;
  else if (saved.registered) hotkeyStatus = <Pill tone="success">{t("hotkey.registered")}</Pill>;
  else hotkeyStatus = <Pill tone="danger">{t("macro.hotkey.notRegistered")}</Pill>;

  const attention = saved.enabled && saved.hotkey !== null && !saved.registered;
  const slotPill = attention ? (
    <Pill tone="danger">{t("macro.state.attention")}</Pill>
  ) : !saved.enabled ? (
    <Pill tone="neutral">{t("macro.state.off")}</Pill>
  ) : saved.hotkey === null ? (
    <Pill tone="warning">{t("macro.state.noHotkey")}</Pill>
  ) : (
    <Pill tone="success">{t("macro.state.on")}</Pill>
  );

  const formAlert =
    formError === "save_failed" ? (
      <InlineAlert tone="warning" role="alert">
        {t("macro.error.save_failed")}
      </InlineAlert>
    ) : formError ? (
      <InlineAlert tone="danger" role="alert">
        {t(`macro.error.${formError}`)}
      </InlineAlert>
    ) : null;

  return (
    <form noValidate onSubmit={(event) => void onSubmit(event)} onKeyDown={onFormKeyDown} className="flex flex-col gap-6">
      <Header
        title={t("macro.title")}
        status={
          <>
            {slotPill}
            {dirty ? (
              <span className="inline-flex items-center gap-1 text-sm text-fg-muted">
                <span aria-hidden="true" className="size-1.5 rounded-full bg-accent" />
                {t("macro.unsaved")}
              </span>
            ) : null}
          </>
        }
        actions={
          <>
            <Button onClick={discard} disabled={!dirty || saving}>
              {t("macro.cancel")}
            </Button>
            <Button type="submit" variant="primary" disabled={!dirty || saving} aria-busy={saving} className="min-w-28">
              {saving ? <Spinner /> : null}
              {saving ? t("macro.saving") : t("macro.save")}
            </Button>
          </>
        }
      />

      {formAlert}
      {attention && !hotkeyDirty ? (
        <InlineAlert tone="danger" title={t("macro.attention.title")}>
          {t("macro.attention.text")}
        </InlineAlert>
      ) : null}

      <Card>
        <SettingRow
          label={<label htmlFor="ks-macro-enabled">{t("macro.enabled.label")}</label>}
          hint={<span id="ks-macro-enabled-hint">{t("macro.enabled.hint")}</span>}
          control={
            <Toggle
              id="ks-macro-enabled"
              checked={current.enabled}
              describedBy="ks-macro-enabled-hint"
              onChange={(enabled) => edit({ enabled })}
            />
          }
        />
        <SettingRow
          label={<label htmlFor="ks-macro-name">{t("macro.name.label")}</label>}
          hint={
            <span id={nameHelpId} role={showNameError ? "alert" : undefined} className={showNameError ? "text-danger" : undefined}>
              {showNameError ? t("macro.name.error") : t("macro.name.hint")}
            </span>
          }
          control={
            <div className="relative w-60">
              <input
                id="ks-macro-name"
                ref={nameRef}
                type="text"
                value={current.name}
                onChange={(event) => {
                  setNameBackendError(false);
                  edit({ name: event.target.value });
                }}
                placeholder={t("macro.name.placeholder")}
                autoComplete="off"
                spellCheck={false}
                aria-invalid={showNameError}
                aria-describedby={nameHelpId}
                className={cx(
                  "h-8 w-full rounded-md border bg-raised px-3 text-base text-fg transition-colors duration-(--ks-dur-fast) placeholder:text-fg-muted focus:border-accent",
                  showNameError ? "border-danger pr-8" : "border-border-strong hover:border-fg-muted",
                )}
              />
              {showNameError ? <IconAlert className="absolute top-2 right-2 text-danger" /> : null}
            </div>
          }
        />
        <SettingRow
          label={<label htmlFor="ks-macro-hotkey">{t("macro.hotkey.label")}</label>}
          hint={
            <span id="ks-macro-hotkey-hint">
              {t("macro.hotkey.hint")}
              {current.enabled && current.hotkey === null ? ` ${t("macro.hotkey.missing")}` : null}
            </span>
          }
          control={
            <HotkeyCapture
              id="ks-macro-hotkey"
              actionLabel={t("macro.hotkey.label")}
              otherActionLabel={t("general.hotkey.label")}
              value={current.hotkey}
              onCommit={commitHotkey}
              onClear={() => edit({ hotkey: null })}
              error={hotkeyError}
              onErrorReset={() => setHotkeyError(null)}
              status={hotkeyStatus}
              describedBy="ks-macro-hotkey-hint"
            />
          }
        />
      </Card>

      <Card title={t("macro.content.title")}>
        <p className="text-base text-fg-secondary">{t("macro.content.intro")}</p>
        <div className="mt-3">
          <InlineAlert tone="warning">{t("macro.content.warning")}</InlineAlert>
        </div>

        <div className="mt-4">
          <label htmlFor="ks-macro-content" className="text-sm font-semibold text-fg-secondary">
            {t("macro.content.label")}
          </label>
          <div className="relative mt-1">
            <textarea
              id="ks-macro-content"
              ref={contentRef}
              value={current.content}
              onChange={(event) => {
                setContentBackendError(null);
                edit({ content: event.target.value });
              }}
              rows={8}
              autoComplete="off"
              spellCheck={false}
              aria-invalid={showContentError}
              aria-describedby={`${contentHelpId} ${counterId}`}
              className={cx(
                "block min-h-24 w-full resize-y rounded-md border bg-raised px-3 py-1.5 text-base whitespace-pre-wrap text-fg transition-colors duration-(--ks-dur-fast) focus:border-accent",
                showContentError ? "border-danger pr-8" : "border-border-strong hover:border-fg-muted",
              )}
            />
            {showContentError ? <IconAlert className="absolute top-2 right-2 text-danger" /> : null}
          </div>
          <div className="mt-1 flex items-start justify-between gap-4">
            <p
              id={contentHelpId}
              role={showContentError ? "alert" : undefined}
              className={cx("min-w-0 text-sm", showContentError ? "text-danger" : "text-fg-muted")}
            >
              {showContentError ? contentErrorText : t("macro.content.hint", { max: MACRO_MAX_CHARS })}
            </p>
            <p
              className={cx(
                "shrink-0 text-sm tabular-nums",
                count > MACRO_MAX_CHARS ? "font-semibold text-danger" : "text-fg-muted",
              )}
            >
              <span aria-hidden="true">
                {count} / {MACRO_MAX_CHARS}
              </span>
              <span id={counterId} className="sr-only">
                {t("macro.content.counter", { count, max: MACRO_MAX_CHARS })}
              </span>
            </p>
          </div>
        </div>

        <div className="mt-4 flex gap-3 border-t border-border-subtle pt-4">
          <input
            id="ks-macro-enter-tab"
            type="checkbox"
            checked={current.allowEnterTab}
            onChange={(event) => {
              setContentBackendError(null);
              edit({ allowEnterTab: event.target.checked });
            }}
            aria-describedby={enterTabHintId}
            className="mt-0.5 size-4 shrink-0 rounded-sm accent-accent"
          />
          <div className="min-w-0">
            <label htmlFor="ks-macro-enter-tab" className="text-md font-medium text-fg">
              {t("macro.enterTab.label")}
            </label>
            <p id={enterTabHintId} className="mt-0.5 text-sm text-fg-muted">
              {t("macro.enterTab.hint")}
            </p>
          </div>
        </div>
      </Card>

      {leaveRequested ? (
        <Modal
          title={t("macro.leave.title")}
          cancelLabel={t("common.cancel")}
          extraLabel={t("macro.leave.discard")}
          onExtra={onLeave}
          actionLabel={t("macro.leave.save")}
          onCancel={onStay}
          onAction={() => {
            void save().then((ok) => (ok ? onLeave() : onStay()));
          }}
        >
          {t("macro.leave.text")}
        </Modal>
      ) : null}
    </form>
  );
}

function Header({ title, status, actions }: { title: string; status?: ReactNode; actions?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <h1 className="font-display text-xl font-semibold text-fg">{title}</h1>
        {status}
      </div>
      {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
    </div>
  );
}
