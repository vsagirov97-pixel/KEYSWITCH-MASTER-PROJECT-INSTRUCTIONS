# TEAM_STRUCTURE — организационная структура KeySwitch

Действует с 2026-11-10 (директива V3). Заменяет `docs/team.md` (оставлен как ссылка). ID ролей уникальны и не переиспользуются. Уровни присвоены по фактически выполненной и проверенной работе; сертификатов и «опыта» агентам не приписывается.

## Иерархия

```mermaid
graph TD
  OWNER[Владелец продукта] --> PD[AGENT 01 Project Director]
  PD -.->|организационно| CISO[AGENT 11 CISO]
  PD -.->|организационно| CLO[AGENT 12 CLO]
  PD --> PM[Product & Market Team<br/>Lead: AGENT 02]
  PD --> ENG[Engineering Team<br/>Lead: AGENT 03]
  PD --> DES[Design & Frontend Team<br/>Lead: AGENT 04]
  PD --> QA[AGENT 08 QA]
  PD --> REL[AGENT 09 Build & Release]
  CISO --> SEC[Security Team]
  CLO --> LEG[Legal Team]
  PM --> A10[AGENT 10 Market Analyst]
  ENG --> A05[AGENT 05 Windows Integration Dev]
  ENG --> A13[AGENT 13 Runtime & Sandbox Engineer]
  DES --> A06[AGENT 06 Frontend Dev]
  SEC --> A07[AGENT 07 Security Analyst]
```

CISO и CLO подчиняются Project Director только организационно: их экспертные заключения независимы, отрицательное заключение не может быть отменено Project Director (эскалация — к владельцу).

## Реестр ролей

| ID | Роль | Агент (`.claude/agents/`) | Уровень | Команда / подчинение | Основание уровня | Право блокировки |
|----|------|---------------------------|---------|----------------------|------------------|------------------|
| AGENT 01 | Project Director / CTO | основная сессия | Director | → владелец | назначен директивой V3 | gate-решения в пределах полномочий |
| AGENT 02 | Product Analyst | `product-analyst.md` | **Lead** (повышен с Senior) | Lead, Product & Market Team | PRD v2 (US/FR/NFR с трассировкой), матрица 18 приложений, 119 граничных случаев; выявил 16 противоречий, часть подтверждена архитектором и Security | — |
| AGENT 03 | System Architect | `system-architect.md` | **Lead** (Engineering Lead) | Lead, Engineering Team | исследование замены поля по исходникам Chromium/Qt и докам MS, архитектура v2, проверенные версии зависимостей; в перекрёстном ревью принял обоснованные исправления | архитектурные дефекты |
| AGENT 04 | UI/UX Designer | `ui-ux-designer.md` | **Lead** (Design & Frontend Lead) | Lead, Design & Frontend Team | концепция, дизайн-система с расчётом контраста (96/96 AA), рабочий прототип, проверенный в headless Chrome | — |
| AGENT 05 | Windows Integration Developer (бывш. Rust/Windows Dev) | `rust-windows-dev.md` | Middle | Engineering Team | ещё не выполнял задач как отдельный агент | — |
| AGENT 06 | Frontend Developer | `frontend-dev.md` | Middle | Design & Frontend Team | ещё не выполнял задач | — |
| AGENT 07 | Security Analyst | `security-analyst.md` (только чтение) | **Senior** | Security Team (→ CISO) | 53 угрозы, 28 находок со ссылками file:line, перекрёстное ревью с 10 новыми находками (подтверждены архитектором) | через CISO |
| AGENT 08 | QA Engineer | `qa-engineer.md` | Middle | → Project Director (индивидуально, независимо от разработки) | ещё не выполнял задач | критические функциональные дефекты (GATE 9) |
| AGENT 09 | Build & Release Engineer | `release-engineer.md` | Middle | → Project Director | ещё не выполнял задач | — |
| AGENT 10 | Market & Competitive Intelligence Analyst | `market-analyst.md` | **Senior** | Product & Market Team; постоянный, на весь срок жизни продукта (D-16) | исследование ~20 конкурентов, 18 болей, 22 идеи; честно отметил непроверенное | — |
| AGENT 11 | **CISO** — Director of Information Security & Product Security | `ciso.md` | Director | независимый директор | назначен директивой V3 | **да**: PR, функции, релиз при открытых Critical/High |
| AGENT 12 | **CLO** — Legal Director & Head of EU Regulatory Compliance | `clo.md` | Director | независимый директор | назначен директивой V3 | **да**: юридически неподготовленный выпуск, несовместимые лицензии |
| AGENT 13 | Runtime & Sandbox Engineer (новый) | `runtime-sandbox-dev.md` | Middle | Engineering Team; security-ревью — CISO | закрывает пробел компетенций WebAssembly/WASI/CPython/QuickJS/AppContainer для S-3 | — |

Приостановленных ролей нет. Слияний ролей нет: Product Analyst и Market Analyst разделены (решение владельца).

## Команды и зоны ответственности

| Команда | Руководитель | Состав | Зона ответственности | Передаёт результат |
|---------|--------------|--------|----------------------|--------------------|
| Product & Market | AGENT 02 (Lead) | 02, 10 | рынок, идеи, требования, MVP, критерии приёмки, матрица приложений | Project Director → GATE 3 |
| Engineering | AGENT 03 (Lead) | 03, 05, 13 | архитектура, ядро, Windows-интеграция, песочница, качество кода | Project Director → GATE 4, 6, 7, 8 |
| Design & Frontend | AGENT 04 (Lead) | 04, 06 | дизайн-система, прототипы, UI, i18n, доступность | Project Director → GATE 5 |
| Security | AGENT 11 CISO | 07 (+ специалисты по запросу CISO) | безопасность всего продукта | GATE 10, security review задач |
| Legal & Compliance | AGENT 12 CLO | специалисты по запросу CLO | право ЕС и национальное, лицензии, IP, распространение | GATE 11, legal review задач |
| QA (индивидуально) | AGENT 08 → Project Director | 08 | независимое функциональное тестирование | GATE 9 |
| Build & Release (индивидуально) | AGENT 09 → Project Director | 09 | CI/CD, сборка, установщик | GATE 12 |

QA и Release пока не образуют отдельных команд (по одному участнику). Команда создаётся, когда в направлении появляется второй участник; тогда назначается Senior/Lead руководитель.

## Правила повышения и замены
- Повышение — после минимум одной задачи, принятой независимой проверкой (QA, CISO или перекрёстное ревью), с записью основания в этой таблице.
- Понижение/замена — при повторных дефектах, обнаруженных проверяющими, или недостоверных отчётах.
- Каждое изменение структуры — запись в журнале ниже; владелец не утверждает обычные кадровые изменения (V3, ч. 8).

## Журнал изменений
| Дата | Изменение |
|------|-----------|
| 2026-11-10 | AGENT 10 Market Analyst добавлен (решение владельца, D-16). |
| 2026-11-10 | Директива V3: назначены AGENT 11 CISO и AGENT 12 CLO; AGENT 07 переподчинён CISO; созданы команды Product & Market, Engineering, Design & Frontend, Security, Legal; повышены 02, 03, 04 до Lead, 07 и 10 — Senior; добавлен AGENT 13 Runtime & Sandbox Engineer; AGENT 05 сфокусирован на Windows-интеграции. |
