import { useState } from "react";
import type { Settings } from "../api";
import { cx } from "../components/ui";
import { useT, type MessageKey } from "../i18n";
import { AboutPage } from "./AboutPage";
import { ConversionPage } from "./ConversionPage";
import { ExcludedPage } from "./ExcludedPage";
import { GeneralPage } from "./GeneralPage";
import { MacroPage } from "./MacroPage";

type Section = "general" | "conversion" | "excluded" | "macro" | "about";

const SECTIONS: ReadonlyArray<{ id: Section; label: MessageKey }> = [
  { id: "general", label: "nav.general" },
  { id: "conversion", label: "nav.conversion" },
  { id: "excluded", label: "nav.excluded" },
  { id: "macro", label: "nav.macro" },
  { id: "about", label: "nav.about" },
];

/** Dev-only deep link for screenshots: `?section=macro` (ignored in production builds). */
function initialSection(): Section {
  if (!import.meta.env.DEV) return "general";
  const value = new URLSearchParams(window.location.search).get("section");
  const match = SECTIONS.find((entry) => entry.id === value);
  return match ? match.id : "general";
}

interface SettingsWindowProps {
  settings: Settings;
  onChange: (patch: Partial<Settings>) => void;
}

export function SettingsWindow({ settings, onChange }: SettingsWindowProps) {
  const t = useT();
  const [section, setSection] = useState<Section>(initialSection);
  // Unsaved-changes guard of the macro form: leaving asks Save / Don't save / Cancel.
  const [macroDirty, setMacroDirty] = useState(false);
  const [pendingSection, setPendingSection] = useState<Section | null>(null);

  function navigate(next: Section) {
    if (next === section) return;
    if (section === "macro" && macroDirty) {
      setPendingSection(next);
      return;
    }
    setSection(next);
  }

  return (
    <div className="flex h-full bg-canvas text-fg">
      <nav aria-label={t("nav.label")} className="flex w-56 shrink-0 flex-col gap-0.5 border-r border-border bg-surface p-3">
        <p className="px-2.5 pt-1 pb-3 font-display text-lg font-semibold">{t("app.name")}</p>
        {SECTIONS.map((entry) => {
          const active = entry.id === section;
          return (
            <button
              key={entry.id}
              type="button"
              aria-current={active ? "page" : undefined}
              onClick={() => navigate(entry.id)}
              className={cx(
                "relative flex min-h-8 items-center rounded-md px-2.5 py-1.5 text-left text-md font-medium -outline-offset-2 transition-colors duration-(--ks-dur-fast)",
                active ? "bg-selected text-fg" : "text-fg-secondary hover:bg-hover hover:text-fg",
              )}
            >
              {active ? <span aria-hidden="true" className="absolute top-1/2 left-0 h-4 w-0.5 -translate-y-1/2 rounded-full bg-accent" /> : null}
              {t(entry.label)}
            </button>
          );
        })}
      </nav>
      <main className="min-w-0 flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-260">
          {section === "general" ? <GeneralPage settings={settings} onChange={onChange} /> : null}
          {section === "conversion" ? <ConversionPage /> : null}
          {section === "excluded" ? <ExcludedPage /> : null}
          {section === "macro" ? (
            <MacroPage
              conversionHotkey={settings.hotkey}
              onDirtyChange={setMacroDirty}
              leaveRequested={pendingSection !== null}
              onLeave={() => {
                setMacroDirty(false);
                setSection(pendingSection ?? "macro");
                setPendingSection(null);
              }}
              onStay={() => setPendingSection(null)}
            />
          ) : null}
          {section === "about" ? <AboutPage /> : null}
        </div>
      </main>
    </div>
  );
}
