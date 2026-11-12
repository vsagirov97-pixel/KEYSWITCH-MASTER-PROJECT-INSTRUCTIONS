// Dev-only gallery (`npm run dev`, then /?view=preview) for pieces that have no place in the
// settings window: language picker, operation status, toast, tray menu model.
// Loaded only when `import.meta.env.DEV` is true, so it is not part of the production bundle.
import { useState } from "react";
import type { Hotkey, OperationOutcome } from "./api";
import { LanguagePicker } from "./components/LanguagePicker";
import { OperationStatus, Toast } from "./components/OperationStatus";
import { Button, Card, PageTitle } from "./components/ui";
import { useT } from "./i18n";
import { buildTrayMenu, type TrayItem, type TrayState } from "./tray/model";

const OUTCOMES: readonly OperationOutcome[] = [
  { kind: "converted" },
  { kind: "nothing" },
  { kind: "refused", reason: "PASSWORD" },
  { kind: "refused", reason: "ELEVATED" },
  { kind: "refused", reason: "EXCLUDED_APP" },
  { kind: "refused", reason: "LOW_CONFIDENCE" },
];

const HOTKEY: Hotkey = { modifiers: ["Ctrl", "Shift"], key: "Space" };
const TRAY_STATES: readonly TrayState[] = [
  { status: "active", pausedUntil: null, hotkey: HOTKEY },
  { status: "paused", pausedUntil: "14:30", hotkey: HOTKEY },
];

function TrayItems({ items }: { items: readonly TrayItem[] }) {
  return (
    <ul className="flex flex-col gap-0.5">
      {items.map((entry, index) => {
        if (entry.kind === "separator") return <li key={index} role="separator" className="my-1 border-t border-border-subtle" />;
        if (entry.kind === "info") {
          return (
            <li key={index} className="px-2 text-sm text-fg-muted">
              {entry.label}
            </li>
          );
        }
        if (entry.kind === "submenu") {
          return (
            <li key={index} className="px-2">
              {entry.label}
              <div className="pl-4">
                <TrayItems items={entry.items} />
              </div>
            </li>
          );
        }
        return (
          <li key={index} className={entry.isDefault ? "px-2 font-semibold" : "px-2"}>
            {entry.label}
          </li>
        );
      })}
    </ul>
  );
}

export default function PreviewPage() {
  const t = useT();
  const [pickerKey, setPickerKey] = useState(0);
  const [toast, setToast] = useState(true);

  return (
    <main className="mx-auto flex max-w-180 flex-col gap-6 p-6">
      <PageTitle>{t("preview.title")}</PageTitle>
      <Card title={t("preview.picker")}>
        <LanguagePicker key={pickerKey} onChoose={() => setPickerKey((k) => k + 1)} onCancel={() => setPickerKey((k) => k + 1)} />
      </Card>
      <Card title={t("preview.status")}>
        <div className="flex flex-col gap-2">
          {OUTCOMES.map((outcome, index) => (
            <OperationStatus key={index} outcome={outcome} />
          ))}
        </div>
      </Card>
      <Card title={t("preview.tray")}>
        <div className="grid grid-cols-2 gap-4">
          {TRAY_STATES.map((state) => {
            const menu = buildTrayMenu(state, t);
            return (
              <div key={state.status} className="rounded-lg bg-overlay p-2 text-base shadow-2">
                <p className="px-2 pb-2 text-sm text-fg-muted">{menu.tooltip}</p>
                <TrayItems items={menu.items} />
              </div>
            );
          })}
        </div>
      </Card>
      <Card title={t("preview.toast")}>
        <Button onClick={() => setToast(true)}>{t("preview.toast")}</Button>
      </Card>
      {toast ? (
        <Toast title={t("preview.toast.title")} onClose={() => setToast(false)}>
          {t("preview.toast.text")}
        </Toast>
      ) : null}
    </main>
  );
}
