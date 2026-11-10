> **УСТАРЕЛО (2026-11-10).** Действующий источник истины: `docs/SYSTEM_ARCHITECTURE.md`. Документ сохранён для истории.

# KeySwitch — Архитектура (v1)

Статус: **черновик для GATE 2** · Автор: System Architect · Ревью: Security Analyst (обязательно)

> Версии библиотек фиксируются при создании каркаса после проверки официальной документации и даты релиза. Здесь — только выбор технологий.

## 1. Принципы
1. Ядро (`core`) не знает об ОС. Всё системное — за трейтами-адаптерами.
2. Минимум зависимостей; каждая новая — с обоснованием в PR.
3. Песочница макросов — отдельный процесс; UI и ядро не исполняют пользовательский код.
4. Пользовательский текст живёт только в памяти, на время операции.

## 2. Структура репозитория
```
keyswitch/
├─ crates/
│  ├─ ks-core/          # конвертер, детектор языка, модель n-грамм, раскладки (no_std-friendly, без ОС)
│  ├─ ks-layouts/       # данные раскладок (TOML) + сгенерированные таблицы
│  ├─ ks-platform/      # трейты адаптеров (FocusProvider, TextAccess, Clipboard, Input, Layout, Hotkeys, Autostart)
│  ├─ ks-platform-win/  # реализация для Windows (windows-rs, UIA)
│  ├─ ks-engine/        # оркестрация операции: политика безопасности, state machine конвертации/макроса
│  ├─ ks-sandbox-host/  # отдельный процесс: Wasmtime + QuickJS.wasm + python.wasm, KeySwitch API
│  └─ ks-storage/       # SQLite: настройки, слот макроса, журнал диагностики (без текста)
├─ app/                 # Tauri 2 shell (src-tauri) + React/TS/Vite/Tailwind UI
│  └─ src/              # UI: экраны, дизайн-система, i18n (ru, en)
├─ tests/fixtures/      # корпус тест-кейсов конвертации
├─ docs/
└─ .github/workflows/   # CI: linux (core), windows (всё), сборка установщика
```

## 3. Ядро конвертации (`ks-core`)
- `Layout` — таблица: `ScanCode → (base, shift)` символ. Загружается из TOML.
- `convert(text, from, to) -> Vec<Candidate>` — для каждого символа, имеющего смысл в обеих раскладках, порождаются альтернативы; кандидаты оцениваются моделью.
- `LanguageModel` — символьные n-граммы (n=3..4), по одной на язык, сериализованы в бинарный файл (~сотни КБ). Обучается офлайн на открытом корпусе, лицензия корпуса фиксируется в `THIRD_PARTY.md`.
- `detect(text) -> Analysis { segments, direction, confidence, mixed }`.
- Детерминированность: одинаковый вход → одинаковый выход. Пороговые значения — в конфиге, покрыты тестами.
- Без `unsafe`, без I/O. Покрытие тестами — цель ≥ 90% строк.

## 4. Адаптеры платформы (`ks-platform`)
```rust
trait FocusProvider { fn focused() -> Result<FocusTarget>; }            // hwnd, pid, exe, runtime_id, control_type, is_password, elevated
trait TextAccess   { fn read_scope(&FocusTarget, Scope) -> Result<TextRegion>;
                     fn select(&FocusTarget, &TextRegion) -> Result<()>; }
trait Input        { fn wait_modifiers_released(timeout) -> Result<()>;
                     fn type_unicode(&str) -> Result<()>;  fn paste() -> Result<()>;
                     fn press(AllowedKey) -> Result<()>; }               // AllowedKey — закрытый enum (D-07)
trait Clipboard    { fn snapshot() -> Result<Snapshot>; fn put_private(&str) -> Result<Seq>;
                     fn restore_if_unchanged(Snapshot, Seq) -> Result<RestoreOutcome>; }
trait KeyboardLayout { fn current(&FocusTarget) -> LayoutId; fn switch(&FocusTarget, LayoutId) -> Result<()>; }
trait Hotkeys      { fn register(id, Chord) -> Result<()>; fn events() -> Receiver<HotkeyEvent>; }
trait Autostart    { fn set(bool) -> Result<()>; }
```
Windows-реализация: `windows` crate (Microsoft), UI Automation (`IUIAutomation`, TextPattern/TextPattern2, ValuePattern только для чтения), `SendInput`, `RegisterHotKey` в отдельном потоке с message loop, `WM_INPUTLANGCHANGEREQUEST`, `GetClipboardSequenceNumber`.

