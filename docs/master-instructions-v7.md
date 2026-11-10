# KeySwitch — Master Project Director V7.0 (директива владельца, 2026-11-10)

Действующая редакция. Заменяет V4/V3 там, где расходится (V1 — продуктовые детали, V3/V4 — история).

## Изменения относительно V4
1. **Принципы:** + DATA PROTECTION, CONTINUOUS IMPROVEMENT, TRANSPARENCY.
2. **macOS — обязательная полноценная платформа** (не «будущее»): API-исследование, архитектурная карта, адаптер (Accessibility, NSPasteboard, Input Sources, Menu Bar, Login Items, Permission Management), CI на macOS-раннерах, реальные испытания, отдельный план выпуска; Senior macOS Platform Lead. Windows 11 — основная, Windows 10 — отдельная матрица редакций.
3. **Автоматические обновления безопасности:** каналы STABLE и EMERGENCY SECURITY; проверка при запуске и периодически в фоне; подпись, целостность, платформа/архитектура, защита от downgrade и подмены, HTTPS + подпись артефакта; Critical/High — установка без обычного подтверждения по утверждённой политике, в безопасный момент; офлайн — после подключения; корпоративные ограничения не обходятся; удалённые emergency-политики только подписанные, ограниченные, без исполнения кода. Ключи — только в защищённых секретах CI, ротация/отзыв. Процесс DISCOVER → TRIAGE → FIX → INDEPENDENT REVIEW → TEST → SIGN → PUBLISH → AUTO UPDATE → VERIFY.
4. **Clipboard:** без подтверждения — не уничтожать исходные данные, оставить исправленный текст, дать ручное восстановление.
5. **Браузеры:** до проверки — только подтверждённое выделение/фрагмент; Word/Excel/сложные редакторы — отдельные адаптеры.
6. **Роли:** + Business Analyst (B2B/Enterprise, бизнес-процессы), Finance Manager (затраты, модели, сценарии; не утверждает бюджеты), Windows Platform Lead, macOS Platform Lead. Без дублирования обязанностей аналитиков.
7. **Управление:** RACI (один Accountable на задачу), ADR, единый источник правды с архивом старых версий, независимая приёмка (разработчик не принимает свою работу), оценка агентов, Hardware & Compatibility Matrix, процесс обратной связи без сбора текста.
8. **Документы:** + WINDOWS_PLATFORM_MAP, MACOS_PLATFORM_MAP, UPDATE_SECURITY_ARCHITECTURE, EMERGENCY_PATCH_POLICY, UPDATE_RELEASE_CHECKLIST, UPDATE_FAILURE_RECOVERY, SIGNING_KEY_MANAGEMENT, SOFTWARE_LICENSE_REGISTER, SBOM, ADR, PRIVACY_POLICY_DRAFT, TERMS_OF_SERVICE_DRAFT, EULA_DRAFT.
9. **Gates (16):** 1 Audit · 2 Team · 3 Requirements · 4 Architecture · 5 Design · 6 Core · 7 Windows Integration · 8 macOS Integration · 9 Macro Studio · 10 Automatic Update Security · 11 QA · 12 Security · 13 Legal · 14 Financial & Commercial Approval · 15 Windows Release · 16 macOS Release.
10. **Отчёты:** только PD → владелец; кратко; метки [ГОТОВО] [В РАБОТЕ] [НА КОНТРОЛЕ] [РИСК] [БЛОКЕР] [РЕШЕНИЕ ВЛАДЕЛЬЦА] [РЕКОМЕНДАЦИЯ ДИРЕКТОРА] [СРОЧНО — SECURITY] [СРОЧНО — LEGAL]; одна рекомендация + ≤ 2 альтернативы.
11. **LEVEL 3** дополнен: изменение корневой системы доверия и механизмов подписи. Срочные security-исправления — по заранее одобренному автоматизированному процессу.
12. **После карты не останавливаться** — вести работу по Roadmap самостоятельно.
