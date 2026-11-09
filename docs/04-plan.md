# KeySwitch — План работ

Процесс: ANALYZE → PLAN → DESIGN → DEVELOP → REVIEW → TEST → SECURITY AUDIT → RELEASE.
Лимит: ≤ 4 активных потока. Каждый поток — своя ветка, PR, независимое ревью. Слияние в `main` — с подтверждения владельца.

## Этап 0 — Анализ (текущий)
| Задача | Роль | Результат | Статус |
|--------|------|-----------|--------|
| PRD | Product Analyst | `01-prd.md` | черновик |
| Архитектура | System Architect | `02-architecture.md` | черновик |
| Модель угроз | Security Analyst | `03-threat-model.md` | черновик |
| Дизайн-концепция и wireframes | UI/UX Designer | интерактивный прототип (artifact) + `06-design-system.md` | не начато |

## Этап 1 — Спайки (снимают риски до массовой разработки)
| ID | Ветка | Роль | Цель | Критерий завершения |
|----|-------|------|------|---------------------|
| S-1 | `spike/converter-model` | Rust Dev | n-граммная модель + кандидаты | Все кейсы FR-CONV-3 проходят на Linux CI |
| S-2 | `spike/win-uia-probe` | Rust/Windows Dev | Чтение/выделение/вставка через UIA в тестовом окне, буфер с форматами исключения | Windows CI зелёный; отчёт: что проверено, что нет |
| S-3 | `spike/sandbox-wasm` | Rust Dev + Security | Wasmtime + QuickJS + python.wasm в AppContainer | Враждебные скрипты отклонены на Windows CI; замеры размера и старта |
| S-4 | `chore/ci-scaffold` | Build Engineer | Каркас Tauri 2 + workflows (linux core, windows build, audit) | Пустое приложение собирается в MSI/NSIS на Windows CI |

Параллельно можно S-1, S-3, S-4 (разные файлы); S-2 — после S-4 (нужен каркас CI).

## Этап 2 — Разработка (после GATE 2–3)
`feat/core` · `feat/platform-win` · `feat/engine` · `feat/sandbox` · `feat/ui-shell` · `feat/macro-studio` · `feat/storage`

## Quality Gates
| Gate | Условие | Кто подтверждает |
|------|---------|------------------|
| 1 | PRD + критерии приёмки согласованы | Владелец |
| 2 | Архитектура + модель угроз, спайки S-1…S-3 | Architect + Security + Владелец |
| 3 | Дизайн-система и основные экраны | Designer + Владелец |
| 4 | Конвертация RU↔EN покрыта тестами, CI зелёный | QA |
| 5 | Windows-интеграция: CI + **ручная приёмка владельцем** | QA + Владелец |
| 6 | Macro Studio: враждебные скрипты на Windows CI | Security + QA |
| 7 | Нет открытых критических функциональных дефектов | QA |
| 8 | Нет открытых Critical/High уязвимостей | Security |
| 9 | Сборка, документация, решение о подписи кода | Build + Владелец |

## Блокеры
- **B-1:** GitHub не подключён к облачной сессии → нет удалённого репозитория и CI.
