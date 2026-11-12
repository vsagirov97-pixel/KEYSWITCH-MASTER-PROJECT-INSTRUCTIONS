import { Suspense, lazy, useEffect, useState } from "react";
import { api, type Settings, type UiLanguage } from "./api";
import { InlineAlert } from "./components/ui";
import { I18nProvider, useT } from "./i18n";
import { SettingsWindow } from "./settings/SettingsWindow";

// Dev-only component gallery; the dynamic import is dropped from production builds.
const PreviewPage = import.meta.env.DEV ? lazy(() => import("./PreviewPage")) : null;
const showPreview = PreviewPage !== null && new URLSearchParams(window.location.search).get("view") === "preview";

function fallbackLanguage(): UiLanguage {
  return navigator.language.toLowerCase().startsWith("ru") ? "ru" : "en";
}

/** "System" theme = prefers-color-scheme, mirrored into `data-sys` (design-system.md §1). */
function useSystemThemeAttribute() {
  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: light)");
    const apply = () => {
      document.documentElement.dataset.sys = query.matches ? "light" : "dark";
    };
    apply();
    query.addEventListener("change", apply);
    return () => query.removeEventListener("change", apply);
  }, []);
}

function Loading({ failed }: { failed: boolean }) {
  const t = useT();
  return (
    <main className="p-6">
      {failed ? (
        <InlineAlert tone="danger" role="alert">
          {t("app.loadError")}
        </InlineAlert>
      ) : (
        <p role="status" className="text-base text-fg-muted">
          {t("app.loading")}
        </p>
      )}
    </main>
  );
}

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [failed, setFailed] = useState(false);
  useSystemThemeAttribute();

  useEffect(() => {
    let alive = true;
    api
      .getSettings()
      .then((loaded) => {
        if (alive) setSettings(loaded);
      })
      .catch(() => {
        if (alive) setFailed(true);
      });
    return () => {
      alive = false;
    };
  }, []);

  const theme = settings?.theme;
  useEffect(() => {
    if (theme) document.documentElement.dataset.theme = theme;
  }, [theme]);

  return (
    <I18nProvider language={settings?.uiLanguage ?? fallbackLanguage()}>
      {settings === null ? (
        <Loading failed={failed} />
      ) : showPreview && PreviewPage ? (
        <Suspense fallback={null}>
          <PreviewPage />
        </Suspense>
      ) : (
        <SettingsWindow
          settings={settings}
          onChange={(patch) => setSettings((current) => (current ? { ...current, ...patch } : current))}
        />
      )}
    </I18nProvider>
  );
}
