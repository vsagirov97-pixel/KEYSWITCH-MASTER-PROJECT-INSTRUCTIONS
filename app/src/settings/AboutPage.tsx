import { useEffect, useState } from "react";
import { api, type AppInfo } from "../api";
import { Card, InlineAlert, PageTitle } from "../components/ui";
import { useT } from "../i18n";

export function AboutPage() {
  const t = useT();
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .getAppInfo()
      .then((value) => {
        if (alive) setInfo(value);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);

  return (
    <div className="flex flex-col gap-6">
      <PageTitle>{t("about.title")}</PageTitle>
      <Card title={t("app.name")}>
        <dl className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1 text-base">
          <dt className="text-fg-secondary">{t("about.version")}</dt>
          <dd className="font-mono text-fg tabular-nums">{info?.version ?? "—"}</dd>
          <dt className="text-fg-secondary">{t("about.coreVersion")}</dt>
          <dd className="font-mono text-fg tabular-nums">{info?.coreVersion ?? "—"}</dd>
        </dl>
        {info && !info.signed ? (
          <div className="mt-4">
            <InlineAlert tone="warning">{t("about.unsigned")}</InlineAlert>
          </div>
        ) : null}
      </Card>
      <Card title={t("about.licence.title")}>
        <p className="text-base text-fg-secondary">{t("about.licence.text")}</p>
      </Card>
      <Card title={t("about.thirdParty.title")}>
        <p className="text-base text-fg-secondary">{t("about.thirdParty.text")}</p>
        <p className="mt-2 text-base text-fg-secondary">{t("about.thirdParty.link")}</p>
      </Card>
    </div>
  );
}
