# B2B_REQUIREMENTS — потребности бизнеса и организаций

Автор: AGENT 17 B2B & Enterprise Market Researcher · Дата: 2026-11-10 · Статус: **первое исследование, черновик для Product Analyst (AGENT 02) → Project Director; ни одна функция не утверждена** (V4 п.5: состав платных функций и цены утверждает только владелец по `COMMERCIAL_PRODUCT_PROPOSAL.md`).
Связано: `BUSINESS_FEATURE_RESEARCH.md`, `ENTERPRISE_FEATURE_RESEARCH.md` (этот же автор); `docs/market/*` (AGENT 10); `docs/LEGAL_COMPLIANCE_REGISTER.md` (CLO); `docs/SYSTEM_ARCHITECTURE.md`.

## 0. Методика и ограничения (читать первым)

**Метки доказательств** (как у AGENT 10):
- **[V] verified** — первичный источник прочитан целиком. В этом отчёте [V] относится **только к документам репозитория**. Ни одна внешняя страница не открыта: `WebFetch` к `learn.microsoft.com` вернул DNS-ошибку (хост вне разрешённой сети сессии), поэтому дальнейшие попытки не делались.
- **[R] reported** — сводка поисковой выдачи (`WebSearch`) по странице; страницу целиком автор не читал. Цифры из [R] **не являются подтверждёнными ценами/фактами**.
- **[A] assumption** — вывод/оценка автора.

Все внешние источники — дата обращения **2026-11-10**.

**Пробелы входных данных:**
1. В `docs/master-instructions-v4.md` в репозитории — только сводка V4 (14 строк); **частей 8.3, 8.6, 8.9 в файле нет**. Требования этих частей реконструированы по заданию Orchestrator и п.4, 6, 9 сводки. Нужен полный текст V4 ч.8 → Orchestrator/владелец.
2. Нет ни одного интервью с ИТ-отделом, закупками или ИБ. **Прямых доказательств B2B-спроса именно на переключатель раскладки нет** — есть косвенные (см. §2). Это главный риск всего B2B-направления.
3. Не известны страна владельца, наличие юрлица, канал продаж (LEGAL_COMPLIANCE_REGISTER, строка «Неизвестно») — это прямо влияет на подпись кода (§5.3), госзакупки (§5.7) и договоры.

Чтобы поднять уровень доказательности до [V], нужно разрешить в сети сессии: `learn.microsoft.com`, `textexpander.com`, `blaze.today`, `phraseexpress.com`, `ergonis.com`, `caramba-switcher.com`, `kavanati.com` (или фактический домен), `attack.mitre.org`, `azure.microsoft.com`, `wingetly.io`, `reestr.digital.gov.ru`.

## 1. Ключевые выводы (TL;DR)

1. **B2B-спрос на исправление раскладки — косвенный и слабый [A].** Сигналы: Caramba Switcher Pro публикуется на Wingetly с параметрами тихой установки для Intune [R, AGENT 10]; израильский Kavanati (иврит↔англ.) продаёт **лицензии на компьютер для организаций** [R]; EveryLang в реестре российского ПО (№ 9637) и позиционируется на МСБ [R]. Ни одного вендора-переключателя с ADMX/админ-консолью не найдено.
2. **Сильный и доказанный B2B-рынок — у соседей: текстовые экспандеры** (TextExpander, Text Blaze, PhraseExpress, Typinator). Их модель «бесплатно/дёшево лично — платно для бизнеса» и набор бизнес-функций (общие библиотеки, админ, SSO, статистика) — лучший ориентир. Но почти всё у них **облачное** [R].
3. **Главная B2B-ценность KeySwitch — не функции, а «допустимость» на корпоративном ПК [A]:** нет хука клавиатуры, нет сети, нет инъекций, текст не сохраняется. Для ИБ «переключатель раскладки» по умолчанию = класс «легальный кейлоггер» (Punto: дневник набора [R anti-malware.ru]); для работодателя в ЕС кейлоггинг сотрудников жёстко ограничен (BAG 2017, CNIL) [R]. Документы, доказывающие отсутствие перехвата, — вероятно, важнее любой платной функции.
4. **Почти все корпоративные потребности решаются без сервера** [A]: MSI per-machine, политики HKLM/ADMX (образец — PowerToys ADMX + Intune OMA-URI [R]), подписанные файлы конфигурации/макросов/лицензии, обновление средствами самой организации (Intune/ConfigMgr/winget). Сервер нужен только для облачной админ-консоли, онлайн-активации, SSO/SCIM, централизованного сбора отчётов и облачной синхронизации — это **изменение концепции (LEVEL 3, владелец)**.
5. **Главные барьеры закупки:** подпись кода (SmartScreen/Smart App Control; Azure Artifact Signing для частных лиц — только США/Канада [R]); AV/EDR и application control (Macro Studio с JS/Python попадает в класс «интерпретатор скриптов», ср. MITRE T1059.010 для AutoHotkey [R]); анкеты безопасности (SIG/CAIQ), SBOM (CRA — с 11.12.2027 [V, LEGAL_COMPLIANCE_REGISTER R-03]); для госсектора РФ — реестр ПО (ПП № 1236) [R].