Профили приложений (`app-profiles.toml`): для известных exe — предпочитаемый способ ввода (SendInput/paste), допустим ли Ctrl+A после проверки UIA, задержка восстановления буфера. Неизвестное приложение — консервативный профиль.

## 5. Конвейер операции конвертации (`ks-engine`)
```
Hotkey → [Guard] → Focus → [Policy: password/elevated/excluded?] → ReadScope (D-03)
  → [len ≤ 5000] → Analyze → mixed? → Dialog(RU/EN/Cancel) → [Re-verify focus]
  → confidence ok? → Select → Write (SendInput | Clipboard-private) → RestoreClipboard → SwitchLayout → Log(metadata)
```
Любая ошибка на любом шаге → `Abort(reason)`; до шага Write изменений нет. Операции сериализованы (одна за раз), повторное нажатие во время операции игнорируется.

## 6. Песочница макросов (`ks-sandbox-host`)
- Отдельный процесс, запускается по требованию, общается с основным процессом по анонимному pipe (длина сообщения ограничена, формат — JSON с схемой).
- Процесс запускается в **AppContainer** без capabilities (нет сети, нет доступа к пользовательским файлам), Job Object: лимит памяти, запрет дочерних процессов, kill on close.
- Внутри — **Wasmtime**: модули `quickjs.wasm` и `python.wasm` (CPython WASI) встраиваются в установщик и проверяются по хешу. WASI без preopen-каталогов, без сокетов, без env, без часов высокого разрешения (по умолчанию). Лимиты: fuel/epoch (время), memory limiter, размер вывода.
- Скрипт **не вызывает систему напрямую**. Он возвращает список действий; выполняет их `ks-engine` после проверки политики.
- API (одинаковый для JS и Python):
  - `ks.context.text` — выделение или поле (по настройке слота), строка.
  - `ks.context.clipboard` — только если включено в слоте (D-07), иначе `null`.
  - `ks.convert(text, to)` — функция ядра.
  - Возврат: `{insert: str} | {replace: str} | {keys: [AllowedKey...]} | null`.
- Проверка синтаксиса: тот же песочный процесс, режим compile-only.

## 7. Tauri и UI
- Capabilities: только собственные команды приложения; плагины — минимальный набор (tray, autostart, single-instance — если нужны). Без `shell`, `fs`, `http` плагинов в WebView.
- CSP: `default-src 'self'`, без удалённых источников; `connect-src` только IPC.
- Состояние UI: React + лёгкий store; i18n — JSON-словари по ключам, язык = файл.
- Дизайн-система: токены (цвета, типографика, отступы) в Tailwind-конфиге, темы через CSS-переменные.

## 8. Хранение (`ks-storage`)
SQLite в `%APPDATA%\KeySwitch\`. Таблицы: `settings`, `macro_slot`, `diag_log` (без текста, ротация 1 000 записей). Скрипт макроса хранится как текст пользователя локально — это его данные.

## 9. macOS — задел
Адаптер `ks-platform-mac`: AXUIElement (Accessibility), TIS input sources, CGEvent, NSPasteboard; песочница — тот же Wasmtime + sandbox-exec/App Sandbox. Ядро и UI не меняются.

## 10. Технические риски
| ID | Риск | Вероятн. | Митигация |
|----|------|----------|-----------|
| R-1 | Неполная поддержка UIA в Electron/Qt (Discord, WhatsApp, Telegram) | Высокая | Профили приложений, Ctrl+A после проверки (D-04), ручная приёмка |
| R-2 | Гонка восстановления буфера | Средняя | SendInput для коротких текстов, проверка sequence number, профильная задержка |
| R-3 | python.wasm: размер, время старта, совместимость сборки | Средняя | Ранний спайк (S-3), замер до GATE 6 |
| R-4 | Ложные срабатывания антивирусов | Высокая | Без хуков, без инъекций; подпись — перед релизом (D-11) |
| R-5 | Качество модели языка на коротких строках | Средняя | Корпус тестов, порог уверенности, отказ при сомнении |
| R-6 | Невозможность проверить реальные приложения в CI | Определённо | Ручная приёмка (D-10) |
