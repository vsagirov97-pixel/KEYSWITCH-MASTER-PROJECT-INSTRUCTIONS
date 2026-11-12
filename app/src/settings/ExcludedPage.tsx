import { useEffect, useRef, useState, type FormEvent } from "react";
import { api, type ExcludedApp } from "../api";
import { Button, Card, IconAlert, IconLock, IconX, InlineAlert, Modal, PageTitle, cx } from "../components/ui";
import { useT, type MessageKey } from "../i18n";

type AddError = "empty" | "format" | "duplicate";

// A bare executable name: no path separators or other characters Windows forbids in file names.
const EXE_NAME = /^[^\\/:*?"<>|\s][^\\/:*?"<>|]*\.exe$/i;

const ALWAYS_EXCLUDED: readonly MessageKey[] = [
  "excluded.always.password",
  "excluded.always.elevated",
  "excluded.always.secureDesktop",
];

export function ExcludedPage() {
  const t = useT();
  const inputRef = useRef<HTMLInputElement>(null);
  const [apps, setApps] = useState<ExcludedApp[] | null>(null);
  const [draft, setDraft] = useState("");
  const [addError, setAddError] = useState<AddError | null>(null);
  const [failed, setFailed] = useState(false);
  const [confirming, setConfirming] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .listExcludedApps()
      .then((list) => {
        if (alive) setApps(list);
      })
      .catch(() => {
        if (alive) setFailed(true);
      });
    return () => {
      alive = false;
    };
  }, []);

  async function add(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setFailed(false);
    const name = draft.trim().toLowerCase();
    const problem: AddError | null =
      name === "" ? "empty" : !EXE_NAME.test(name) ? "format" : apps?.some((app) => app.exe === name) ? "duplicate" : null;
    setAddError(problem);
    if (problem) {
      inputRef.current?.focus();
      return;
    }
    try {
      setApps(await api.addExcludedApp(name));
      setDraft("");
    } catch {
      setFailed(true);
    }
  }

  async function remove(exe: string) {
    setConfirming(null);
    setFailed(false);
    try {
      setApps(await api.removeExcludedApp(exe));
    } catch {
      setFailed(true);
    }
    inputRef.current?.focus(); // the row (and its button) is gone; keep focus somewhere sensible
  }

  return (
    <div className="flex flex-col gap-6">
      <PageTitle>{t("excluded.title")}</PageTitle>
      <Card>
        <p className="text-base text-fg-secondary">{t("excluded.intro")}</p>

        <form onSubmit={(event) => void add(event)} noValidate className="mt-4">
          <label htmlFor="ks-exclude-input" className="text-sm font-semibold text-fg-secondary">
            {t("excluded.add.label")}
          </label>
          <div className="mt-1 flex flex-wrap items-center gap-2">
            <div className="relative min-w-50 flex-1">
              <input
                id="ks-exclude-input"
                ref={inputRef}
                type="text"
                value={draft}
                onChange={(event) => {
                  setDraft(event.target.value);
                  setAddError(null);
                }}
                placeholder={t("excluded.add.placeholder")}
                autoComplete="off"
                spellCheck={false}
                aria-invalid={addError !== null}
                aria-describedby="ks-exclude-help"
                className={cx(
                  "h-8 w-full rounded-md border bg-raised px-3 font-mono text-base text-fg transition-colors duration-(--ks-dur-fast) placeholder:font-sans placeholder:text-fg-muted focus:border-accent",
                  addError ? "border-danger pr-8" : "border-border-strong hover:border-fg-muted",
                )}
              />
              {addError ? <IconAlert className="absolute top-2 right-2 text-danger" /> : null}
            </div>
            <Button type="submit" variant="primary">
              {t("excluded.add.button")}
            </Button>
          </div>
          <p id="ks-exclude-help" role={addError ? "alert" : undefined} className={cx("mt-1 text-sm", addError ? "text-danger" : "text-fg-muted")}>
            {addError ? t(`excluded.add.error.${addError}`) : t("excluded.add.hint")}
          </p>
        </form>

        {failed ? (
          <div className="mt-3">
            <InlineAlert tone="danger" role="alert">
              {t("excluded.error")}
            </InlineAlert>
          </div>
        ) : null}

        <div className="mt-4 overflow-x-auto rounded-md border border-border">
          <table className="w-full border-collapse text-left text-base">
            <thead>
              <tr className="h-8 border-b border-border bg-surface text-sm font-semibold text-fg-muted">
                <th scope="col" className="px-3 font-semibold">
                  {t("excluded.col.app")}
                </th>
                <th scope="col" className="px-3 font-semibold">
                  {t("excluded.col.source")}
                </th>
                <th scope="col" className="px-3">
                  <span className="sr-only">{t("excluded.col.actions")}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {(apps ?? []).map((app) => (
                <tr key={app.exe} className="h-9 border-b border-border-subtle last:border-b-0 hover:bg-hover">
                  <td className="px-3 font-mono text-fg">{app.exe}</td>
                  <td className="px-3 text-fg-secondary">{t(`excluded.source.${app.source}`)}</td>
                  <td className="px-3 text-right">
                    {app.source === "user" ? (
                      <button
                        type="button"
                        aria-label={t("excluded.remove.aria", { exe: app.exe })}
                        title={t("excluded.remove")}
                        onClick={() => setConfirming(app.exe)}
                        className="inline-flex size-6 items-center justify-center rounded-md align-middle text-fg-secondary transition-colors duration-(--ks-dur-fast) hover:bg-overlay hover:text-fg"
                      >
                        <IconX />
                      </button>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>

      <Card title={t("excluded.always.title")}>
        <ul className="flex flex-col gap-2">
          {ALWAYS_EXCLUDED.map((key) => (
            <li key={key} className="flex items-center gap-2 text-base text-fg-secondary">
              <IconLock className="text-fg-muted" />
              <span>{t(key)}</span>
              <span className="sr-only">{t("excluded.always.locked")}</span>
            </li>
          ))}
        </ul>
      </Card>

      {confirming !== null ? (
        <Modal
          title={t("excluded.confirm.title")}
          cancelLabel={t("common.cancel")}
          actionLabel={t("excluded.confirm.action")}
          actionVariant="danger"
          onCancel={() => setConfirming(null)}
          onAction={() => void remove(confirming)}
        >
          {t("excluded.confirm.text", { exe: confirming })}
        </Modal>
      ) : null}
    </div>
  );
}
