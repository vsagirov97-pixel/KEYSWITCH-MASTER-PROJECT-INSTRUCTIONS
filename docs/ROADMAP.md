# ROADMAP — последовательность развития KeySwitch

Обновлено 2026-11-10. Этапы и gates — по директиве V3/V4. Основная платформа — Windows 11, Windows 10 — дополнительная совместимость. Независимые задачи идут параллельно (≤ 4 активных потока), обязательные контрольные точки не пропускаются.

| # | Этап | Основные задачи (BACKLOG) | Команда / руководитель | Выход | Состояние |
|---|------|---------------------------|------------------------|-------|-----------|
| 0 | Принятие проекта | BL-GOV-01…03 | Project Director | GATE 1 Audit, GATE 2 Team | **закрыт** 2026-11-10 |
| 1 | Product Research | BL-PRD-01…03, BL-MKT-01…02 | Product & Market / AGENT 02 | GATE 3 Requirements | исследования готовы, консолидация впереди |
| 2 | Architecture & Governance | BL-ARCH-01, BL-SEC-01, BL-LEG-01…03, **спайки S-2, S-3** | Engineering / AGENT 03, CISO, CLO | GATE 4 Architecture | архитектура v2.1 записана; спайки не начаты |
| 3 | Design | BL-DES-01…02 | Design & Frontend / AGENT 04 | GATE 5 Design | прототип ждёт отзыва владельца |
| 4 | Core Engine | BL-CORE-01…03 | Engineering | GATE 6 Core | прототип S-1 готов |
| 5 | Windows Integration | BL-WIN-01…04 | Engineering / AGENT 05 | GATE 7 (CI + ручная приёмка) | после S-2 |
| 6 | Macro Studio | BL-SBX-01…02, BL-UI-03 | Engineering / AGENT 13 + Design & Frontend | GATE 8 | после S-3 |
| 7 | Interface Integration | BL-UI-01…02, BL-SYS-01 | Design & Frontend / AGENT 04 | — | после GATE 5 |
| 8 | Quality Assurance | BL-QA-01…03 | AGENT 08 | GATE 9 | — |
| 9 | Security & Legal Clearance | BL-SEC-02, BL-LEG-04 | CISO, CLO | GATE 10, GATE 11 | — |
| 10 | Release | BL-REL-01…03 | AGENT 09 | GATE 12 (разрешение владельца) | — |

## Коммерческий трек (V4 §8, параллельно разработке)
C-1/C-2 исследование (MKT, AGENT 17) → C-3…C-7 оценка всеми профильными специалистами → C-8 единое предложение → **решение владельца** (LEVEL 3). Платные функции не реализуются до решения. См. `COMMERCIAL_PRODUCT_PROPOSAL.md`.

## Ближайшие шаги (порядок)
1. ✅ Пакет принятия проекта завершён (GATE 1–2). Коммерческое исследование C-1/C-2 — в работе.
2. Параллельно (3 кодовых потока + дизайн):
   - S-2 Windows integration probe (AGENT 05, ревью CISO);
   - S-3 Sandbox (AGENT 13, ревью CISO, лицензии CPython/QuickJS — CLO);
   - BL-CORE-01 корпус ≥ 500 фраз и защита URL/e-mail (AGENT 08 + AGENT 05);
   - BL-DES-01 правки дизайна по отзыву владельца.
3. Консолидация PRD (GATE 3) после отчёта рынка и результатов S-2 по матрице приложений.

## Future Roadmap (вне MVP)
macOS-версия · дополнительные языки (украинский, немецкий, французский) · несколько слотов макросов (платно) · языковые пакеты · корпоративная политика · платежи · идеи AGENT 10 со статусом «later». Облачная синхронизация и интернет-доступ скриптов — не планируются без отдельного решения владельца, CISO и CLO.