## 2. Доказательства спроса — сводка

| Сигнал | Что показывает | Метка, источник |
|---|---|---|
| Caramba Switcher Pro на Wingetly с параметрами тихой установки/Intune | интерес админов к массовой установке переключателя | [R] https://wingetly.io/apps/sergey-moskalev/caramba-switcher-pro (через AGENT 10, competitive-landscape §2) |
| Kavanati (иврит↔англ., Win/Mac): лицензии «на компьютер» для организаций; Pro ₪99/год для частных лиц | **единственный найденный переключатель с явным предложением организациям** | [R] https://alternativeto.net/software/kavanati/about/ (сайт вендора не открыт; цены орг. лицензии не найдены) |
| EveryLang: реестр российского ПО № 9637, «ориентирован на МСБ» | спрос в РФ-сегменте, где реестр — условие закупки | [R] https://soware.ru/products/everylang |
| PowerToys: ADMX-шаблоны (с 0.64), глобальное и поштучное отключение утилит, Intune OMA-URI `./Device/Vendor/MSFT/Policy/Config/PowerToys~Policy~PowerToys/...` | Microsoft считает обязательным для утилиты-«клавиатурного» инструмента дать ИТ запрет/фиксацию функций | [R] https://learn.microsoft.com/windows/powertoys/grouppolicy , https://microsoft.github.io/PowerToys/processes/gpo/ |
| TextExpander Business — $99.96/польз./год (офиц. страница цен в сводке поиска), общие группы сниппетов организации, права админа, статистика организации; SOC 2/SOC 3 по запросу | готовность бизнеса платить за «текстовый» инструмент per-seat | [R] https://textexpander.com/pricing , https://textexpander.com/learn/organization/managing-organization-snippet-groups , https://textexpander.com/security |
| Text Blaze Business $6.99/польз./мес (год); Enterprise — по запросу: SAML SSO, SCIM, журнал аудита | то же; SSO/SCIM — только Enterprise | [R] https://blaze.today/plans |
| PhraseExpress: бесплатно лично, коммерческое использование платно; Enterprise ≈ $230–249.95/польз. разово (сторонние сайты, расхождение); SQL Server-архитектура и Client Manager для центр. управления пользователями и лицензиями; свой сервер упразднён в v16 (2022) | бизнес покупает **разовые** per-seat лицензии с собственным (on-prem) сервером | [R] https://www.phraseexpress.com/features/ , https://phraseexpress.com/doc/share/sql/ , https://phraseexpress.com/blog/software-releases/phraseexpress-server-retired , https://us.fitgap.com/products/phrase-express |
| Typinator (Mac): Business-подписка «для малого бизнеса», > 20 лицензий — через продажи; ISO 27001; данные не на серверах вендора, синхронизация через NAS/WebDAV/OneDrive по выбору; Enterprise ≈ $59.99/год (агрегатор) | **локальная модель (без облака вендора) продаётся бизнесу как преимущество приватности** — ближайший аналог позиционирования KeySwitch | [R] https://ergonis.com/typinator/pricing , https://ergonis.com/typinator/privacy , https://blog.ergonis.com/hc/en-us/articles/23719662968220 |
| Espanso (OSS): нет центр. управления, нет компании → нет DPA/SLA/договора поддержки (по оценке конкурента Ergonis) | «бесплатный OSS» не закрывает потребность бизнеса в поставщике с обязательствами | [R] https://us.fitgap.com/products/espanso , https://blog.ergonis.com/hc/articles/27897272054684 (источник ангажирован) |
| Кейлоггинг сотрудников: BAG (ФРГ) 27.07.2017 — скрытый кейлоггер недопустим; CNIL — кейлоггеры = постоянный контроль, нужна DPIA (напоминание 07.2026) | работодатель в ЕС не может легально ставить ПО, похожее на кейлоггер, без серьёзного обоснования → спрос на **доказуемо не-кейлоггер** | [R] https://www.hunton.com/privacy-and-cybersecurity-law-blog/french-data-protection-authority-rules-on-keylogger-software , https://twobirds.com/en/insights/2017/germany/employee-monitoring-by-keylogger-software-unlawful-except-in-case-of-severe-suspicions , https://leglobal.law/2026/08/31/employee-monitoring-the-french-data-protection-agency-issues-a-reminder-on-applicable-rules/ |
| Punto: «легальный кейлоггер» (дневник набора), антивирусы не детектируют; эвристика помечала `pshook.dll` | ИБ имеет основания блокировать класс продуктов | [R] https://www.anti-malware.ru/threats/keyloggers ; конкретных корпоративных запретов Punto **не найдено** |
| Колл-центры/поддержка: шаблонные ответы снижают AHT (цифры 25–35 % — из глоссариев без исследования) | спрос на шаблоны у поддержки реален, **на исправление раскладки — данных нет** | [R] https://textexpander.com/blog/average-handle-time , https://converge-static.pages.dev/glossary/canned-response (слабый источник) |

