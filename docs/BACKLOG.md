# BACKLOG — задачи KeySwitch

Обновлено 2026-11-10. Правила — `DEVELOPMENT_WORKFLOW.md`. Статусы: Backlog / Ready / In Progress / Review / Security Review / Legal Review / QA / Blocked / Done. Приоритет: P0 блокирует gate, P1 — нужно в MVP, P2 — желательно.
Сокращения ролей: PD — Project Director (01), PA — 02, ARCH — 03, DES — 04, WIN — 05, FE — 06, SA — 07, QA — 08, REL — 09, MKT — 10, CISO — 11, CLO — 12, SBX — 13. «—» в колонках QA/Sec/Legal: проверка не требуется или ещё не проводилась (см. статус).

| ID | Название и описание | P | Исполнитель | Руководитель | Проверяющий | Зависит от | Критерии приёмки | Статус | Результат | QA | Security | Legal |
|----|---------------------|---|-------------|--------------|-------------|------------|------------------|--------|-----------|----|----------|-------|
| BL-GOV-01 | Аудит проекта и карта (V3 ч. 7) | P0 | PD | — | CISO, CLO (свои разделы) | — | 11 документов V3 созданы, противоречия устранены | Done (GATE 1) | `docs/PROJECT_MAP.md` и др. | — | CISO: реестр готов | CLO: реестры готовы |
| BL-GOV-02 | Организационная структура, назначение CISO/CLO, AGENT 13 | P0 | PD | — | владелец (прозрачность) | — | `TEAM_STRUCTURE.md`, агенты созданы | Done (GATE 2) | `docs/TEAM_STRUCTURE.md` | — | — | — |
| BL-GOV-03 | Пометить устаревшие документы, обновить CLAUDE.md под V3 | P1 | PD | — | ARCH | BL-GOV-01 | Нет двух «источников истины» по одной теме | In Progress | — | — | — | — |
| BL-SEC-01 | Единый реестр безопасности, решения CISO по Q-1/3/8/9, X-4, XR-07 | P0 | CISO | — | SA | — | `SECURITY_REVIEW.md` с владельцами и статусами | Done (v1, проверено PD) | `docs/SECURITY_REVIEW.md` | — | — | — |
| BL-LEG-01 | Реестр лицензий всех зависимостей и данных | P0 | CLO | — | ARCH | — | `LICENSE_REGISTER.md`: лицензия, обязательства, действия | Done (v1) | `docs/LICENSE_REGISTER.md` | — | — | — |
| BL-LEG-02 | Применимость права ЕС (GDPR, ePrivacy, CRA, AI Act, DSA, EAA, защита потребителей, ответственность за продукт) | P0 | CLO | — | человек-юрист (по списку CLO) | — | Реестр с актами, статьями, источниками, датами | Done (v1, всё UNVERIFIED — нужна проверка источников и юристом) | `docs/LEGAL_COMPLIANCE_REGISTER.md` | — | — | — |
| BL-LEG-03 | Название «KeySwitch», товарные знаки; лицензия данных модели (R-7) | P1 | CLO | — | владелец (решение) | BL-LEG-01 | Рекомендация с вариантами | Review (ждёт решения владельца) | `docs/legal/name-and-ip-review.md` | — | — | — |
| BL-LEG-05 | Notice/атрибуция в сборках: CC-BY-SA данных, лицензии крейтов; LICENSE репозитория после решения владельца | P0 | LS-1 (AGENT 16) + REL | CLO | CLO | решение владельца о лицензии | Публичный CI-артефакт содержит notice; в репозитории есть LICENSE | Ready | — | — | — | нужен |
| BL-GOV-04 | Защита `main`, secret scanning, push protection, Dependabot (настройки GitHub — делает владелец) | P0 | владелец | PD | CISO | — | Настройки включены, CISO подтвердил | Blocked (ждёт владельца) | — | — | нужен | — |
| BL-LEG-06 | Механизм обновлений безопасности без нарушения «нет сети» (требования CRA/PLD) | P0 | CLO + CISO + ARCH | PD | владелец | решение владельца | Вариант утверждён до GATE 4 | Blocked (ждёт владельца) | — | — | нужен | нужен |
| BL-COM-01 | Коммерческое исследование B2C: рынок, конкуренты, потребности, Premium, монетизация (V4 §8) | P1 | MKT | PA | PD | BL-MKT-01 | 5 документов в `docs/commercial/` с источниками и пометками доказательств | In Progress | `docs/commercial/` | — | — | — |
| BL-COM-02 | Коммерческое исследование B2B: требования, Business, Enterprise | P1 | AGENT 17 | PA | PD | — | 3 документа в `docs/commercial/` | In Progress | `docs/commercial/` | — | — | — |
| BL-COM-03 | Оценка кандидатов по 12 критериям: PA (требования), ARCH (техника), DES (UX), CISO, CLO | P1 | PA, ARCH, DES, CISO, CLO | PD | PD | BL-COM-01, BL-COM-02 | Каждая функция оценена всеми | Backlog | — | — | нужен | нужен |
| BL-COM-04 | Единое коммерческое предложение владельцу (LEVEL 3) | P1 | PD | — | владелец | BL-COM-03 | `COMMERCIAL_PRODUCT_PROPOSAL.md` по V4 §8.9 | Backlog | `docs/COMMERCIAL_PRODUCT_PROPOSAL.md` | — | нужен | нужен |
| BL-ARCH-02 | Архитектурный задел под уровни лицензий без сети (feature flags, офлайн-файл лицензии) — только проектирование, без реализации платных функций | P2 | ARCH | ARCH | CISO, CLO | BL-COM-04 | Проект согласован, не реализуется до решения владельца | Backlog | — | — | нужен | нужен |
| BL-MKT-01 | Исследование рынка и конкурентов (B-14) | P0 | MKT | PA | PD | — | Отчёты в `docs/market/` | **Done** (проверено PD: файлы, источники, пометки V/R) | `docs/market/` | — | — | — |
| BL-MKT-02 | Оценка 22 идей MI-01…MI-22 | P1 | PA | PA | ARCH, CISO, CLO | BL-MKT-01 | Каждая идея: принято/отклонено/later с обоснованием | Ready | `docs/market/ideas-backlog.md` | — | — | — |
| BL-PRD-01 | Консолидация PRD v2 → `01-prd.md`, учёт решений D-17…D-19 и идей | P0 | PA | PA | PD, QA (тестируемость) | BL-MKT-02, BL-SEC-01 | GATE 3: MVP утверждён | Backlog | — | — | нужен | нужен |
| BL-PRD-02 | Матрица приложений: учесть данные рынка (WhatsApp/UIA), результаты S-2 | P1 | PA | PA | QA | S-2 | Матрица с фактическими статусами | Backlog | `docs/research/pa-app-matrix.md` | — | — | — |
| BL-PRD-03 | Сочетания по умолчанию (без конфликтов: Ctrl+Shift+Space занят 1Password) | P2 | PA + DES | PA | QA | S-2 | Список проверенных сочетаний | Backlog | — | — | — | — |
| BL-ARCH-01 | Внести 13 исправлений в AV2/FR/DEP; поддерживать `SYSTEM_ARCHITECTURE.md` | P0 | ARCH | ARCH | CISO, CLO | BL-SEC-01 | Документы согласованы, CISO без замечаний | Ready | `docs/SYSTEM_ARCHITECTURE.md` | — | нужен | нужен (зависимости) |
| **S-2** | Windows integration probe: фикстуры (Win32 Edit/RichEdit, WPF, Edge/WebView2, Electron, Qt/PySide6, Firefox), UIA read/select/verify, W1/W3/W2, контракт буфера, шлюз поколений, раскладка, RegisterHotKey, окно выбора, crash/WER | P0 | WIN | ARCH | CISO, QA | BL-ARCH-01 (частично), CLO по PySide6 | 27 сценариев на Windows CI; отчёт спайка «проверено/не проверено» | Ready | — | — | нужен | нужен (PySide6 LGPL) |
| **S-3** | Sandbox: wasmtime ≥ 48.0.8, сборка QuickJS-ng/CPython из SHA, stdlib из памяти + Wizer, LPAC/Job, протокол с лимитами, H-01…H-40, матрица Cranelift/Pulley × ACG | P0 | SBX | ARCH | CISO | BL-SEC-01 | Все враждебные скрипты отклонены на Windows CI; замеры | Ready | — | — | нужен | нужен (CPython PSF, QuickJS MIT) |
| BL-CORE-01 | Корпус ≥ 500 фраз, метрики качества, защита URL/e-mail/путей (FR-CONV-9) | P1 | QA + WIN | ARCH | QA | — | Метрики PRD выполнены на корпусе | Ready | — | — | — | нужен (источник корпуса) |
| BL-CORE-02 | Тип `UserText` (zeroize, без Debug/Display/Clone) в `ks-types` | P1 | WIN | ARCH | CISO | BL-ARCH-01 | Тесты + clippy-правило | Backlog | — | — | нужен | — |
| BL-CORE-03 | Калибровка порогов уверенности, fuzz ядра | P2 | WIN | ARCH | QA | BL-CORE-01 | Fuzz без паник; пороги обоснованы корпусом | Backlog | — | — | — | — |
| BL-CFG-01 | Быстрые правки конфигурации: явные capabilities вместо `core:default`, `--locked`, бан-лист `deny.toml`, `--ignore-scripts`, автозаполнение WebView2 выкл. | P1 | REL | PD | CISO | — | CI зелёный, CISO подтвердил | Ready | — | — | нужен | — |
| BL-DES-01 | Правки дизайна по отзыву владельца и данным рынка | P1 | DES | DES | PD | отзыв владельца | GATE 5 | Blocked (ждёт отзыва) | прототип | — | — | — |
| BL-DES-02 | Юридически значимые экраны: о программе, лицензии, приватность | P2 | DES | DES | CLO | BL-LEG-02 | CLO согласовал тексты | Backlog | — | — | — | нужен |
| BL-ENG-01 | `ks-types`, `ks-platform` (трейты), `ks-platform-fake`, `ks-engine` | P1 | WIN + ARCH | ARCH | CISO, QA | S-2 | Unit-тесты конвейера на fake | Backlog | — | — | нужен | — |
| BL-WIN-01 | `ks-platform-win` по итогам S-2, профили приложений | P1 | WIN | ARCH | CISO, QA | S-2, BL-ENG-01 | Windows CI + ручная приёмка | Backlog | — | — | нужен | — |
| BL-WIN-02 | Трей, автозапуск, single-instance | P1 | WIN | ARCH | QA | BL-ENG-01 | Тесты + ручная проверка | Backlog | — | — | нужен | — |
| BL-WIN-03 | `ks-storage` (SQLite, LocalAppData, журнал метаданных) | P1 | WIN | ARCH | CISO | BL-ENG-01 | Тесты; схема без текста | Backlog | — | — | нужен | нужен (GDPR) |
| BL-WIN-04 | Panic hook, исключение из WER, crash-тест | P1 | WIN | ARCH | CISO | BL-ENG-01 | Нет дампов с текстом | Backlog | — | — | нужен | — |
| BL-SBX-01 | `ks-sandbox` + `ks-sandbox-proto` продуктово по итогам S-3 | P1 | SBX | ARCH | CISO | S-3 | GATE 8 | Backlog | — | — | нужен | — |
| BL-SBX-02 | API макросов, разрешения слота (буфер, Enter/Tab), привязка к хэшу | P1 | SBX | ARCH | CISO, PA | BL-SBX-01 | Тесты политики | Backlog | — | — | нужен | — |
| BL-UI-01 | UI-каркас, 6 разделов, i18n RU/EN, темы | P1 | FE | DES | DES, QA | GATE 5 | Соответствие дизайн-системе, a11y | Backlog | — | — | — | — |
| BL-UI-02 | Интеграция UI с backend (typed invoke без текста) | P1 | FE | DES | CISO | BL-ENG-01 | Нет пользовательского текста в IPC | Backlog | — | — | нужен | — |
| BL-UI-03 | Macro Studio (CodeMirror 6), проверка ошибок | P1 | FE | DES | QA | BL-SBX-02 | Сценарии F-3 дизайна | Backlog | — | — | нужен | — |
| BL-SYS-01 | Окно выбора языка (нативное, WS_EX_NOACTIVATE) | P1 | WIN | ARCH | DES, CISO | S-2 | Фокус поля не меняется | Backlog | — | — | нужен | — |
| BL-QA-01 | Тестовая стратегия и чек-лист ручной приёмки по матрице | P1 | QA | PD | PA | S-2 | Утверждён владельцем | Backlog | — | — | — | — |
| BL-QA-02 | Регрессия по 119 граничным случаям | P1 | QA | PD | PA | BL-WIN-01 | Отчёт с фактическим выводом | Backlog | — | — | — | — |
| BL-QA-03 | Ручная приёмка владельцем (GATE 7) | P0 | владелец + QA | PD | QA | BL-WIN-01 | Чек-лист пройден | Backlog | — | — | — | — |
| BL-SEC-02 | Полный security-аудит перед релизом | P0 | CISO + SA | — | — | BL-QA-02 | GATE 10 | Backlog | — | — | — | — |
| BL-LEG-04 | Юридическое заключение к релизу, EULA, политика приватности, проверка юристом-человеком | P0 | CLO | — | человек-юрист | BL-LEG-02 | GATE 11 | Backlog | — | — | — | — |
| BL-REL-01 | Подпись кода (решение владельца, платно) | P1 | REL | PD | CISO, CLO | — | Решение + настройка | Backlog | — | — | нужен | нужен |
| BL-REL-02 | SBOM и THIRD_PARTY из реестра лицензий | P1 | REL | PD | CLO | BL-LEG-01 | Генерируется в CI | Backlog | — | — | — | нужен |
| BL-REL-03 | Релизная сборка, документация | P0 | REL | PD | QA, CISO, CLO | все gates | GATE 12 | Backlog | — | — | нужен | нужен |
