# MARKET_OVERVIEW — обзор рынка KeySwitch

Автор: AGENT 10 Market & Competitive Intelligence Analyst · Дата: 2026-11-10 · Директива: V4 (§4–6 `docs/master-instructions-v4.md`)
Статус: **исследование, не решение.** Состав платных функций, тарифы и цены утверждает только владелец по `COMMERCIAL_PRODUCT_PROPOSAL.md` (V4 п.5).
Опирается на: `docs/market/2026-11-10-competitive-landscape.md`, `docs/market/2026-11-10-user-needs.md`, `docs/market/ideas-backlog.md` (методика и первичные источники там; здесь не дублируются).
B2B/Enterprise-рынок — зона AGENT 17 (`B2B_REQUIREMENTS`, `BUSINESS_FEATURE_RESEARCH`, `ENTERPRISE_FEATURE_RESEARCH`); здесь только ссылки.

## 0. Маркировка и ограничения

- **[V]** — verified: первичный источник прочитан аналитиком 2026-11-10.
- **[R]** — reported: сводка поисковой выдачи / вторичный источник (СМИ, каталоги, агрегаторы); страница целиком не открыта.
- **[A]** — assumption: оценка или вывод аналитика.
- Все обращения к источникам — **2026-11-10**, если не указано иное. Цифры по датам — те, что указаны в источнике.
- **Доступность.** `WebFetch` из облачной сессии: `caramba-switcher.com` — ошибка DNS (вне разрешённых доменов), то же ранее для `yandex.ru`, `habr.com`, `alternativeto.net`, `apps.apple.com`, `github.com` (403). Открывается `raw.githubusercontent.com`. Поэтому почти все рыночные цифры — **[R]**. Для повышения доказательности нужно разрешить: `gs.statcounter.com`, `ec.europa.eu`, `caramba-switcher.com`, `yandex.ru`, `apps.microsoft.com`, `apps.apple.com`, `paddle.com`, `eur-lex.europa.eu`.

## 1. Главное (TL;DR)

1. **Прямого измерения размера рынка «исправителей раскладки» не существует** в доступных источниках: ни Яндекс (Punto), ни Caramba не публикуют число пользователей; счётчики скачиваний каталогов найти не удалось. Размер оцениваем только через **косвенные сигналы** (носители языков с кириллицей × доля Windows) — это **порядок величины, а не TAM** [A].
2. **Windows доминирует в целевых регионах:** в России Windows ≈ 80–91 % десктопного трафика (StatCounter, май–июль 2026; страницы противоречат друг другу) [R]. Среди Windows-ПК в мире Windows 11 ≈ 69 %, Windows 10 ≈ 30 % (июль 2026) [R]; в Европе Windows 11 ≈ 70 %, Windows 10 ≈ 27 % (май 2026) [R]. Решение V4 «Windows 11 — основная, Windows 10 — дополнительная» соответствует рынку.
3. **Windows 10 — значимый «хвост»:** окончание поддержки 2025-10-14; бесплатные ESU для потребителей в ЕЭЗ — до 13 (или 14) октября 2026 г. [R]. То есть **с сегодняшнего дня (2026-11-10) домашние Windows 10 в ЕС, по сообщениям, без обновлений безопасности**, но доля Windows 10 всё ещё ~27–30 % [R]. Для продукта это вопрос политики поддержки и безопасности (CISO), а не только рынка.
4. **Аудитория шире «русскоязычных в России»:** ~258 млн носителей русского всего, ~147–150 млн как родного [R, Ethnologue через вторичные]; **4,43 млн украинцев под временной защитой в ЕС** (июль 2026, Eurostat через СМИ) [R], в т. ч. >1,28 млн в Германии и >0,96 млн в Польше [R]; в Германии ~2,5 млн используют русский дома [R]. Это пользователи, у которых одновременно 2–3 раскладки (UK/RU/EN/DE/PL) — сильный сигнал для языковых пакетов (MI-17) [A].
5. **Тренды, работающие на позиционирование KeySwitch:** (а) недоверие к «ИИ, который смотрит экран» — Signal, Brave, AdGuard блокируют Windows Recall (2025) [R]; (б) Grammarly и аналоги воспринимаются ИБ как «кейлоггеры», по сообщениям — запреты в крупных организациях [R]; (в) ИИ-функции письма приходят в саму ОС (Writing Tools в Notepad, требуют учётной записи Microsoft) [R] — это и конкурент «умного» редактирования, и контраст «облако vs локально».
6. **Каналы:** Microsoft Store стал бесплатен для индивидуальных разработчиков (сентябрь 2025), комиссия 15 % для приложений или **0 % при собственной коммерции** (не игры) [R]; ближайший OSS-конкурент Kright уже в Microsoft Store [V]; Caramba — в winget-каталоге (Wingetly) [R].
7. **Регуляторика ЕС делает монетизацию «взрослой»:** обязанности по CRA (отчётность об эксплуатируемых уязвимостях) действуют с **2026-09-11**, основные требования — с **2027-12-11** [R]; по рецесивам CRA неденежный OSS вне сферы, а **монетизированный продукт (в т. ч. open core) — вероятно в сфере** [R]. Это прямо влияет на стоимость Premium → передать CLO + CISO.

