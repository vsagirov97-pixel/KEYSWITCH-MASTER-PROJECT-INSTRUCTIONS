# ADR — реестр архитектурных решений KeySwitch

Владелец реестра: AGENT 03 (System Architect). Требование V7 §7. Источник истины по решениям — `docs/00-decisions.md` (D-xx); ADR фиксирует **обоснование** решения (контекст, альтернативы, последствия, проверку) и ссылается на D-xx. Если ADR и D-xx расходятся — действует D-xx, ADR исправляется.

## Правила
- Файл `ADR-NNNN-<kebab-name>.md`, ≤ 40 строк; разделы: Контекст · Проблема · Альтернативы · Решение · Обоснование · Последствия · Проверка.
- Статусы: **Proposed** (ждёт одобрения) → **Accepted** → **Superseded by ADR-NNNN** / **Deprecated**. Принятый ADR не переписывается по существу — вместо этого новый ADR, старый помечается Superseded.
- Новый ADR — при решении уровня LEVEL 2/3, меняющем границы компонентов, модель доверия, зависимости ядра, процессную модель или контракты безопасности.
- Ревью: CISO — для решений с security-триггерами (`DEVELOPMENT_WORKFLOW.md` §1), CLO — для лицензий/данных.

## Индекс

| ADR | Решение | Статус | D-xx / основание |
|---|---|---|---|
| [0001](ADR-0001-layouts-as-scancode-data.md) | Раскладки — данные по скан-кодам + языковая модель | Accepted | D-01, D-15 |
| [0002](ADR-0002-direction-detection.md) | Направление по составу текста; раскладка — tie-breaker | Accepted | D-02, D-20, D-19(b) |
| [0003](ADR-0003-write-methods-no-setvalue.md) | Запись W1 → W3 → W2 → отказ; без `ValuePattern.SetValue` | Accepted | D-12, D-19(c) |
| [0004](ADR-0004-clipboard-contract-no-blind-restore.md) | Контракт буфера; нет сигнала → не восстанавливать | Accepted | Q-1, D-13, D-26 |
| [0005](ADR-0005-registerhotkey-only.md) | Только `RegisterHotKey`, без хуков | Accepted | D-14 |
| [0006](ADR-0006-sandbox-wasmtime-separate-process.md) | Wasmtime в отдельном процессе с минимальными правами, ноль preopen | Accepted | D-08, XR-07, Q-9, D-18 |
| [0007](ADR-0007-mixed-text-picker-noactivate.md) | Окно выбора языка `WS_EX_NOACTIVATE` + временные хоткеи | Accepted | X-4 |
| [0008](ADR-0008-core-adapter-split-windows-macos.md) | Ядро/движок общие, адаптеры Windows и macOS | Accepted | D-24 |
| [0009](ADR-0009-update-channel-only-network.md) | Канал обновлений — единственная сеть в runtime | Accepted (реализация — Proposed) | D-25 |
| [0010](ADR-0010-proprietary-licence.md) | Закрытая лицензия, сторонние — по своим | Accepted | D-22(3) |
| [0011](ADR-0011-own-updater-tuf-lite.md) | Свой механизм обновлений TUF-lite вместо `tauri-plugin-updater` | **Proposed** | P-U1…P-U6 (`UPDATE_SECURITY_ARCHITECTURE.md` §20) |
