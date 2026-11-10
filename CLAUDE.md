# KeySwitch — инструкции для Claude

Фоновое приложение Windows 10/11 (позже macOS): исправляет текст, набранный в неверной раскладке (EN ↔ RU), по горячей клавише; Macro Studio с текстом/JS/Python в песочнице. Всё локально.

**Главный принцип: качество, безопасность и сохранность данных важнее скорости.**

## Документы (источник истины — `docs/PROJECT_MAP.md` §3)
- Директива: `docs/master-instructions-v4.md` (поверх V3 и V1). Платные функции и цены — только решение владельца (LEVEL 3).
- `docs/00-decisions.md` — принятые решения D-xx. Не нарушать.
- `docs/TEAM_STRUCTURE.md` — команда, уровни, руководители. `docs/DEVELOPMENT_WORKFLOW.md` — статусы задач, ревью, 12 gates.
- `docs/SYSTEM_ARCHITECTURE.md` — архитектура. `docs/SECURITY_REVIEW.md` — реестр CISO. `docs/LEGAL_COMPLIANCE_REGISTER.md`, `docs/LICENSE_REGISTER.md` — реестры CLO.
- `docs/BACKLOG.md`, `docs/ROADMAP.md`, `docs/PROJECT_STATUS.md`, `docs/RISK_REGISTER.md`.
- Требования: `docs/research/pa-prd.md` (до GATE 3). Документы `01`–`04` устарели.

## Структура
- `crates/ks-core` — ядро конвертации. **Без I/O, без ОС-зависимостей, без `unsafe`.**
- `app/` — React/TS/Vite/Tailwind UI; `app/src-tauri` — Tauri 2 оболочка (crate `keyswitch`).
- Будущие crates: `ks-layouts`, `ks-platform`, `ks-platform-win`, `ks-engine`, `ks-sandbox-host`, `ks-storage` (см. архитектуру).

## Команды
- Все проверки: `scripts/check.sh` (`--quick` — только fmt/clippy/тесты/фронтенд). Отчитываться по его Summary.
- Фронтенд: `npm --prefix app ci`, `npm --prefix app run build`.
- Windows cross-check (только компиляция): `cargo clippy -p keyswitch --target x86_64-pc-windows-msvc`.
- Хост облачной сессии — Linux: собрать/запустить Tauri-приложение здесь нельзя (нет webkit2gtk), и это не нужно. Исполнение на Windows — только через GitHub Actions (`.github/workflows/ci.yml`).

## Обязательные правила
1. Текст пользователя: только тип `UserText` (без `Debug`/`Display`), никогда в логи, файлы, panic-сообщения, тестовые снапшоты с реальными данными.
2. Никаких сетевых вызовов в runtime. Не добавлять Tauri-плагины `fs`, `shell`, `http`; capabilities — минимальные.
3. Новая зависимость — только с обоснованием (назначение, лицензия, риск) в описании PR; точная версия, не моложе 2 недель; `cargo deny` должен проходить.
4. GitHub Actions — только с фиксацией по SHA и комментарием версии.
5. Один поток = одна ветка (`spike/*`, `feat/*`, `fix/*`, `chore/*`, `docs/*`) = один PR. Не редактировать файлы, которыми владеет другой активный поток.
6. **Не сливать в `main`, не публиковать релиз, не принимать остаточный риск Critical/High — без явного подтверждения владельца.**
7. Не выдумывать результаты. В отчётах разделять: «проверено (как, где)» / «не проверено (почему)». Код написан ≠ этап завершён.
8. Горячие клавиши — только `RegisterHotKey`; никаких низкоуровневых хуков клавиатуры, инъекций в чужие процессы, повышения прав.
9. Пользовательский код (макросы) исполняется только в `ks-sandbox-host` (Wasmtime + AppContainer); его действия применяет ядро после проверки политики.

## Коммуникация
- С владельцем — по-русски, кратко, с явными решениями, которые нужны от него.
- Код, идентификаторы, комментарии в коде, сообщения коммитов — по-английски.

## Команда (реестр — `docs/TEAM_STRUCTURE.md`, субагенты в `.claude/agents`)
AGENT 01 Project Director (основная сессия) · 02 product-analyst (Lead, Product & Market) · 03 system-architect (Lead, Engineering) · 04 ui-ux-designer (Lead, Design & Frontend) · 05 rust-windows-dev (Windows Integration) · 06 frontend-dev · 07 security-analyst (Senior, под CISO, только чтение) · 08 qa-engineer · 09 release-engineer · 10 market-analyst (Senior, постоянный) · **11 ciso** и **12 clo** — независимые директора с правом блокировки · 13 runtime-sandbox-dev · 14 runtime-security · 15 windows-security · 16 licensing-specialist · 17 b2b-researcher. ≤ 4 активных потока.
- Security Review (CISO) и Legal Review (CLO) — по триггерам `DEVELOPMENT_WORKFLOW.md` §1; их отрицательное заключение Project Director отменить не может.
- **market-analyst обязателен**: при старте, перед проектированием UI, перед каждой новой функцией, на каждом крупном этапе (новые идеи), перед коммерческим запуском и крупными обновлениями (позиционирование, монетизация). Не пропускать при планировании; его идеи оценивают product-analyst, ui-ux-designer, system-architect, security-analyst; решение — Orchestrator с владельцем.

## Навыки (`.claude/skills`)
`/check` — прогнать проверки и честно отчитаться · `/spike` — запуск и закрытие спайка · `/security-gate` — независимое ревью безопасности по T-xx · `/stage-report` — отчёт этапа по форме ТЗ §20 · `/add-layout` — добавить язык/раскладку.