**Вывод [A]:** доказан спрос бизнеса на (а) управляемые текстовые инструменты per-seat и (б) управляемость утилит через GPO/Intune. Спрос на *платный корпоративный переключатель раскладки* — гипотеза (H-4 AGENT 10 «слабая» — подтверждаю). Валидация: 5–10 интервью с ИТ/ИБ мультиязычных компаний (RU/UA/KZ/IL/GR/страны Балтии) до любой разработки — задача AGENT 02, согласие владельца.

## 3. Потребности по сегментам

| Сегмент | Кто решает / покупает | Ключевые потребности | Что важно в KeySwitch | Барьеры | Сила доказательств |
|---|---|---|---|---|---|
| **Малый бизнес (1–50)** | владелец/офис-менеджер, «приходящий» админ | легально использовать на работе; простая покупка (карта, счёт); 1 ключ на всех; без сервера | бесплатное ядро; понятная коммерческая лицензия; тихая установка | цена должна быть ниже порога «без согласования» [A]; неподписанный exe = SmartScreen | Слабая [A]; аналогия PhraseExpress/Typinator Business [R] |
| **Средние (50–1000)** | ИТ-отдел, иногда ИБ; закупка по счёту | MSI, Intune/GPO, фиксированные настройки, запрет Macro Studio, групповые настройки по отделам, объём лицензий, счёт/договор | ADMX, HKLM-политики, подписанный конфиг, лицензия-файл, документ для ИБ | анкета безопасности, AV/EDR allow-list, поддержка | Средняя по аналогам (PowerToys ADMX, TextExpander Business) [R] |
| **Крупные (1000+)** | CIO/CISO, закупки, юристы; пилот → тендер | ConfigMgr/Intune, App Control (WDAC/AppLocker), SBOM, подписанные бинарники, политика обновлений (кольца), документация развёртывания, SLA, договор (DPA не нужен при отсутствии обработки — проверить CLO) | всё выше + whitepaper, data-flow statement, воспроизводимая сборка, период поддержки | SIG/CAIQ, юр. лицо и страхование, срок поддержки ≥ 5 лет (CRA ст.13(8)) | Слабая для переключателя; сильная для требований к любому ПО [R] |
| **Международные** | центральный ИТ + локальные офисы | несколько языковых пар (UK/RU/EN, HE/EN, EL/EN, KK/RU), единый пакет, локализованный UI, разные раскладки по офисам | D-15 «раскладка = данные»; групповые профили языков | языки вне RU/EN не готовы (NG-9) | Средняя для мультиязычия ([V] Kright/RuSwitcher — любые пары, через AGENT 10) |
| **ИТ-отделы (как пользователи)** | сами админы | терминалы, RDP, консоли — там KeySwitch отказывает (по решению) | честный отказ; не мешает RDP (нет хука) | ожидание «работает везде» | [V] SYSTEM_ARCHITECTURE §4.2 |
| **Колл-центры** | руководитель КЦ + ИТ | скорость ответа; шаблоны; единые формулировки; **недопустимость случайной отправки** (Enter); работа в CRM/веб-чатах | исправление раскладки в браузерных CRM (только выделение до S-2); управляемые макросы-шаблоны; Enter выключен (D-18) | браузеры/contenteditable не подтверждены (S-2); часто VDI/Citrix [A] | Слабая на раскладку, средняя на шаблоны [R] |
| **Службы поддержки (helpdesk)** | руководитель поддержки | то же + тикет-системы (Zendesk/Jira в браузере) | то же | то же | то же |
| **Компании с многоязычным персоналом** | HR/ИТ | меньше ошибок раскладки в переписке; поддержка UA/KZ/HE | ядро + языковые пакеты | качество UK/RU различения (MI-17) | Средняя (AGENT 10 N-14) |
| **Чувствительные к конфиденциальности (финансы, юристы, госсектор, медицина)** | CISO, DPO, комплаенс | **доказуемое отсутствие перехвата и сети**; нет облака; нет логов текста; офлайн-активация; подписанные сборки; SBOM; совместимость с DLP/EDR; отказ в полях паролей | архитектурные принципы KeySwitch (D-05, D-14, нет сети) + доказательства | анкеты, сертификации (ISO 27001/SOC 2 — у Typinator/TextExpander [R]), для РФ-госсектора — реестр (ПП 1236) [R]; для HIPAA — BAA не нужен, если нет PHI у вендора [A, CLO] | Средняя: требования известны; спрос на KeySwitch — не проверен |

