# KeySwitch — подбор зависимостей (исследование)

Статус: **исследование / предложение** · Автор: System Architect (Agent 03) · Дата проверки реестров: **2026-11-09**
Связано: `docs/research/arch-architecture-v2.md` (далее [ARCH]), `docs/research/arch-windows-field-replacement.md` (далее [FR]).

**Редакция 2026-11-10 (BL-ARCH-01):** внесены исправления 1 и 13 из `review-arch-on-security.md` §2 (метки `[испр. N]`); версии в этих строках проверены по crates.io API 2026-11-10. Остальные версии — по состоянию на 2026-11-09, перед добавлением перепроверяются.

## 0. Метод и правила

- Версии и даты — **из реестров в этот день**: crates.io API (`curl -s https://crates.io/api/v1/crates/NAME -H "User-Agent: keyswitch-research"`), npm (`npm view NAME time --json`), PyPI JSON API. Сортировка по semver, пре-релизы исключены.
- Правило CLAUDE.md п.3: версия **не моложе 2 недель** → граница **2026-10-26**. Если новейшая версия моложе, указана рекомендуемая предыдущая и отмечено «(<2 нед.)».
- Лицензии — из метаданных реестра; соответствие `deny.toml` (allow: MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-3-Clause, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0) проверено по заявленной лицензии крейта, **не** по всему транзитивному графу — это сделает `cargo deny` в PR.
- «Размер» — размер пакета в реестре (crate_size / unpackedSize), это **не** вклад в бинарь. Вклад в бинарь/установщик — **UNVERIFIED**, измеряется в спайках.
- Не проверено: CVE/RUSTSEC по каждой версии (сделает `cargo deny`/`npm audit` в CI), даты релизов GitHub-ассетов (API GitHub недоступен из сессии — 403).
- Принцип: **минимум новых зависимостей**. Отмечаю, что уже присутствует в `Cargo.lock` через Tauri (не добавляет кода в граф).

Уже в `Cargo.lock` (через Tauri): `windows 0.62.2`, `windows-core 0.62.2`, `windows-sys 0.61.2/0.45.0`, `webview2-com 0.39.1`, `tauri 2.12.5`, `tao 0.37.2`, `wry 0.57.1`, `serde`, `serde_json`, `log`, `thiserror`. Замечание: в `app/src-tauri/Cargo.toml` указано `tauri = "2.12.4"`, а lock разрешён в `2.12.5` (caret) — корректно, но для воспроизводимости предлагаю указывать фактическую версию.

---

## 1. Rust — платформа Windows

