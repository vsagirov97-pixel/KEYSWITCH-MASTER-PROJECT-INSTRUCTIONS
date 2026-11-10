# PROJECT_MAP — карта проекта KeySwitch

Состояние на 2026-11-10 (аудит Project Director). Легенда: ✅ реализовано и проверено · 🧪 прототип · 📄 только документ · ⬜ не начато.

## 1. Направления и компоненты

| Направление | Компонент | Состояние | Где | Ответственный |
|-------------|-----------|-----------|-----|---------------|
| Продукт | PRD v2, сценарии, FR/NFR, критерии приёмки | 📄 (черновик, не утверждён) | `docs/research/pa-prd.md` | AGENT 02 |
| Продукт | Матрица приложений, граничные случаи | 📄 (всё UNVERIFIED) | `docs/research/pa-app-matrix.md`, `pa-edge-cases.md` | AGENT 02 |
| Рынок | Конкуренты, боли, UI-референсы, 22 идеи | 📄 | `docs/market/` | AGENT 10 |
| Архитектура | Консолидированная архитектура | 📄 | `docs/SYSTEM_ARCHITECTURE.md` (+ `docs/research/arch-*`) | AGENT 03 |
| Ядро | `ks-core`: раскладки EN/RU по скан-кодам, модель языка, конвертация, смешанный текст | 🧪 30 тестов (Linux + Windows CI) | `crates/ks-core` | AGENT 03 / 05 |
| Windows-интеграция | UIA, SendInput, буфер, хоткеи, раскладки, трей, автозапуск | ⬜ (спайк S-2 спроектирован) | — | AGENT 05 |
| Песочница | Wasmtime + QuickJS/CPython WASI, LPAC/Job | ⬜ (спайк S-3 спроектирован) | — | AGENT 13 |
| Оболочка | Tauri 2 каркас, IPC-смоук `core_version` | 🧪 (собирается в NSIS на Windows CI, не запускался на Windows) | `app/src-tauri` | AGENT 09 / 06 |
| UI | React/TS/Tailwind каркас-заглушка | 🧪 | `app/src` | AGENT 06 |
| Дизайн | Концепция, дизайн-система, кликабельный прототип | 📄 + прототип | `docs/research/design-*`, https://claude.ai/artifact/WSAEX8vEH4aqUZv1HvRy8k | AGENT 04 |
| Безопасность | Модель угроз, аудит, требования, ревью | 📄 | `docs/SECURITY_REVIEW.md` (CISO), `docs/research/sec-*`, `review-*` | AGENT 11 / 07 |
| Право | Реестр соответствия, реестр лицензий, IP | 📄 (в работе) | `docs/LEGAL_COMPLIANCE_REGISTER.md`, `docs/LICENSE_REGISTER.md`, `docs/legal/` | AGENT 12 |
| QA | Тесты ядра; корпус ≥ 500 фраз; чек-лист ручной приёмки | 🧪 / ⬜ | `crates/ks-core/tests` | AGENT 08 |
| Сборка | CI: core (Linux), frontend, Windows build + NSIS (unsigned), supply chain | ✅ зелёный на `main` | `.github/workflows/ci.yml`, `deny.toml` | AGENT 09 |
| Управление | Структура команды, workflow, roadmap, backlog, статус, риски | 📄 | `docs/*.md` (этот набор) | AGENT 01 |

## 2. Репозиторий
- `main` — каркас (S-4), CI, документы этапа 0 (PR #1, #2 слиты владельцем).
- `spike/converter-model` — S-1 (в `main` не слит).
- `research/phase-1` — S-1 + исследования фазы 1 + рынок (в `main` не слит, PR не открыт).
- `docs/governance-v3` — этот пакет управления (поверх `research/phase-1`).
- Issues: нет. Задачи ведутся в `docs/BACKLOG.md`.

## 3. Документы — что является источником истины

| Тема | Источник истины | Устарело / заменено |
|------|-----------------|---------------------|
| Директива | `docs/master-instructions-v3.md` (+ продуктовые требования `master-instructions.md`) | — |
| Решения | `docs/00-decisions.md` | — |
| Команда | `docs/TEAM_STRUCTURE.md` | `docs/team.md` (ссылка) |
| Требования | `docs/research/pa-prd.md` → после GATE 3 переносится в `docs/01-prd.md` | `docs/01-prd.md` (черновик этапа 0) |
| Архитектура | `docs/SYSTEM_ARCHITECTURE.md` | `docs/02-architecture.md` |
| Безопасность | `docs/SECURITY_REVIEW.md` | `docs/03-threat-model.md` |
| План | `docs/ROADMAP.md`, `docs/BACKLOG.md`, `docs/DEVELOPMENT_WORKFLOW.md` | `docs/04-plan.md`, `docs/research/synthesis.md` §4–6 |
| Статус | `docs/PROJECT_STATUS.md` | — |
| Риски | `docs/RISK_REGISTER.md` | разделы рисков в research-документах |
| Право | `docs/LEGAL_COMPLIANCE_REGISTER.md`, `docs/LICENSE_REGISTER.md` | `THIRD_PARTY.md` (будет сгенерирован из реестра) |