## 4. Исследуемые потребности (по заданию)

Для каждой: спрос (доказательства) · кто покупает · барьеры · конкуренты · **без сервера?** (соответствие принципу «нет сети в runtime»).

### 4.1 Централизованное управление лицензиями
- **Спрос:** у экспандеров — стандарт: PhraseExpress Client Manager (именные и конкурентные лицензии из пула на SQL Server) [R phraseexpress.com/doc/share/sql/configuration/licenses]; TextExpander/Text Blaze — облачные организации [R]. У переключателей — Kavanati «per-computer» [R]; Caramba — «3 ПК на лицензию» (потребительская) [R].
- **Покупатель:** ИТ/закупки средних и крупных.
- **Барьеры:** онлайн-активация не проходит в изолированных сетях; per-device учёт при VDI.
- **Без сервера:** **да** — организационная лицензия как подписанный файл (Ed25519) в `HKLM`/`ProgramData`, разворачивается MSI/Intune; учёт мест — по договору/самодекларации (true-up) [A]. Аналог офлайн-кода: JetBrains offline activation code [R https://sales.jetbrains.com/hc/articles/360016995379]. Онлайн-пул лицензий — **сервер → концепция (владелец)**.

### 4.2 Корпоративные политики
- **Спрос:** PowerToys ADMX/Intune [R]; MI-16 AGENT 10 (ADMX: отключить Macro Studio, зафиксировать хоткеи и исключения) [V ideas-backlog]. Прямых запросов к переключателям не найдено.
- **Покупатель:** ИТ/ИБ средних+.
- **Барьеры:** ADMX должен корректно грузиться в Intune (ADMX ingestion) — проверить [A].
- **Без сервера:** **да** (HKLM\Software\Policies\KeySwitch + ADMX/ADML).

### 4.3 Массовое развёртывание (MSI/MSIX, Intune, SCCM/ConfigMgr, GPO, winget)
- **Спрос:** админы предпочитают MSI (стандартные ключи `/qn`, журнал, ProductCode для детекта), контекст System → `ALLUSERS=1`, нельзя «съезжать» в per-user [R https://www.splashtop.com/blog/msi-vs-exe-installer , https://doc.acrobits.net/troubleshooting/windows-silent-bulk-installation/ , https://codenote.pages.dev/en/posts/windows-desktop-app-intune-distribution-isv-playbook/]. Intune Enterprise App Catalog — только Win32, 64-бит, Microsoft не отвечает за сторонние приложения; авто-обновление каталога GA в 06.2026 (по Recast) [R https://learn.microsoft.com/en-us/intune/app-management/deployment/enterprise-app-management , https://www.recastsoftware.com/resources/microsoft-intune-enterprise-application-management-auto-update-now-generally-available/].
- **Технически [R/A]:** Tauri 2 собирает MSI (WiX v3, только на Windows) и NSIS; WebView2 — режим offline installer (~127 МБ) или fixed runtime (~180 МБ) нужен для изолированных сетей — bootstrapper без доступа к CDN Microsoft молча падает [R https://v2.tauri.app/zh-cn/distribute/windows-installer/]. Сейчас в проекте — NSIS [V LEGAL_COMPLIANCE_REGISTER].
- **Без сервера:** **да**.

### 4.4 Панель администратора
- **Спрос:** у SaaS-экспандеров — веб-консоль (TextExpander, Text Blaze) [R]; у PhraseExpress — десктопный Client Manager + SQL [R].
- **Без сервера:** только как **локальный «редактор политик/пакетов»** (десктоп-утилита, генерирующая подписанный конфиг/ADMX-значения). Веб-консоль с онлайн-состоянием парка — **сервер → концепция**.

### 4.5 Групповые настройки
- **Спрос:** TextExpander — группы сниппетов организации с правами [R]; PowerToys — только машинные/пользовательские политики [R].
- **Без сервера:** **да** — нацеливание средствами ИТ (группы безопасности AD/Entra → разные GPO/Intune-профили с разными подписанными конфигами).

### 4.6 Управляемые макросы
- **Спрос:** сильный у экспандеров (общие библиотеки — ядро их Business-тарифов) [R]. Для KeySwitch — **сдержан**: D-07/NG-5 (импорт чужих макросов в v1 отсутствует, один слот).
- **Барьеры:** ИБ воспринимает исполнение скриптов как риск (T1059.010 для AHK/AutoIt; рекомендация MITRE — application control на интерпретаторы) [R https://attack.mitre.org/techniques/T1059/010/]. Управляемые макросы должны быть **подписаны организацией** и исполняться только в песочнице (CLAUDE.md п.9).
- **Без сервера:** **да** — подписанный пакет макросов в `ProgramData`. Требует пересмотра D-07/NG-5 → **владелец**.

### 4.7 Контроль функций
- **Спрос:** PowerToys — включение/выключение каждой утилиты политикой [R]. Для KeySwitch наиболее вероятный запрос ИБ [A]: «выключить Macro Studio целиком», «запретить доступ макросов к буферу», «запретить W2 (вставку через буфер)», «зафиксировать список исключений».
- **Без сервера:** **да**.

### 4.8 Офлайн-работа
- **Спрос:** для изолированных сетей (финансы, госсектор, оборона, промышленность) офлайн — условие [A]; рынок показывает: Typinator — «интернет только для проверки лицензии и обновлений» [R]; JetBrains — офлайн-коды [R].
- **KeySwitch:** офлайн по архитектуре [V SYSTEM_ARCHITECTURE §1.4]. **Сильное отличие, если и лицензирование офлайн** [A].

### 4.9 Корпоративная поддержка
- **Спрос:** у Typinator Business — приоритетная поддержка, онбординг [R]; Text Blaze Enterprise — обучение и внедрение [R]; Espanso — отсутствие поставщика с обязательствами как недостаток [R, ангажированный источник].
- **Барьеры:** стоимость поддержки для команды без штата; SLA требует юр. лица и ресурсов [A].
- **Без сервера:** да (почта/портал вне продукта; продукт сам в сеть не ходит).

### 4.10 Управление обновлениями
- **Спрос:** организации хотят обновлять сами (кольца, отсрочка), без самообновления приложения [A; ср. Intune EAM supersedence/auto-update [R]]. Self-updating apps требуют правил сети к вендору [R learn.microsoft.com EAM].
- **Связь:** блокер B-2 (CRA/DCD требуют механизм доставки обновлений, у нас его нет — NG-7) [V LEGAL_COMPLIANCE_REGISTER]. **Для B2B проблема мягче**: обновления доставляет ИТ (MSI в Intune/ConfigMgr/winget); продукту нужен только «уведомитель без сети» (локальная проверка даты сборки vs. периода поддержки) [A].
- **Без сервера:** да — со стороны KeySwitch; публикация MSI на сайте/GitHub Releases и манифест в `winget-pkgs` — это канал распространения, а не runtime-сеть [A; подтвердить CISO + CLO, т.к. «обновления» — совместная проверка V4 п.9].

### 4.11 Интеграция с MDM/endpoint management (Intune, SCCM/ConfigMgr, GPO/ADMX, MSI/MSIX, winget)
- **Intune:** Win32 (.intunewin) из MSI; ADMX ingestion для политик (образец PowerToys OMA-URI) [R]; Enterprise App Catalog — отдельная лицензия (Intune Suite / M365 E5 с 01.07.2026 по Tech Community) [R].
- **ConfigMgr/GPO:** MSI + MST-трансформации, назначение на компьютер [R support.kaspersky.com/KSC/15.1/ru-RU/155188.htm — общий порядок].
- **MSIX:** не исследовано глубоко; у Tauri нет штатного MSIX [A]; MSIX требует подписи и ограничивает автозапуск/доступ — оценка AGENT 09/03.
- **winget:** манифесты с `InstallerType` (msi/nullsoft/…); Wingetly публикует тихие ключи [R].
- **Без сервера:** да.

### 4.12 Отчётность без сбора текстов
- **Спрос:** TextExpander — статистика организации (сэкономленное время, использование групп) [R]; это облачный сбор. Для KeySwitch [A]: ИТ хочет знать «установлено/версия/политика применена/сколько отказов по кодам» — для оценки пользы и инцидентов.
- **Ограничения:** никакого текста, заголовков окон, URL, длины текста (SYSTEM_ARCHITECTURE §7). Даже агрегаты по сотрудникам — данные о работниках (GDPR, трудовое право; CNIL/BAG) [R/A → CLO].
- **Без сервера:** да — локальный агрегированный файл (счётчики по кодам/дням), который ИТ забирает своими инструментами (Intune Remediations/ConfigMgr inventory) при включённой политике; по умолчанию выкл. Центральный дашборд вендора — **сервер → концепция** и, вероятно, конфликт с принципом «нет телеметрии» [A].

## 5. Барьеры закупки и проверки безопасности

### 5.1 Анкеты безопасности поставщика
- Форматы SIG (Shared Assessments), CAIQ (CSA), VSA; SBOM в SPDX/CycloneDX — частый пункт [R https://www.vmukti.com/answers/vendor-security-questionnaire-vms — маркетинговый источник].
- **Для KeySwitch [A]:** большая часть разделов (хостинг, шифрование данных клиента у вендора, доступ персонала) — «неприменимо: данные не покидают устройство». Нужен **готовый пакет ответов** (pre-filled CAIQ-lite / «Security & Privacy Statement») — дешёвый способ пройти закупку.
- SOC 2 у TextExpander, ISO 27001 у Ergonis [R]. Для on-device продукта без облака SOC 2 менее релевантен, но крупные покупатели могут спросить [A]; стоимость сертификации — вне ресурсов на старте [A].

### 5.2 AV/EDR и application control
- MITRE T1059.010: AutoHotkey/AutoIt как техника атак; mitigation — запрет интерпретаторов application control [R https://attack.mitre.org/techniques/T1059/010/]. Руководства ИБ: блокировать `AutoHotkey.exe` вне разрешённых каталогов (AppLocker/WDAC; WDAC предпочтительнее) [R https://answers.securityscientist.net/q/23419/...].
- **Последствие [A]:** `ks-sandbox.exe` с QuickJS/CPython — «встроенный интерпретатор». Нужны: подписанный отдельный exe, документ «почему песочница не T1059» (LPAC, Job Object, нет сети/ФС), политика `MacroStudio=Disabled`, чтобы ИБ могла разрешить основной exe, не разрешая скрипты.
- Microsoft Defender PUA-блокировка для MDE включена по умолчанию [R https://learn.microsoft.com/... detect-block-potentially-unwanted-apps]. Риск классификации KeySwitch как PUA — низкий без бандлинга [A].
- Нет хука/инъекций → меньше эвристических срабатываний «keylogger» [A, AGENT 10 N-11].

### 5.3 Подпись кода
- Smart App Control (Win 11) блокирует неподписанные/неизвестные приложения; подпись необходима, но не достаточна (облачная репутация); на 25H2 SAC проще включить [R https://textslashplain.com/2026/04/28/smart-app-control/ , https://www.computerworld.com/article/4043925/windows-11-smart-app-control-explained.html].
- Azure Artifact Signing (бывш. Trusted Signing): по сводке DevClass $9.99/мес (5 000 подписей) / $99.99/мес; публичное доверие — **организации США/Канады/ЕС/Великобритании; частные лица — только США/Канада**; ИП/sole trader ЕС может не пройти [R https://devclass.com/2026/01/14/code-signing-windows-apps-may-be-easier-and-more-secure-with-new-azure-artifact-service/ , https://learn.microsoft.com/azure/artifact-signing/quickstart , https://learn.microsoft.com/en-us/answers/questions/5915697/].
- **Вывод [A]:** без подписи B2B невозможен; D-11 (решение перед релизом) для B2B становится **предусловием**. Решение о юр. лице/стране владельца влияет на выбор CA → владелец + AGENT 09 + CLO.

### 5.4 SBOM и цепочка поставок
- CRA: SBOM, CVD-политика, период поддержки ≥ 5 лет, обязательства с 11.12.2027, отчётность ст.14 с 11.09.2026 [V LEGAL_COMPLIANCE_REGISTER R-03, сам регламент UNVERIFIED].
- США: EO 14028 — требования SBOM для федеральных поставщиков; по Endor Labs (10.2026) OMB отменил обязательную аттестацию в 01.2026, SBOM-требование сохранено (M-26-05) — **не подтверждено** [R https://www.endorlabs.com/learn/executive-order-14028-sbom-requirements]. Частные заказчики копируют эти формулировки [A].
- Уже есть: `cargo deny`, фиксация зависимостей, публичный репозиторий (D-09) — сильная база; нужен SBOM в CI (CycloneDX) — AGENT 09.

### 5.5 Право и приватность
- Нет обработки персональных данных вендором → DPA/BAA вероятно не нужны, но заказчик всё равно спросит письменное заявление о потоках данных [A → CLO].
- Локальная отчётность (4.12) может сделать заказчика контролёром данных о сотрудниках — опция только по политике, по умолчанию выкл. [A → CLO].
- Доступность: для публичных закупок ЕС — EN 301 549/WCAG [V LEGAL_COMPLIANCE_REGISTER].

### 5.6 Юридическое лицо, договор, срок поддержки
- Крупные покупатели требуют юр. лицо, счёт с НДС, договор, ограничение ответственности, иногда страхование [A]; CRA — период поддержки [V register]. Пока неизвестно (N-7, N-8 register).

### 5.7 Госсектор РФ (если рынок рассматривается)
- ПП № 1236 от 16.11.2015: запрет закупки иностранного ПО при наличии аналога в реестре Минцифры [R https://www.consultant.ru/law/podborki/zapret_1236/ , https://www.ixbt.com/editorial/foreign-software-ban.shtml]; EveryLang — в реестре [R]. Для иностранного правообладателя этот сегмент закрыт [A]. Санкционный комплаенс — CLO (N-8).

## 6. Как конкуренты продают бизнесу (сводка моделей и наблюдаемых цен)

**Цены — только наблюдения из сводок поиска [R] на 2026-11-10, не подтверждены на страницах вендоров; это не рекомендация цены.**

| Продукт | Модель для бизнеса | Наблюдаемая цена | Облако? | Источник |
|---|---|---|---|---|
| TextExpander | подписка per-user; Business → Growth → Enterprise (SSO/SCIM на верхних) | Business $99.96/польз./год (офиц. стр.); иные агрегаторы $8.33–12/мес; одно сообщение о росте до $39.96 (04.2026) — **сомнительно** | да | [R] textexpander.com/pricing ; getpulsesignal.com/changes/textexpander |
| Text Blaze | подписка per-user; Enterprise по запросу (SAML SSO, SCIM, аудит) | Business $6.99/польз./мес (год), ≈ $8.39 помесячно | да | [R] blaze.today/plans |
| PhraseExpress | бесплатно лично; разовая per-user лицензия (+1 год обновлений); собственный SQL Server заказчика; объёмные скидки; **детект «коммерческого использования» эвристикой по содержимому** | Standard ≈ €49.95–$99.95; Pro ≈ €99.95–$149.95; Enterprise ≈ $230–249.95 (разные источники) | нет (on-prem SQL / NAS / облачное хранилище по выбору) | [R] phraseexpress.com/shop ; us.fitgap.com/products/phrase-express ; pcworld.com review |
| Typinator | разовая Basic; подписки Advanced/Business; > 20 лицензий — через продажи | Enterprise ≈ $59.99/год (агрегатор) | нет (данные у пользователя) | [R] ergonis.com/typinator/pricing |
| Caramba Switcher | потребительская подписка | Pro $4.99/год (Win, 3 ПК) | — | [R] через AGENT 10 |
| Kavanati | частные Pro + лицензии на компьютер для организаций | Pro ₪99/год; орг. цена не найдена | — | [R] alternativeto |
| PowerToys | бесплатно; ADMX/Intune как «цена входа» в корпоративную среду | 0 | нет | [R] learn.microsoft.com |
| Espanso | OSS, без B2B-предложения | 0 | нет | [R] |

**Наблюдения [A]:**
1. **Запрет на «детект бизнеса по содержимому» для KeySwitch:** модель PhraseExpress (эвристика по тексту) несовместима с правилом CLAUDE.md п.1 — коммерческое разграничение только **по лицензии/честному слову/политике**, не по анализу текста.
2. Диапазоны для обсуждения владельцем (не цены): (а) подписка per-seat ниже экспандеров, т.к. функция уже и есть бесплатные OSS-альтернативы — ориентир «ниже $7/мес Text Blaze»; (б) разовая per-seat с годом обновлений по образцу PhraseExpress; (в) site license (фикс за организацию/диапазон мест) — удобно при «нет сервера, нет учёта». Выбор модели — AGENT 10 (MONETIZATION_STRATEGY) и владелец.
3. Ценность для покупателя в B2B KeySwitch — **«разрешённость» и управляемость**, а не количество функций; бесплатная версия не ухудшается (V4 п.5) — бизнес-уровень добавляет управление, документы, поддержку, лицензию на коммерческое использование (если владелец выберет такую модель — MI-19, лицензия кода).

## 7. Что требует сервера (концепция → владелец, LEVEL 3)

| Возможность | Почему нужен сервер | Альтернатива без сервера |
|---|---|---|
| Облачная админ-консоль с состоянием парка | хранение и агрегация состояния устройств | ADMX + Intune/ConfigMgr отчёты заказчика; локальный редактор политик |
| Онлайн-активация / пул лицензий / плавающие лицензии | учёт мест в реальном времени | подписанный файл лицензии организации; true-up по договору |
| SSO/SCIM | вход в облачную учётку | не нужен, если нет облачной учётки |
| Облачная синхронизация управляемых макросов | доставка контента | подписанный пакет через Intune/GPO/файловый ресурс |
| Централизованный дашборд использования у вендора | сбор телеметрии | локальные агрегаты, собираемые инструментами заказчика (по умолчанию выкл.) |
| Автообновление из сети | runtime-сеть (NG-7) | обновления средствами ИТ; локальное «устарело» по дате |

Рекомендация [A]: **не предлагать серверные функции до доказательства спроса**; вся линейка Business/Enterprise первой волны реализуема без сервера и усиливает главный аргумент продукта.

## 8. Пробелы и следующие шаги
1. Полный текст V4 ч.8.3/8.6/8.9 → Orchestrator.
2. Интервью: 5–10 ИТ/ИБ в мультиязычных организациях (вопросы: блокируют ли переключатели; кто разрешает; нужен ли ADMX; готовность платить; модель — per-seat/site). Ответственный AGENT 02, согласие владельца.
3. Сетевой доступ к сайтам вендоров для перевода [R] → [V] (список в §0).
4. Решения владельца, влияющие на B2B: юр. лицо/страна (подпись, договоры, РФ-рынок), лицензия кода (MI-19), D-11 подпись, B-2 обновления.
5. Проверки: CISO — политика Macro Studio и документ про песочницу для ИБ; CLO + CISO + PA — корпоративное лицензирование (V4 п.9); AGENT 09 — MSI per-machine, WebView2 offline, SBOM.

## 9. Источники (все — 2026-11-10, [R], если не указано иное)
- Репозиторий [V]: `CLAUDE.md`, `docs/master-instructions-v4.md`, `docs/00-decisions.md`, `docs/SYSTEM_ARCHITECTURE.md`, `docs/LEGAL_COMPLIANCE_REGISTER.md`, `docs/research/pa-prd.md`, `docs/market/*`.
- PowerToys GPO: https://learn.microsoft.com/windows/powertoys/grouppolicy · https://microsoft.github.io/PowerToys/processes/gpo/ · https://www.anoopcnair.com/powertoys-utilities-in-windows-11-group-policy/
- TextExpander: https://textexpander.com/pricing · https://textexpander.com/blog/how-textexpander-pricing-saves-your-team-money · https://textexpander.com/learn/organization/managing-organization-snippet-groups · https://textexpander.com/security · https://getpulsesignal.com/changes/textexpander
- Text Blaze: https://blaze.today/plans
- PhraseExpress: https://www.phraseexpress.com/features/ · https://phraseexpress.com/doc/share/sql/ · https://phraseexpress.com/doc/share/sql/configuration/licenses · https://phraseexpress.com/blog/software-releases/phraseexpress-server-retired · https://www.phraseexpress.com/shop/freeware/ · https://phraseexpress.com/misc/PhraseExpress_license_agreement.pdf · https://us.fitgap.com/products/phrase-express · https://www.pcworld.com/article/456015/review-phraseexpress-9-adds-application-specific-macro-features.html
- Typinator: https://ergonis.com/typinator/pricing · https://ergonis.com/typinator/privacy · https://blog.ergonis.com/hc/en-us/articles/23719662968220 · https://blog.ergonis.com/hc/articles/27897272054684
- Espanso: https://us.fitgap.com/products/espanso · https://curtismchale.ca/2023/06/13/cross-platform-snippet-management-with-espanso
- Переключатели: https://alternativeto.net/software/kavanati/about/ · https://soware.ru/products/everylang · https://wingetly.io/apps/sergey-moskalev/caramba-switcher-pro · https://www.anti-malware.ru/threats/keyloggers
- Кейлоггинг и право: https://www.hunton.com/privacy-and-cybersecurity-law-blog/french-data-protection-authority-rules-on-keylogger-software · https://twobirds.com/en/insights/2017/germany/employee-monitoring-by-keylogger-software-unlawful-except-in-case-of-severe-suspicions · https://leglobal.law/2026/08/31/employee-monitoring-the-french-data-protection-agency-issues-a-reminder-on-applicable-rules/
- Развёртывание: https://www.splashtop.com/blog/msi-vs-exe-installer · https://doc.acrobits.net/troubleshooting/windows-silent-bulk-installation/ · https://codenote.pages.dev/en/posts/windows-desktop-app-intune-distribution-isv-playbook/ · https://learn.microsoft.com/en-us/intune/app-management/deployment/enterprise-app-management · https://www.recastsoftware.com/resources/microsoft-intune-enterprise-application-management-auto-update-now-generally-available/ · https://v2.tauri.app/zh-cn/distribute/windows-installer/ · https://support.kaspersky.com/KSC/15.1/ru-RU/155188.htm
- Безопасность/закупки: https://attack.mitre.org/techniques/T1059/010/ · https://answers.securityscientist.net/q/23419/what-are-the-key-mitigations-for-t1059-010 · https://learn.microsoft.com/sk-sk/defender-endpoint/detect-block-potentially-unwanted-apps-microsoft-defender-antivirus · https://www.vmukti.com/answers/vendor-security-questionnaire-vms · https://www.endorlabs.com/learn/executive-order-14028-sbom-requirements · https://www.ntia.doc.gov/files/ntia/publications/sbom_minimum_elements_report.pdf
- Подпись: https://textslashplain.com/2026/04/28/smart-app-control/ · https://www.computerworld.com/article/4043925/windows-11-smart-app-control-explained.html · https://devclass.com/2026/01/14/code-signing-windows-apps-may-be-easier-and-more-secure-with-new-azure-artifact-service/ · https://learn.microsoft.com/azure/artifact-signing/quickstart · https://learn.microsoft.com/en-us/answers/questions/5915697/can-an-irish-sole-trader-(non-incorporated)-qualif
- Офлайн-лицензии: https://sales.jetbrains.com/hc/articles/360016995379
- Колл-центры: https://textexpander.com/blog/average-handle-time · https://www.genesys.com/blog/post/4-ways-texting-improves-contact-center-metrics · https://converge-static.pages.dev/glossary/canned-response
- РФ госзакупки: https://www.consultant.ru/law/podborki/zapret_1236/ · https://www.ixbt.com/editorial/foreign-software-ban.shtml