| Крейт | Назначение | Версия (дата) | Лицензия | Сопровождение | Безопасность / заметки | Альтернативы | Рекомендация |
|-------|-----------|---------------|----------|---------------|------------------------|--------------|--------------|
| **`windows`** | Win32 + COM (UIA, буфер, ввод, хоткеи, AppContainer, Job) | **0.62.2 (2025-10-06)**; новее на crates.io нет | MIT OR Apache-2.0 | Microsoft (microsoft/windows-rs), ~108 млн загрузок за 90 дн. | Уже в графе через Tauri — **без нового кода**. Источник крейта ~9 МБ, компилируются только включённые features. В master репозитория `windows` уже версии 0.100.0 и features переименованы в «группы заголовков» (`winuser`, `handleapi`…) [SRC: `crates/libs/windows/Cargo.toml`, readme в master]; на crates.io из 0.100 опубликованы только `windows-core`, `windows-version`, `windows-bindgen` (2026-09-03). → ожидается ломающая миграция. Все нужные сигнатуры проверены компиляцией [FR §9.1]. | `windows-sys` (нет COM-обёрток — UIA неудобен); `windows-bindgen` (приватные сгенерированные биндинги, минимальный код) | **Да**, `=0.62.2`, только в `ks-platform-win`, минимальный набор features [FR §9.1]. Миграция на 0.100 — отдельной задачей после её выхода и 2 недель. |
| `windows-sys` | Сырые Win32 FFI | 0.61.2 (2025-10-06) | MIT OR Apache-2.0 | Microsoft | Уже в графе. Без COM. | — | Нет (не нужен поверх `windows`). |
| `windows-bindgen` | Генерация своих биндингов | 0.100.0 (2026-09-03) / 0.66.0 (2026-01-08) | MIT OR Apache-2.0 | Microsoft | Build-time инструмент; уменьшает поверхность и время компиляции. | — | Не сейчас; рассмотреть при миграции R-A12. |
| `uiautomation` | Обёртка над UIA | 0.25.1 (2026-09-04) | Apache-2.0 | Один автор (leexgone/uiautomation-rs), ~0,6 млн загрузок за 90 дн. | Зависит от `windows ^0.62.2`, `windows-core`, `chrono`, `uiautomation_derive` (proc-macro). Скрывает обработку ошибок/таймаутов, которые для нас критичны; bus factor = 1. Нам нужно ~30 вызовов UIA. | сырой `windows` | **Нет.** Использовать `windows` напрямую. |
| `webview2-com` | WebView2 для тестовой фикстуры | 0.39.1 (в lock) | MIT | tauri-apps/wry экосистема | Уже в графе. Только dev-фикстура S-2. | Electron-фикстура | Только dev (S-2), если фикстура — своё окно WebView2. |
| `global-hotkey` | Глобальные хоткеи | 0.9.0 (2026-10-16) | Apache-2.0 OR MIT | tauri-apps | На Windows — `RegisterHotKey` на скрытом окне, всегда `MOD_NOREPEAT` [SRC: `src/platform_impl/windows/mod.rs`]; зависит от `windows-sys ^0.59` (вторая копия windows-sys), `keyboard-types`, `crossbeam-channel`. | своя реализация (~150 строк) | **Нет.** Своя реализация в потоке T1 [ARCH §4]. |
| `clipboard-win` / `arboard` | Буфер обмена | 5.4.3 (2026-10-23) BSL-1.0 / 3.7.0 (2026-10-15) MIT OR Apache-2.0 | см. | DoumanAsh / 1Password | Не поддерживают delayed rendering как «сигнал потребления», snapshot всех форматов и форматы D-13 в нужной нам форме; BSL-1.0 нет в allow-list `deny.toml`. | свой код на `windows` | **Нет.** |
| `interprocess` | Pipes/IPC | 2.4.7 (2026-10-23); 2.4.8 (<2 нед.) | 0BSD OR Apache-2.0 | один автор | Нам нужен `CreateProcess` с AppContainer-атрибутами — всё равно через `windows`; анонимные pipe — `CreatePipe`, оборачиваются в `std::fs::File`. | std + `windows` | **Нет.** |

## 2. Rust — ядро, движок, хранение

