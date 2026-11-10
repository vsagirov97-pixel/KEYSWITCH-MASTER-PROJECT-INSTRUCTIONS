# Реестр команды KeySwitch

Единый реестр ролей. Идентификатор роли уникален и не переиспользуется. Изменения состава — только по решению владельца.

| ID | Роль | Агент Claude Code (`.claude/agents/`) | Статус | Право блокировки | Основные артефакты |
|----|------|-----------------------------------------|--------|------------------|--------------------|
| AGENT 01 | Project Orchestrator | основная сессия (файла нет) | постоянный | gate-решения (с владельцем) | `docs/04-plan.md`, `docs/research/synthesis.md`, отчёты этапов |
| AGENT 02 | Product Analyst | `product-analyst.md` | постоянный | — | `docs/01-prd.md`, матрица приложений, граничные случаи |
| AGENT 03 | System Architect | `system-architect.md` | постоянный | архитектурные дефекты | `docs/02-architecture.md`, зависимости |
| AGENT 04 | UI/UX Designer | `ui-ux-designer.md` | постоянный | — | дизайн-концепция, дизайн-система, прототипы |
| AGENT 05 | Rust / Windows Developer | `rust-windows-dev.md` | постоянный | — | `crates/*` |
| AGENT 06 | Frontend Developer | `frontend-dev.md` | постоянный | — | `app/` |
| AGENT 07 | Security Analyst (независимый) | `security-analyst.md` (только чтение) | постоянный | **да** — Critical/High блокируют релиз | `docs/03-threat-model.md`, аудиты |
| AGENT 08 | QA Engineer | `qa-engineer.md` | постоянный | критические функциональные дефекты (GATE 7) | тесты, чек-листы приёмки |
| AGENT 09 | Build & Release Engineer | `release-engineer.md` | постоянный | — | CI/CD, установщик |
| AGENT 10 | Market & Competitive Intelligence Analyst | `market-analyst.md` | **постоянный, на весь срок жизни продукта** | — | `docs/market/` |

## Обязательное участие AGENT 10
| Момент | Что делает | Кто принимает результат дальше |
|--------|-----------|-------------------------------|
| Старт проекта | Комплексное исследование рынка и конкурентов | Orchestrator → Product Analyst |
| Перед проектированием интерфейса | Лучшие дизайн-решения desktop-приложений | UI/UX Designer |
| Перед добавлением любой новой функции | Аналоги и возможности улучшения | Product Analyst (полезность), Architect (реализуемость), Security |
| Каждый крупный этап | Новые продуктовые идеи | Orchestrator, отбор с владельцем |
| Перед коммерческим запуском и каждым крупным обновлением | Позиционирование и модели монетизации | Orchestrator + владелец |

Окончательное решение о включении функций принимает Project Orchestrator по согласованию с владельцем продукта. Orchestrator обязан запрашивать отчёты AGENT 10 на каждом этапе и не пропускать его при планировании.