## 2. Сигналы размера рынка

| Сигнал | Значение | Дата данных | Метка / источник |
|---|---|---|---|
| Носители русского (всего / родной) | ~258 млн / ~147–150 млн | Ethnologue 2024 (через вторичные) | [R] https://www.babbel.com/en/magazine/how-many-people-speak-russian-and-where-spoken ; https://www.lingoda.com/blog/en/how-many-people-really-speak-russian/ |
| Украинцы под временной защитой в ЕС | 4,43 млн (DE >1,28 млн, PL >0,96 млн, CZ >0,39 млн) | конец июля 2026 | [R] https://www.eunews.it/en/2026/09/10/eurostat-4-43-million-ukrainians-under-temporary-protection-in-the-eu-in-july-2026/ ; https://ec.europa.eu/eurostat/web/products-eurostat-news/w/ddn-20260310-1 (январь 2026: 4,38 млн) |
| Используют русский дома в Германии | ~2,5 млн (≈3 % населения); выходцев из бывшего СССР ~4,7 млн | 2024 | [R] https://www.posle.media/article/the-russian-speaking-left-in-german-politics (издание признано в РФ «иноагентом» — использовать осторожно); bpb предупреждает, что популярные оценки завышены: https://www.bpb.de/apuz/243862/ |
| Доля Windows на десктопе, Россия | 80,7–91,1 % (разные страницы) | май–июль 2026 | [R] https://gs.statcounter.com/os-market-share/desktop/russian-federation |
| Windows 11 / Windows 10 среди Windows, мир | 68,93 % / 29,83 % | июль 2026 | [R] https://gs.statcounter.com/windows-version-market-share/desktop/ |
| То же, Европа | 70,17 % / 26,88 % | май 2026 | [R] https://gs.statcounter.com/windows-version-market-share/desktop/europe/ |
| То же, США | 78,95 % / 19,25 % | июнь 2026 | [R] https://gs.statcounter.com/windows-version-market-share/desktop/united-states-of-america/ |
| Рост и откат Windows 11 | 72,8 % в феврале 2026 → ~69 % в июле | 2026 | [R] https://windowsreport.com/?p=1489041 ; https://www.windowscentral.com/microsoft/windows-11/windows-11s-growth-has-officially-hit-a-brick-wall-and-users-appear-to-be-fleeing-back-to-windows-10 |
| Установки Punto / Caramba | **не найдено** | — | Пробел. Яндекс и Caramba не публикуют; softportal и аналоги не открываются |
| Установки OSS-конкурентов | **не установлено** (`github.com` 403 — нет счётчиков релизов) | — | Пробел |
| PowerToys (ориентир «утилиты Windows») | «>15 млн скачиваний» (маркетинговая страница подрядчика); Chocolatey ~2,2 млн | неизвестно / 2024 | [R] https://janeasystems.com/client-success/microsoft-powertoys ; https://www.chocolatey.org/packages/powertoys — слабый сигнал |

