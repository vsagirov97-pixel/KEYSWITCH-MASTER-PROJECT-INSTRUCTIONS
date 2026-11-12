import { Card, Kbd, PageTitle } from "../components/ui";
import { useT, type MessageKey } from "../i18n";

const STEPS: ReadonlyArray<{ title: MessageKey; text: MessageKey }> = [
  { title: "conversion.behaviour.step1.title", text: "conversion.behaviour.step1.text" },
  { title: "conversion.behaviour.step2.title", text: "conversion.behaviour.step2.text" },
  { title: "conversion.behaviour.step3.title", text: "conversion.behaviour.step3.text" },
];

export function ConversionPage() {
  const t = useT();
  return (
    <div className="flex flex-col gap-6">
      <PageTitle>{t("conversion.title")}</PageTitle>
      <Card title={t("conversion.languages.title")}>
        <p className="flex items-center gap-2 text-md font-medium text-fg">
          <Kbd>EN</Kbd>
          <Kbd>RU</Kbd>
          {t("conversion.languages.pair")}
        </p>
        <p className="mt-2 text-sm text-fg-muted">{t("conversion.languages.hint")}</p>
      </Card>
      <Card title={t("conversion.behaviour.title")}>
        <p className="text-base text-fg-secondary">{t("conversion.behaviour.intro")}</p>
        <ol className="mt-3 flex flex-col gap-3">
          {STEPS.map((step, index) => (
            <li key={step.title} className="flex gap-3">
              <span
                aria-hidden="true"
                className="inline-flex size-6 shrink-0 items-center justify-center rounded-full bg-hover text-sm font-semibold text-fg-secondary tabular-nums"
              >
                {index + 1}
              </span>
              <div className="min-w-0">
                <p className="text-md font-medium text-fg">{t(step.title)}</p>
                <p className="text-base text-fg-secondary">{t(step.text)}</p>
              </div>
            </li>
          ))}
        </ol>
        <p className="mt-4 border-t border-border-subtle pt-3 text-sm text-fg-muted">{t("conversion.behaviour.mixed")}</p>
      </Card>
    </div>
  );
}