| Крейт | Назначение | Версия (дата) | Лицензия | Сопровождение | Заметки | Альтернативы | Рекомендация |
|-------|-----------|---------------|----------|---------------|---------|--------------|--------------|
| **`zeroize`** | Затирание `UserText` (T-09) | 1.9.1 (2026-10-06) | Apache-2.0 OR MIT | RustCrypto, ~248 млн загрузок за 90 дн. | Без `unsafe` в нашем коде, no I/O, совместим с правилами `ks-types`. | `secrecy` 0.10.3 (2024-10-09) — обёртка над zeroize, лишний слой | **Да** (`ks-types`). |
| `serde` / `serde_json` | Протокол песочницы, настройки JSON, профили | 1.0.229 (2026-07-18) / 1.0.154 (2026-10-11) | MIT OR Apache-2.0 | dtolnay | Уже в графе через Tauri. `deny_unknown_fields` в протоколе. | `postcard` 1.1.3, `ciborium` 0.2.2 — компактнее, но новая зависимость и хуже отладка | **Да** (уже есть). |
| `toml` | Профили в TOML (черновик v1) | 1.1.8 (2026-10-09); 1.2.x (<2 нед.) | MIT OR Apache-2.0 | toml-rs | Новая зависимость ради формата. | JSON (`serde_json` уже есть) | **Нет** — профили в JSON [ARCH C-6]. |
| `thiserror` | Типы ошибок | 2.0.21 (2026-09-23) | MIT OR Apache-2.0 | dtolnay | Уже в графе (Tauri). proc-macro. | ручные `impl Display` | Опционально; в `ks-core` не добавлять (там ошибки уже ручные). |
| **`rusqlite`** (`bundled`) | SQLite | **0.40.2 (2026-08-08)**; 0.41.0 (2026-10-24, 16 дн.), 0.41.1 (<2 нед.) | MIT | rusqlite org, ~58 млн загрузок за 90 дн. | `bundled` → `libsqlite3-sys` 0.38.2 (2026-08-08) компилирует SQLite из исходников (нужен MSVC в CI — есть). Синхронный API — подходит (операции маленькие). Версия встроенного SQLite — UNVERIFIED (проверить в changelog `libsqlite3-sys` при добавлении). Выход 0.41.1 через 4 дня после 0.41.0 указывает на исправления → пока брать зрелую 0.40.2. | `sqlx` 0.9.0 (2026-05-21; async, макросы, тянет tokio/драйверы — избыточно) | **Да**, `rusqlite = { version = "=0.40.2", features = ["bundled"] }`. |
| `rusqlite_migration` | Миграции | 2.6.0 (2026-05-28) | Apache-2.0 | один автор | Удобно, но 3–5 миграций проще написать через `PRAGMA user_version`. | своё | **Нет.** |
| `sha2` | Проверка SHA-256 wasm-модулей (T-13) | 0.11.1 (2026-10-12) | MIT OR Apache-2.0 | RustCrypto | Чистый Rust, маленький. | Windows CNG (`BCryptHash`) через `windows` — без зависимости, но песочница/хост тогда Windows-only | **Да** (`ks-sandbox-host`, проверка в app при установке/запуске). |
| `log` | Фасад логирования | 0.4.34 (2026-08-22); 0.4.35 (<2 нед.) | MIT OR Apache-2.0 | rust-lang | Уже в графе. Логи — только метаданные; текст физически не может попасть (тип `UserText` без `Debug`/`Display`). | `tracing` 0.1.44 + `tracing-subscriber` 0.3.23 — мощнее, тяжелее | **Да** (`log`), без файлового логгера по умолчанию; диагностика — в SQLite. |
| `directories` / `dirs` | Пути `%LOCALAPPDATA%` | 6.0.0 (2025-01-12) / 7.0.0 (2026-09-05) | MIT OR Apache-2.0 | | Tauri уже даёт `app_local_data_dir()`. | Tauri path API | **Нет.** |

## 3. Rust — песочница макросов