**Оценка порядка величины [A] (только для обсуждения).** Если взять ~150 млн носителей русского как родного + ~4–5 млн украиноязычных в ЕС, ~80 % пользователей ПК на Windows, и предположить, что регулярно печатают на двух раскладках и сталкиваются с ошибкой раскладки хотя бы десятки миллионов человек, то *активный* рынок переключателей — это, вероятно, единицы–десятки миллионов установок исторически (Punto существует с 2001 г. [R: https://3dnews.ru/565834]). Это **не проверено**; для бизнес-плана нужна первичная проверка (см. §7).

## 3. Сегменты (B2C)

Подробно — `B2C_USER_NEEDS.md`. Сводно:

| Сегмент | Почему важен | Чувствительность к цене [A] | Сила сигнала |
|---|---|---|---|
| Обычные двуязычные пользователи RU/EN | Массовая база Punto/Caramba | Высокая (ожидают бесплатно) | Средняя [R] |
| Многоязычные в ЕС (UK/RU/EN + DE/PL/CZ) | 3+ раскладки, недоверие к продуктам Яндекса [A] | Средняя | Средняя (Eurostat [R]) |
| Разработчики и IT | Терминалы, vim, RDP; ценят open source и проверяемость | Средняя; платят за инструменты (Sublime $99, Typora $14,99 [R]) | Средняя |
| Офисные сотрудники | Мессенджеры, почта, CRM; ограничения ИБ | Низкая лично, решает работодатель (AGENT 17) | Слабая |
| «Тяжёлые» пользователи текста (авторы, переводчики, поддержка) | Шаблоны, трансформации текста; платят за экспандеры | Низкая–средняя (TextExpander ~$40/год [R]) | Средняя (косвенно) |
| Студенты/преподаватели | Многоязычные тексты; бюджет ограничен | Очень высокая | Слабая [A] |

## 4. Тренды

| Тренд | Факты | Вывод для KeySwitch [A] |
|---|---|---|
| **Windows 11 как стандарт, Windows 10 как хвост** | См. §2; ESU для дома в ЕЭЗ — до 13/14.10.2026 [R: https://www.helpnetsecurity.com/2025/09/25/windows-10-extended-security-updates-europe/ ; https://evezone.evetech.co.za/daily-drop/free-windows-10-security-updates-end-in-october-2026-here-s-the-deadline] | Тестировать в первую очередь на 11; для Windows 10 — явная политика «поддерживается, пока ...» (MI-12). Платные функции не должны зависеть от Windows 10. |
| **Приватность и ИИ на ПК** | Signal (май 2025) блокирует Recall через DRM-флаг окна; Brave 1.81 и AdGuard блокируют Recall по умолчанию [R: https://www.techspot.com/news/108817-privacy-apps-signal-brave-adguard-push-back-against.html ; https://www.theregister.com/2025/07/23/brave_browse_block_microsoft_recall/] | «Приложения защищают пользователя от ОС» — признанный рынком нарратив. Идея: окно KeySwitch (Macro Studio с кодом/текстом) исключить из захвата экрана (`SetWindowDisplayAffinity`) — **новая идея MI-23, оценивают CISO/ARCH/DES**. |
| **Ассистенты письма = «кейлоггеры» в глазах ИБ** | Сообщения о запрете Grammarly в Microsoft; Grammarly заявляет, что не пишет все нажатия, блокирует пароли «best effort»; Enterprise-версия продаёт SSO/DLP/аудит [R: https://talk.macpowerusers.com/t/microsoft-bans-grammarly-because-of-security-concerns/12723 ; https://dope.security/post/is-grammarly-safe-for-work] | Подтверждает H-1 «не слушает клавиатуру». B2B-сторону — AGENT 17. |
| **ИИ-редактирование встраивается в ОС** | Notepad: Write/Rewrite/Summarize, переименовано в Writing Tools (11.2512.28.0), нужна учётная запись Microsoft; планы «rewrite в любом поле» в 2026 — **не подтверждено, что выпущено** [R: https://www.elevenforum.com/t/use-rewrite-in-notepad-app-to-enhance-writing-with-ai-in-windows-11.30364/latest ; https://www.windowscentral.com/microsoft/windows-11/windows-11-is-gaining-a-powerful-set-of-new-ai-productivity-features-in-2026-heres-what-to-expect]. PowerToys Advanced Paste — локальные модели (Foundry Local/Ollama, Phi Silica) [R, см. competitive-landscape §3] | Конкурировать с ИИ-переписыванием не нужно; ниша — **детерминированные, предсказуемые** трансформации без облака. Локальный ИИ — только как далёкая опция (P-14 в PREMIUM_FEATURE_RESEARCH). |
| **OSS-волна переключателей 2025–2026** | Kright, RuSwitcher, SimpleSwitcher, LangSwitcher [V, competitive-landscape §2] | Цена базовой функции на рынке = 0. Монетизировать только надстройку. |
| **Конфликт имени** | На Product Hunt есть «KeySwitch» того же назначения [R] | Блокер коммерции до решения владельца (MI-20, `docs/legal/name-and-ip-review.md`). |

## 5. Каналы дистрибуции

| Канал | Факты (дата) | Плюсы | Минусы / вопросы | Метка |
|---|---|---|---|---|
| **Microsoft Store** | Регистрация индивидуального разработчика — бесплатно (с 09.2025, >200 рынков); компании — $99 разово; комиссия 15 % (приложения) / 12 % (игры); для не-игр можно **своя коммерция с 0 %** | Доверие, SmartScreen не мешает, автообновление средствами ОС (без сети в нашем процессе [A]) | Требования MSIX/Win32-листинга, правила Store; лицензирование через Store API — оценить, не создаёт ли сетевых вызовов из нашего процесса (CISO) | [R] https://blogs.windows.com/windowsdeveloper/2025/09/10/free-developer-registration-for-individual-developers-on-microsoft-store/ ; https://www.windowscentral.com/microsoft/windows-11/microsoft-store-drops-fees-for-individual-developers-apple-still-charges-usd99-per-year/ |
| **winget** | Каталог содержит Caramba Switcher (+Pro), Rust Switcher, thaikeyfix и др. | Привычен IT/разработчикам; удобен для массовой установки (B2B — AGENT 17) | Манифест указывает на наш URL загрузки; подпись (D-11) | [R] https://wingetly.io/apps/sergey-moskalev/caramba-switcher ; https://wingetly.io/tags/keyboard-layout |
| **Прямая загрузка (GitHub Releases / сайт)** | Kright: и GitHub, и Store — «тот же подписанный установщик» | Полный контроль; проверяемость (MI-03) | SmartScreen для неподписанных (D-11); платёж — отдельно | [V] https://raw.githubusercontent.com/walteryaron/kright/HEAD/README.md |
| **Региональные реселлеры (RU)** | Caramba продаётся через Allsoft (499 ₽/год) [R, competitive-landscape] | Оплата российскими картами | Санкции/юр. риски для продавца из ЕС — **CLO** | [R] |
| **Каталоги/обзоры** (Product Hunt, AlternativeTo, Habr, Lifehacker) | Основной источник трафика OSS-конкурентов [A] | Дёшево | Имя «KeySwitch» уже занято на PH | [R]/[A] |

## 6. Региональные заметки: дистрибуция в ЕС (предварительно, **для CLO**)

1. **CRA (Regulation (EU) 2024/2847).** Отчётность производителей об активно эксплуатируемых уязвимостях и серьёзных инцидентах — с 2026-09-11 (первое уведомление в течение 24 ч, по комментариям юрфирм); основные требования и оценка соответствия — с 2027-12-11; руководство Комиссии опубликовано 2026-07-27 [R: https://www.mccannfitzgerald.com/knowledge/data-privacy-and-cyber-risk/cyber-resilience-act-reporting-obligations-apply-from-11-september-2026 ; https://www.jonesday.com/en/insights/2026/07/eu-cyber-resilience-act-24hour-reporting-duties-start-september-11-2026]. Немонетизированный FOSS — не коммерческая деятельность; **open core вероятно в сфере** [R: https://digital-strategy.ec.europa.eu/en/policies/cra-open-source ; https://presencis.com/regulations/cra/exemptions/open-source]. **Вывод [A]:** решение о Premium = решение о статусе «производителя» по CRA с процессами уязвимостей, SBOM, сроком поддержки. Учесть в стоимости.
2. **НДС на цифровые услуги.** Для B2C в ЕС — НДС страны покупателя (OSS-режим) [A, общеизвестно; CLO подтвердить]. Посредник Merchant of Record (Paddle/FastSpring/Lemon Squeezy) берёт это на себя [R: https://dev.to/jettfu/stripe-vs-paddle-vs-lemon-squeezy-fee-comparison-2026-2c77].
3. **Право потребителя на отказ** от цифрового контента (14 дней, с возможностью отказа от права при немедленной поставке с согласия) — [A, требует CLO]. Влияет на формат «пробный период / возврат».
4. **GDPR.** Даже офлайн-лицензия с e-mail/именем — персональные данные у продавца; в приложении — минимизировать (лицензионный ID вместо e-mail) [A → CLO]. Телеметрии нет — это упрощает.
5. **Санкции и платежи из РФ/РБ.** Paddle блокирует транзакции из России и Беларуси на уровне оформления заказа, подписки покупателей из РФ/РБ отменены с апреля 2023 [R: https://paddle.com/help/legal/sanctions/impact-of-sanctions-on-russia-and-belarus]. Запрет ЕС ст. 5n Reg. 833/2014 касается ПО для управления предприятиями и проектирования (Annex XXXIX) [R: https://finance.ec.europa.eu/system/files/2024-02/faqs-sanctions-russia-software_en%2C.pdf]; потребительской утилиты прямо не касается, но **это не подтверждено** — CLO. **Вывод [A]:** крупнейшая языковая аудитория (Россия) — наименее доступна для платежей из ЕС; Free для неё — единственный реалистичный путь, Premium — через отдельное юридическое решение.
6. **Украинская аудитория.** Чувствительность к связке «Яндекс/Россия» [R, competitive-landscape §2.2]. Локализация UI на украинский и украинские раскладки — вероятный фактор выбора [A].

## 7. Пробелы и как их закрыть

| Пробел | Почему | Как закрыть |
|---|---|---|
| Число пользователей Punto/Caramba/Kright | Не публикуется; сайты недоступны | Разрешить `yandex.ru`, `apps.microsoft.com`, `github.com` (счётчики релизов); Store-рейтинги |
| Готовность платить (WTP) в нише переключателей | Нет опросов; отзывов App Store не прочитано | Опрос/интервью 10–20 пользователей (с согласия владельца; без сбора данных в продукте); анализ отзывов Store после разрешения домена |
| Точные данные StatCounter | Только сводки | Разрешить `gs.statcounter.com` |
| Применимость CRA, НДС, права на отказ, санкций | Юридический анализ вне компетенции MKT | CLO |
| Статус «Writing Tools в любом поле» Windows 11 | Нет подтверждения выпуска | Повторить поиск при следующем этапе |

## 8. История
| Дата | Изменение | Автор |
|---|---|---|
| 2026-11-10 | Создан документ (V4) | AGENT 10 |
