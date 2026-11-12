import { useState } from "react";
import { api, type Hotkey, type HotkeyResult, type Settings, type Theme, type UiLanguage } from "../api";
import { HotkeyCapture } from "../components/HotkeyCapture";
import { Card, InlineAlert, PageTitle, Segmented, SettingRow, Toggle } from "../components/ui";
import { useT } from "../i18n";

interface GeneralPageProps {
  settings: Settings;
  onChange: (patch: Partial<Settings>) => void;
}

type FailedSetting = "autostart" | "other" | null;

export function GeneralPage({ settings, onChange }: GeneralPageProps) {
  const t = useT();
  const [autostartPending, setAutostartPending] = useState(false);
  const [failed, setFailed] = useState<FailedSetting>(null);

  async function commitHotkey(hotkey: Hotkey): Promise<HotkeyResult> {
    const result = await api.setHotkey(hotkey);
    if (result.ok) onChange({ hotkey });
    return result;
  }

  // Applied at once (no Save button); on failure the control rolls back and an inline alert appears.
  async function changeAutostart(next: boolean) {
    setFailed(null);
    setAutostartPending(true);
    onChange({ autostart: next });
    try {
      await api.setAutostart(next);
    } catch {
      onChange({ autostart: !next });
      setFailed("autostart");
    } finally {
      setAutostartPending(false);
    }
  }

  async function changeLanguage(next: UiLanguage) {
    const previous = settings.uiLanguage;
    setFailed(null);
    onChange({ uiLanguage: next });
    try {
      await api.setUiLanguage(next);
    } catch {
      onChange({ uiLanguage: previous });
      setFailed("other");
    }
  }

  async function changeTheme(next: Theme) {
    const previous = settings.theme;
    setFailed(null);
    onChange({ theme: next });
    try {
      await api.setTheme(next);
    } catch {
      onChange({ theme: previous });
      setFailed("other");
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <PageTitle>{t("general.title")}</PageTitle>
      <Card>
        <SettingRow
          label={<label htmlFor="ks-hotkey">{t("general.hotkey.label")}</label>}
          hint={t("general.hotkey.hint")}
          control={
            <HotkeyCapture
              id="ks-hotkey"
              actionLabel={t("general.hotkey.label")}
              value={settings.hotkey}
              onCommit={commitHotkey}
            />
          }
        />
        <SettingRow
          label={<label htmlFor="ks-autostart">{t("general.autostart.label")}</label>}
          hint={<span id="ks-autostart-hint">{t("general.autostart.hint")}</span>}
          control={
            <Toggle
              id="ks-autostart"
              checked={settings.autostart}
              pending={autostartPending}
              describedBy="ks-autostart-hint"
              onChange={(next) => void changeAutostart(next)}
            />
          }
          below={
            failed === "autostart" ? (
              <InlineAlert tone="danger" role="alert">
                {t("general.autostart.error")}
              </InlineAlert>
            ) : null
          }
        />
        <SettingRow
          label={<label htmlFor="ks-language">{t("general.language.label")}</label>}
          control={
            <select
              id="ks-language"
              value={settings.uiLanguage}
              onChange={(event) => {
                const next = event.target.value;
                if (next === "ru" || next === "en") void changeLanguage(next);
              }}
              className="h-8 min-w-40 rounded-md border border-border-strong bg-raised px-3 text-base text-fg transition-colors duration-(--ks-dur-fast) hover:border-fg-muted focus:border-accent"
            >
              <option value="ru">{t("general.language.ru")}</option>
              <option value="en">{t("general.language.en")}</option>
            </select>
          }
        />
        <SettingRow
          label={t("general.theme.label")}
          control={
            <Segmented<Theme>
              label={t("general.theme.label")}
              value={settings.theme}
              onChange={(next) => void changeTheme(next)}
              options={[
                { value: "dark", label: t("general.theme.dark") },
                { value: "light", label: t("general.theme.light") },
                { value: "system", label: t("general.theme.system") },
              ]}
            />
          }
          below={
            failed === "other" ? (
              <InlineAlert tone="danger" role="alert">
                {t("general.saveError")}
              </InlineAlert>
            ) : null
          }
        />
      </Card>
    </div>
  );
}