| Компонент | Назначение | Версия (дата) | Лицензия | Сопровождение | Заметки | Альтернативы | Рекомендация |
|-----------|-----------|---------------|----------|---------------|---------|--------------|--------------|
| **`wasmtime`** | Wasm-рантайм (D-08) | **LTS 48.0.7 (2026-10-19)**; последняя 50.0.0 (2026-10-20); 50.0.1, 49.0.5, 48.0.8 — 2026-10-29 (<2 нед.) | Apache-2.0 WITH LLVM-exception | Bytecode Alliance; мажор ежемесячно (ветка 5-го, релиз 20-го); **каждый релиз, кратный 12, — LTS с поддержкой 24 мес.**, остальные — 2 мес.; security-фиксы бэкпортятся во все поддерживаемые [DOC: [stability-release.md](https://github.com/bytecodealliance/wasmtime/blob/main/docs/stability-release.md)]. `x86_64-pc-windows-msvc` — Tier 1; Cranelift и Winch — Tier 1; Pulley — Tier 2 [DOC: [stability-tiers.md](https://github.com/bytecodealliance/wasmtime/blob/main/docs/stability-tiers.md)]. MSRV 48.x = 1.95 (у нас 1.97). | Одновременный выпуск 48.0.8/49.0.5/50.0.1 2026-10-29 похож на security-патч → **до добавления проверить advisory**; если это security-фикс, он конфликтует с правилом «≥ 2 недель» → нужна процедура исключения (P-16). `default-features = false`, включить `runtime`, `cranelift` (или `pulley` по итогам S-3), `std`; не включать `cache`, `wat` (только dev), профилировщики. Вклад в бинарь — UNVERIFIED (S-3). | `wasmi` 2.0.0 (2026-09-01, MIT/Apache, интерпретатор — без JIT, совместим с ACG, но D-08 фиксирует Wasmtime); `wasmtime` + Pulley | **Да**, линия **48 LTS** (`=48.0.7`, либо 48.0.8 после проверки advisory и одобрения исключения). |
| **`wasmtime-wasi`** | WASI p1 для `qjs-wasi.wasm` и `python.wasm` | 48.0.7 (2026-10-19); 48.0.8 (2026-10-29, по процедуре Q-3) | Apache-2.0 WITH LLVM-exception | там же | `WasiCtxBuilder` без сокетов/env и **без preopen** (P-12 отозван, решение CISO XR-07) [испр. 1]. Stdlib Python — из памяти (вариант A). Если в S-3 выбран собственный read-only слой WASI поверх дерева в памяти — `wasmtime-wasi` **удаляется** из зависимостей (меньше кода ФС: 5 из 9 advisory `wasmtime-wasi` 2026 — в коде ФС). | `wasi-common` 36.0.19 (2026-10-19) — устаревший путь; собственный слой WASI p1 | **Условно**: та же версия, что `wasmtime`, только если S-3 не выберет собственный слой. |
| `wasmtime-wizer` | Снимок инициализированного CPython (вариант A+) — **только CI/сборка**, не в продукте | 48.0.8 (2026-10-29), линия 48 LTS [проверено crates.io 2026-11-10] | Apache-2.0 WITH LLVM-exception | Bytecode Alliance | Снимок фиксирует PRNG/хэш-сид — обязателен пересев при старте инстанса (риск R-A23 [ARCH]) [испр. 1]. | `wizer` 10.0.0 (2025-08-27) — прежний отдельный проект; без снимка (медленнее старт) | **Да, если A+** (решение по замеру S-3); только build-dependency CI-инструмента. |
| **QuickJS (wasm)** | JS-движок внутри Wasm | quickjs-ng master: `QJS_VERSION 0.18.x` [SRC: `quickjs.h`]; дата релиза/ассетов — **UNVERIFIED** (GitHub API недоступен) | MIT | quickjs-ng (активный форк QuickJS) | Официальный release-workflow собирает `qjs-wasi.wasm` и `qjs-wasi-reactor.wasm` через **wasi-sdk 29** [SRC: [release.yml](https://github.com/quickjs-ng/quickjs/blob/master/.github/workflows/release.yml)]. | (B) `rquickjs` 0.14.0 (2026-09-18, MIT) — Rust-гость в `wasm32-wasip1` (нужен wasi-sdk для C-части); (C) `javy` 8.1.0 (2026-07-30, Bytecode Alliance) — тулчейн «JS → wasm» под каждый скрипт, сложнее для runtime-eval; (D) `boa_engine` 0.22.0 (2026-08-28, чистый Rust) — другой движок, расхождение с D-08 | **(A)** собирать `qjs-wasi-reactor.wasm` в нашем CI из закреплённого тега quickjs-ng тем же способом, хеш — в манифест. Финальный выбор A/B — в S-3. Вариант (B) `rquickjs` запрещён бан-листом SR-62 в основном workspace — допустим **только** как гостевой crate вне workspace (отдельный каталог, например `guest/`, со своим `Cargo.lock` и `deny.toml`), собираемый в `wasm32-wasip1` и встраиваемый как wasm-артефакт [испр. 13]. |
| **CPython (wasm)** | Python внутри Wasm | Python 3.14.x (последний патч ≥ 2 нед. на момент S-3 — дата UNVERIFIED); WASI 0.1, **WASI SDK 24**; SDK 26/27 пропущены из-за бага с зависаниями [DOC: [PEP 11](https://peps.python.org/pep-0011/) §WASI, [PEP 816](https://peps.python.org/pep-0816/)] | PSF-2.0 (бинарь + stdlib → `THIRD_PARTY.md`) | CPython core (WASI — Tier 2 с 3.13) | Официальных бинарей python.org для WASI **нет**; сборка — скриптом `Tools/wasm/wasi` в репозитории CPython [SRC]. Урезание stdlib (D-08): убрать `ctypes`, `socket`, `ssl`, `subprocess`, `multiprocessing`, `sqlite3` и т.п. — в WASI многие и так недоступны. Размер ~десятки МБ — UNVERIFIED (S-3). | (B) неофициальные сборки core-разработчика [WR: brettcannon/cpython-wasi-build] — допустимо **только** для спайка; (C) VMware Wasm Labs `webassembly-language-runtimes` — Python 3.12.0 (дек. 2023), репозиторий объявлен к архивации, развитие в форке [WR] — **нет**; (D) Pyodide — Emscripten, не WASI — **нет**; (E) `componentize-py` 0.25.1 (PyPI, 2026-09-12) — компонентная модель, иная задача — **нет** | **Собирать в CI** из тега CPython 3.14.x с WASI SDK 24; для S-3 допустимо начать с (B) с проверкой SHA-256. |

## 4. Rust — Tauri и плагины

| Крейт | Назначение | Версия (дата) | Лицензия | Заметки | Рекомендация |
|-------|-----------|---------------|----------|---------|--------------|
| `tauri` | Оболочка | **2.12.5 (2026-10-21)** (в lock) | Apache-2.0 OR MIT | Линия 3.0 в alpha с 2026-09 (3.0.0-alpha.6 от 2026-11-06) — **не использовать**. Нужна фича `tray-icon` (есть в списке features 2.12.5 [SRC: crates.io]). | **Да**, `2.12.5` + `features = ["tray-icon"]`. |
| `tauri-build` | build.rs | 2.7.4 (2026-10-21) | Apache-2.0 OR MIT | В Cargo.toml указано 2.7.3. | Да. |
| `tauri-plugin-single-instance` | Один экземпляр | **2.5.4 (2026-10-14)**; 2.5.5 (<2 нед.) | Apache-2.0 OR MIT | Windows: именованный mutex + `FindWindow` + `WM_COPYDATA` [SRC: plugins-workspace `v2/plugins/single-instance/src/platform_impl/windows.rs`]. Без JS-пакета (JS-API нет). | **Да**, `=2.5.4`. |
| `tauri-plugin-autostart` | Автозапуск | 2.7.0 (2026-10-01) | Apache-2.0 OR MIT | Через крейт `auto-launch` [SRC]; добавляет JS-API `@tauri-apps/plugin-autostart` и permissions. Нам нужна одна запись в HKCU Run. | **Нет** — свой `Autostart` (~40 строк в `ks-platform-win`). |
| `tauri-plugin-global-shortcut` | Хоткеи | 2.4.1 (2026-10-10) | Apache-2.0 OR MIT | Обёртка над `global-hotkey` (см. §1); JS-API. | **Нет** — свой поток `RegisterHotKey` (D-14, [ARCH §7.3]). |
| `tauri-plugin-window-state` | Позиция окна | 2.5.0 (2026-09-26) | Apache-2.0 OR MIT | Пишет файл в app dir; не критично. | Нет (не нужен для v1). |
| `tauri-plugin-fs/shell/http` | — | — | — | Запрещены CLAUDE.md п.2. | **Нет.** |

## 5. Frontend (npm)

Текущие: `react`/`react-dom` 19.2.0, `@tauri-apps/api` 2.12.3, `vite` 8.3.4, `tailwindcss` 4.3.0, `typescript` 6.0.3 — не пересматриваю.

| Пакет | Назначение | Версия (дата) | Лицензия | Заметки | Рекомендация |
|-------|-----------|---------------|----------|---------|--------------|
| `@codemirror/state` | Ядро CM6 | 6.7.6 (2026-09-22) | MIT | Автор — Marijn Haverbeke, активно. | **Да** |
| `@codemirror/view` | Ядро CM6 | 6.43.15 (2026-10-19) | MIT | unpacked ~1,26 МБ | **Да** |
| `@codemirror/commands` | Клавиатурные команды, history | 6.11.1 (2026-09-15) | MIT | | **Да** |
| `@codemirror/language` | Подсветка, отступы | 6.13.1 (2026-10-07) | MIT | | **Да** |
| `@codemirror/lang-javascript` | JS-синтаксис | 6.2.5 (2026-03-02) | MIT | | **Да** |
| `@codemirror/lang-python` | Python-синтаксис | 6.2.1 (2025-05-14) | MIT | Редкие релизы — нормально для грамматики. | **Да** |
| `@codemirror/lint` | Показ ошибок проверки синтаксиса (из песочницы) | 6.9.7 (2026-06-09) | MIT | Источник диагностик — команда `check_macro`. | **Да** |
| `@codemirror/search` | Поиск в редакторе | 6.7.2 (2026-08-31) | MIT | Полезно, не обязательно. | Опционально |
| `@codemirror/autocomplete` | Автодополнение API `ks.*` | 6.20.4 (2026-10-19) | MIT | Можно позже. | Опционально |
| `@lezer/highlight` | Теги подсветки для тем | 1.2.5 (2026-09-28) | MIT | Транзитивно у language. | Транзитивно |
| `codemirror` (мета-пакет) | «basicSetup» | `latest` = 6.0.2 (2025-06-19) | MIT | В реестре есть ошибочная версия `6.65.7` (2022-07-20) — semver-сортировка её поднимает; использовать только `dist-tags.latest`. Тянет всё сразу. | **Нет** — собирать набор явно. |
| `@uiw/react-codemirror` | React-обёртка | 4.25.12 (2026-09-24) | MIT | Лишний слой; обёртка на 50 строк своя. | **Нет** |
| `i18next` + `react-i18next` | i18n | 26.5.11 (2026-10-25) + 17.1.9 (2026-10-25); 27.0.1/18.0.1 (<2 нед.) | MIT | Мажоры вышли 7–8.11 — нестабильный момент. ~150 строк UI не оправдывают. | **Нет** — свой слой на `Intl.PluralRules` [ARCH §8.2]. |
| `@lingui/core` | i18n | 6.10.1 (2026-10-22); 6.12.0 (<2 нед.) | MIT | Требует компиляции каталогов. | Нет |
| `zustand` | Состояние | 5.0.16 (2026-10-22); 5.1.0 (<2 нед.) | MIT | Маленький, без зависимостей. | Не сейчас; запасной вариант. |
| `jotai` | Состояние | 3.0.1 (2026-09-29); 3.0.2 (<2 нед.) | MIT | | Нет |
| `@tauri-apps/api` | IPC | 2.12.4 (2026-10-14) | Apache-2.0 OR MIT | Сейчас 2.12.3; обновить вместе с CLI. | Да (есть) |
| `@tauri-apps/plugin-autostart`, `@tauri-apps/plugin-global-shortcut` | JS-API плагинов | 2.7.0 / 2.4.1 | MIT OR Apache-2.0 | Плагины не используем. | **Нет** |
| `@fontsource-variable/inter`, `@fontsource-variable/jetbrains-mono` | Шрифты (решение дизайнера) | 5.3.0 (2026-07-19) | **OFL-1.1** | Встраиваются в бандл (CSP `font-src 'self'`); OFL нужно добавить в `THIRD_PARTY.md`. | На усмотрение UI/UX Designer |
| `lucide-react` | Иконки (решение дизайнера) | 1.61.0 (2026-10-21); 1.70.0 (<2 нед.) | ISC | Частые релизы. | На усмотрение дизайнера; альтернатива — свои SVG |

## 6. Dev-only (CI-фикстуры S-2, не входят в продукт)

| Пакет | Назначение | Версия (дата) | Лицензия | Заметки |
|-------|-----------|---------------|----------|---------|
| `electron` (npm) | Electron-фикстура (поведение Discord/Slack-класса) | 44.9.3 (2026-10-21) или 45.0.0 (2026-10-21); 45.2.x (<2 нед.) | MIT | Скачивает бинарь при установке; только в job `windows-integration`. Версию Chromium внутри — проверить по release notes (UNVERIFIED). |
| `PySide6` (PyPI) | Qt6-фикстура (механизм Qt UIA, ближе всего к Telegram) | 6.12.0 (2026-10-08); 6.11.3 (2026-10-26, на границе) | LGPL-3.0-only OR GPL-2.0/3.0 | Не распространяется с продуктом; требует подтверждения (Q-B4). Устанавливать с `--require-hashes`. |
| Python 3.12 (на раннере) | Запуск Qt-фикстуры | 3.12.10 (в образе) | PSF-2.0 | Предустановлен [DOC: runner Windows2025-Readme]. |
| PowerShell + WPF | WPF-фикстура | PowerShell 7.6.9, .NET Framework 4.8.1 (в образе) | — | Без загрузок. |

## 7. Итог: предлагаемые новые прямые зависимости

| Где | Зависимость | Версия |
|-----|-------------|--------|
| `ks-types` | `zeroize` | =1.9.1 |
| `ks-types`, `ks-sandbox-proto` | `serde` (+derive), `serde_json` | уже в графе |
| `ks-platform-win` | `windows` (features по [FR §9.1]) | =0.62.2 (уже в графе) |
| `ks-storage` | `rusqlite` (`bundled`) | =0.40.2 |
| `ks-sandbox-host` | `wasmtime`, `wasmtime-wasi` (условно, [испр. 1]) | =48.0.8 (LTS, процедура Q-3; 48.0.7 уязвим — RUSTSEC-2026-0422/0423/0424 по `SECURITY_REVIEW.md` SEC-01) |
| CI (сборка wasm, вариант A+) | `wasmtime-wizer` | =48.0.8 |
| `ks-sandbox-host`, `keyswitch` | `sha2` | =0.11.1 |
| `keyswitch` | `tauri` (+`tray-icon`), `tauri-plugin-single-instance` | 2.12.5, =2.5.4 |
| `app` (npm) | `@codemirror/{state,view,commands,language,lang-javascript,lang-python,lint}` | см. §5 |
| артефакты CI | `qjs-wasi-reactor.wasm` (quickjs-ng тег), `python.wasm` 3.14.x + `python314.zip` (stdlib) | собираются в CI, встраиваются `include_bytes!` в `ks-sandbox.exe`, SHA-256 проверяется [испр. 1, 11] |

**Отказываемся:** `uiautomation`, `global-hotkey`/`tauri-plugin-global-shortcut`, `tauri-plugin-autostart`, `clipboard-win`/`arboard`, `interprocess`, `toml`, `sqlx`, `rusqlite_migration`, `i18next`/`react-i18next`, `@uiw/react-codemirror`, мета-пакет `codemirror`, `tracing`.

**Гостевые crate (код, исполняемый внутри Wasm) — вне основного workspace** [испр. 13]: отдельный каталог со своим `Cargo.lock`, `deny.toml` и CI-job; в продукт попадает только wasm-артефакт с проверенным SHA-256 (`include_bytes!` в `ks-sandbox.exe`, [ARCH §6.2]). Это снимает конфликт с бан-листом SR-62 (`rquickjs` и др. запрещены в workspace хоста).

Каждая из рекомендованных зависимостей при добавлении требует записи в PR (назначение, лицензия, риск) и зелёного `cargo deny` (CLAUDE.md п.3). Особо проверить в `cargo deny`: транзитивные лицензии графа Wasmtime (ожидаются Apache-2.0 WITH LLVM-exception / MIT / Apache-2.0 — UNVERIFIED до запуска).

## 8. Источники
- crates.io API, npm registry, PyPI JSON API — запросы 2026-11-09 (команды в §0).
- windows-rs master: https://github.com/microsoft/windows-rs/blob/master/crates/libs/windows/Cargo.toml , https://github.com/microsoft/windows-rs/blob/master/crates/libs/windows/readme.md
- Wasmtime: https://github.com/bytecodealliance/wasmtime/blob/main/docs/stability-release.md , https://github.com/bytecodealliance/wasmtime/blob/main/docs/stability-tiers.md , https://github.com/bytecodealliance/wasmtime/blob/main/docs/security.md
- quickjs-ng: https://github.com/quickjs-ng/quickjs/blob/master/.github/workflows/release.yml
- CPython WASI: https://peps.python.org/pep-0011/ , https://peps.python.org/pep-0816/ (исходники github.com/python/peps), https://github.com/python/cpython/tree/main/Tools/wasm/wasi
- Неофициальные сборки CPython WASI и VMware Wasm Labs — только выдача поиска (UNVERIFIED): https://discuss.python.org/t/wasi-releases-for-cpython-3-11-9-and-3-12-10/90257 , https://github.com/vmware-labs/webassembly-language-runtimes
- Tauri: https://github.com/tauri-apps/global-hotkey/blob/dev/src/platform_impl/windows/mod.rs , https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins (autostart, single-instance), https://v2.tauri.app/develop/calling-frontend/ (по исходнику tauri-docs)
