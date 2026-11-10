# LICENSE_REGISTER — реестр лицензий сторонних компонентов KeySwitch

Владелец документа: AGENT 12 CLO · Редакция 1 · Дата: **2026-11-10** · Ветка: `docs/governance-v3`
Статус: **первичный аудит при принятии проекта**. Не является юридическим заключением; CLO — AI, не лицензированный юрист (см. `docs/legal/legal-team-plan.md`, вопросы к юристу L-Q*).

Связано: `THIRD_PARTY.md` (текущая краткая таблица), `deny.toml`, `docs/research/arch-dependencies.md` ([DEP]), `docs/research/design-system.md` ([DS]), `docs/LEGAL_COMPLIANCE_REGISTER.md`, `docs/legal/name-and-ip-review.md`.

## 0. Метод (что проверено и как)

| Что | Как | Результат |
|---|---|---|
| Rust: полный граф | `cargo metadata --format-version 1 --locked --offline` → поле `license` каждого пакета с `source` (сторонний) | **403** сторонних пакета в `Cargo.lock` (все платформы) |
| Rust: что попадает в Windows-бинарь | `cargo tree -p keyswitch --target x86_64-pc-windows-msvc -e normal,no-proc-macro` | **130** пакетов (без proc-macro и build-зависимостей); ещё **89** — только время сборки на Windows; **184** — только другие платформы |
| Rust: политика | `cargo deny --offline check licenses` (2026-11-10) | **`licenses ok`** |
| Rust: файлы лицензий/NOTICE в пакетах | просмотр каталогов пакетов в `~/.cargo/registry` для runtime-набора | файлов `NOTICE` (Apache-2.0 §4(d)) **не найдено**; у `alloc-stdlib`, `selectors`, `webview2-com`, `webview2-com-sys`, `webview2-com-macros` **нет файла лицензии в пакете** — текст брать из upstream-репозитория при генерации notice-файла |
| npm | `app/package-lock.json`, поля `license` | **100** пакетов; в продуктовый бандл попадают только 4 (`react`, `react-dom`, `scheduler`, `@tauri-apps/api`) |
| Данные | `crates/ks-core/data/README.md`, `crates/ks-core/layouts/*.layout`, `crates/ks-core/src/lib.rs` | `.freq` и `.layout` **встраиваются в бинарь** через `include_str!` (`crates/ks-core/src/lib.rs:28-35`) |
| Планируемые компоненты | [DEP] §1–6, [DS] §шрифты/иконки | лицензии — по метаданным реестров из [DEP] (2026-11-09) и по памяти CLO; помечено UNVERIFIED, где текст лицензии не перечитан |

**Не проверено:** тексты лицензий не сверялись построчно с SPDX-идентификаторами (поле `license` — заявление автора пакета); точное содержимое итогового `.exe` (какие крейты реально слинкованы после LTO) — нужна генерация notice-файла инструментом (напр. `cargo-about`) в CI на Windows; содержимое NSIS-установщика и WebView2-загрузчика; лицензии npm-пакетов, встроенных Vite в `dist/` (проверено только по lock-файлу).

Легенда колонок: **Комм.** — коммерческое использование допустимо; **Атриб.** — обязанность указать авторство/лицензию (notice); **Исх.** — обязанность раскрыть исходный код; **Совм. P / OSS** — совместимость с проприетарным KeySwitch / с KeySwitch под открытой лицензией (MIT/Apache-2.0 как ориентир; для GPL-сценария см. §6).

---

## 1. Сводка красных флагов

| # | Флаг | Серьёзность | Действие | Срок |
|---|---|---|---|---|
| LF-1 | **CC-BY-SA-4.0 данные `en.freq`/`ru.freq` встроены в бинарь** (`include_str!`). Share-alike распространяется на адаптированную базу данных (CC BY-SA 4.0 §4(b)); запрет «дополнительных ограничений» и технических мер защиты (§2(a)(5)(C)) — EULA и любой DRM не должны ограничивать эти данные. Происхождение корпуса (OpenSubtitles) не проверено. = риск **R-7** | **Высокая — блокирует релиз (GATE 11)** | Варианты — `docs/legal/name-and-ip-review.md` §5; решение владельца + юрист (L-Q4) | до GATE 9/11 |
| LF-2 | **У репозитория нет файла `LICENSE`**, в `Cargo.toml` — `LicenseRef-Proprietary`, репозиторий публичный (D-09). Правовой режим кода для посторонних не определён (по умолчанию «все права защищены», плюс права просмотра/форка по Условиям GitHub); вклад сторонних авторов не урегулирован (нет CLA/DCO) | Средняя (станет Высокой при первом внешнем PR или выборе модели монетизации) | Решение MI-19 (лицензия кода); до решения — не принимать внешние PR с кодом; добавить явное уведомление | до GATE 3 (решение), до первого внешнего вклада |
| LF-3 | **Нет файла уведомлений (notice) для сторонних компонентов**, а неподписанный установщик уже публикуется как артефакт CI (`.github/workflows/ci.yml:69-74`, `keyswitch-windows-unsigned`, хранение 14 дней) в публичном репозитории = фактическое распространение бинаря. MIT/BSD/Apache/ISC/Unicode/MPL требуют передачи текстов лицензий и уведомлений вместе с бинарём | Средняя | Генерировать `THIRD_PARTY_NOTICES` в CI (Release Engineer) и включать в установщик и в «О программе»; до этого — пометка в README, что артефакты CI — для тестирования | до GATE 9; желательно раньше |
| LF-4 | **MPL-2.0 в бинаре: `option-ext@0.2.0`** (через `dirs`→`tauri`). Файловый копилефт: при распространении бинаря обязаны сообщить, где получить исходный код MPL-файлов (MPL-2.0 §3.2) | Низкая | Указать в notice-файле ссылку на исходник (crates.io / репозиторий) и текст MPL-2.0; не модифицировать файлы | GATE 9 |
| LF-5 | **CPython (PSF-2.0) и его stdlib** содержат компоненты под другими лицензиями; PSF-2.0 п.3 требует краткого описания изменений при создании производной работы (урезанная stdlib — изменение) | Средняя (при S-3/Macro Studio) | Сформировать перечень лицензий из `Doc/license.rst` выбранного тега CPython и wasi-libc/wasi-sdk; описание изменений stdlib | при добавлении (S-3) |
| LF-6 | **WebView2 Runtime / загрузчик** — проприетарный компонент Microsoft; установщик Tauri по умолчанию **скачивает** WebView2 bootstrapper при установке (сетевой вызов, противоречит PRIV-7 «без пингов в установщике», если не изменить режим) — **UNVERIFIED** для текущего `tauri.conf.json` (режим `webviewInstallMode` не задан) | Средняя (юридически — условия распространения MS; продуктово — заявление «без сети») | Release Engineer: проверить режим; CLO: проверить условия распространения WebView2 (Microsoft) | до GATE 9 |
| LF-7 | **Шрифт Segoe UI / Segoe UI Variable — проприетарный Microsoft**: допустимо только ссылаться на системный шрифт в CSS, **запрещено** включать файлы шрифта в установщик или в сайт | Низкая (сейчас соблюдено — [DS] использует системные шрифты) | Правило для дизайна/сайта: файлы Segoe не распространять; для сайта — web-safe fallback или OFL-шрифт | постоянно |
| LF-8 | **PySide6 (LGPL-3.0/GPL) и Qt** — только тестовая фикстура CI; станет распространением, если попадёт в публичный артефакт CI | Низкая | Не загружать фикстуру/окружение PySide в артефакты; не включать в установщик | при S-2 |

Копилефт с сильным эффектом (GPL/AGPL) в текущем Windows-графе **не обнаружен**. LGPL встречается только как альтернатива в `r-efi` (UEFI, не Windows; выбирается MIT) и в тестовой фикстуре PySide6.

---

## 2. Rust-зависимости (текущие, из `Cargo.lock`)

### 2.1 Группировка по лицензиям — что попадает в Windows-бинарь (130 пакетов)

| Лицензия | Кол-во | Комм. | Атриб. | Исх. | Совм. P / OSS | Требуемое действие |
|---|---|---|---|---|---|---|
| `MIT OR Apache-2.0` / `Apache-2.0 OR MIT` | 86 | Да | Да (текст MIT или Apache-2.0 + копирайты) | Нет | Да / Да | Выбрать MIT (проще), включить тексты в notice |
| `MIT` | 15 | Да | Да | Нет | Да / Да | Notice |
| `Unicode-3.0` (ICU4X: `icu_*`, `zerovec`, `yoke`, `tinystr`…) | 15 | Да | Да — уведомление Unicode в документации | Нет | Да / Да | Включить текст Unicode License v3 |
| `Unlicense OR MIT` (`memchr`, `aho-corasick`, `walkdir`…) | 5 | Да | Выбрать MIT → notice (Unlicense — без обязательств, но в ряде стран ЕС отказ от авторских прав неэффективен — выбирать MIT) | Нет | Да / Да | Notice (MIT) |
| `BSD-3-Clause`, `BSD-3-Clause AND MIT`, `BSD-3-Clause OR MIT` (`brotli`, `alloc-*`) | 4 | Да | Да (уведомление в документации; запрет использовать имена авторов для продвижения) | Нет | Да / Да | Notice; не упоминать Dropbox в маркетинге |
| `Apache-2.0` (`tao`), `Apache-2.0 AND MIT` (`dpi`) | 2 | Да | Да; NOTICE-файлов нет (проверено) | Нет | Да / Да; **несовместимо с GPL-2.0-only** | Notice; сохранить уведомления об изменениях, если будем модифицировать |
| `CC0-1.0 OR MIT-0 OR Apache-2.0` (`dunce`), `MIT OR Apache-2.0 OR Zlib` (`raw-window-handle`) | 2 | Да | Нет (CC0/MIT-0/Zlib-бинарь) | Нет | Да / Да | Перечислить в notice (добросовестно) |
| **`MPL-2.0` (`option-ext@0.2.0`)** | **1** | Да | Да | **Да, но только для файлов MPL** (сообщить, где взять исходник) | Да / Да (MPL совместим с GPL через §3.3) | См. LF-4 |

Полный поимённый список — Приложение А.

### 2.2 Только время сборки (89 пакетов) и другие платформы (184 пакета)

- **Build-time (proc-macro, `build.rs`)** — в бинарь не входят, не распространяются → обязательств по распространению нет. Сюда относятся MPL-2.0 крейты `cssparser`, `cssparser-macros`, `dtoa-short`, `selectors` (через `tauri-codegen`/`tauri-macros` → `dom_query`), а также `static_vcruntime` (статическая линковка MSVC CRT — см. §4, MSVC).
  Оговорка: принадлежность к build-time определена по графу `cargo tree`; окончательно подтверждается генерацией notice-файла по фактической сборке (UNVERIFIED).
- **Другие платформы** (GTK/WebKitGTK-биндинги для Linux, `objc2-*` для macOS, `r-efi`, `jiff`, `wasi`…) — в Windows-сборку не входят. **Для будущей macOS/Linux-версии** — отдельная ревизия: Linux-сборка динамически линкует системные GTK3/WebKitGTK (**LGPL-2.1+**) — допустимо для проприетарного ПО при динамической линковке, но требует уведомления и ревизии; `r-efi` — выбрать MIT.

### 2.3 Прямые зависимости проекта

| Компонент | Версия (lock) | Лицензия | Комм. | Атриб. | Исх. | Совм. P / OSS | Действие |
|---|---|---|---|---|---|---|---|
| `tauri` (+`tauri-runtime`, `tauri-runtime-wry`, `tauri-utils`) | 2.12.5 | Apache-2.0 OR MIT | Да | Да | Нет | Да / Да | Notice. Имя/логотип «Tauri» — товарные знаки проекта Tauri: не использовать в нашем брендинге (иконка приложения своя — проверено визуально, не логотип Tauri) |
| `wry`, `tao`, `muda`, `webview2-com(-sys)` | 0.57.1, 0.37.2, 0.20.0, 0.39.1 | Apache-2.0 OR MIT; `tao` — Apache-2.0; `webview2-com*` — MIT | Да | Да | Нет | Да / Да | Notice; `webview2-com-sys` — проверить, не включает ли он заголовки/библиотеки WebView2 SDK под лицензией Microsoft (UNVERIFIED) |
| `windows`, `windows-core`, `windows-sys` | 0.62.2 | MIT OR Apache-2.0 | Да | Да | Нет | Да / Да | Notice |
| `tauri-build` (build) | 2.7.4 | Apache-2.0 OR MIT | Да | Нет (не распространяется) | Нет | Да / Да | — |
| `ks-core` (собственный) | 0.1.0 | `LicenseRef-Proprietary` | — | — | — | — | См. LF-2 |

---

## 3. npm-пакеты (`app/package-lock.json`, 100 пакетов)

| Группа | Пакеты | Лицензия | Распространяется? | Комм. | Атриб. | Исх. | Действие |
|---|---|---|---|---|---|---|---|
| Продуктовый бандл (`dist/`, встраивается в `.exe`) | `react@19.2.0`, `react-dom@19.2.0`, `scheduler`, `@tauri-apps/api@2.12.3` | MIT; Apache-2.0 OR MIT | **Да** | Да | Да | Нет | Notice (тексты MIT); проверить, что Vite сохраняет или выносит legal-комментарии |
| Инструменты сборки (dev) | `vite`, `@vitejs/plugin-react`, `tailwindcss`, `@tailwindcss/vite`, `typescript` (Apache-2.0), `@tauri-apps/cli` (Apache-2.0 OR MIT), `source-map-js` (BSD-3-Clause), `graceful-fs`, `picocolors` (ISC), `detect-libc` (Apache-2.0) и др.; всего 58 MIT, 13 Apache-2.0 OR MIT, 2 Apache-2.0, 2 ISC, 1 BSD-3-Clause | разные пермиссивные | Нет | Да | Нет (не распространяются) | Нет | — |
| **`lightningcss` + платформенные бинарники (24 записи, 1.32.0 и 1.33.0)** | MPL-2.0 | Нет (dev-инструмент CSS) | Да | Нет | Нет (используется, не распространяется) | Мониторинг: Tailwind-рантайм в бандл не попадает; если когда-либо встроим lightningcss в продукт — MPL §3.2 |

**Наблюдение:** две версии `lightningcss` (1.32.0 и 1.33.0) в lock-файле — не юридическая проблема; передано Frontend/Release для гигиены.

---

## 4. Данные, ресурсы, инструменты распространения

| Компонент | Где | Лицензия | Комм. | Атриб. | Исх. / share-alike | Совм. P / OSS | Действие |
|---|---|---|---|---|---|---|---|
| **FrequencyWords (Hermit Dave), OpenSubtitles 2018** → `en.freq`, `ru.freq` (топ-30 000 слов) | `crates/ks-core/data/`, **встроено в бинарь** | **CC-BY-SA-4.0** (контент репозитория FrequencyWords; код репозитория — MIT) — по `THIRD_PARTY.md`, текст лицензии upstream в сессии не перечитан (UNVERIFIED) | Да | **Да**: автор, название, ссылка на лицензию, указание на изменения (фильтрация, усечение до 30 000) — CC BY-SA 4.0 §3(a) | **Share-alike** на адаптированную базу (§3(b), §4(b)); **запрет доп. ограничений/DRM** (§2(a)(5)(C)) | P: да, если данные изолированы и лицензированы отдельно под BY-SA; OSS: да (BY-SA односторонне совместим с GPLv3 — для кода не нужен) | **LF-1 / R-7** — см. `docs/legal/name-and-ip-review.md` §5 |
| Происхождение корпуса OpenSubtitles | upstream | Права на сами субтитры у правообладателей фильмов/переводчиков; частотный список отдельных слов вряд ли воспроизводит охраняемые произведения, но **права на базу данных sui generis** (Директива 96/9/EC) у составителя списка возможны | — | — | — | — | Вопрос юристу L-Q4 (низко-средний риск) |
| Таблицы раскладок `en-us.layout`, `ru.layout` | `crates/ks-core/layouts/`, встроено в бинарь | Собственные файлы KeySwitch; содержат факты (соответствие скан-код ↔ символ) раскладок Microsoft «US» (00000409) и «Russian» (00000419). Факты и стандартные раскладки (ЙЦУКЕН, QWERTY) не охраняются авторским правом как таковые; файлы Microsoft (KBD*.DLL, MSKLC) **не копировались** (по комментариям в файлах) | Да | Нет | Нет | Да / Да | Правило для `/add-layout`: описывать раскладку по наблюдаемым фактам/стандартам (ГОСТ, DIN и т.п.), не копировать файлы `.klc`/DLL и чужие таблицы с лицензией; записывать источник в заголовке файла |
| Иконка приложения `app/src-tauri/icons/*` | бандл | Собственная (сгенерирована в скаффолде S-4, коммит `578d643`; визуально — стрелка в рамке, не логотип Tauri) | — | — | — | — | Авторско-правовой статус сгенерированного ИИ изображения сомнителен (см. name-and-ip-review §4); перед регистрацией знака/логотипа — дизайн человеком |
| **NSIS** (установщик Tauri, `bundle.targets = ["nsis"]`) | инструмент сборки; стаб NSIS входит в `.exe` установщика | zlib/libpng-подобная лицензия NSIS; части — bzip2 (BSD-подобная), плагины Tauri NSIS (MIT/Apache) — UNVERIFIED | Да | Да для некоторых частей (bzip2 — если используется) | Нет | Да / Да | Release Engineer: включить NSIS-уведомления в notice |
| **MSVC CRT** (статическая линковка через `static_vcruntime`) | build | Microsoft Visual Studio / Build Tools — «Distributable Code» разрешает распространение runtime; использование Build Tools на раннерах GitHub — по лицензии образа раннера | Да (UNVERIFIED) | Нет | Нет | Да / Да | Вопрос низкого приоритета: подтвердить условия лицензии VS Build Tools для коммерческой сборки (L-Q9) |
| **WebView2 Runtime / bootstrapper** | у пользователя (Windows 11 — предустановлен; Windows 10 — устанавливается) | Проприетарная Microsoft, условия распространения WebView2 | Да | По условиям MS | Нет | Да / Да | **LF-6** |
| Системные шрифты Segoe UI (Variable), Consolas — используются по имени в CSS | ОС пользователя | Проприетарные Microsoft | Да (как системные) | Нет | Нет | Да / Да | **LF-7**: файлы не распространять |
| Cascadia Mono / Cascadia Code — по имени в CSS | ОС пользователя | SIL OFL-1.1 (Microsoft) | Да | При распространении файлов — да | Нет | Да / Да | Не встраиваем → обязательств нет |

---

## 5. Планируемые компоненты ([DEP], [DS]) — оценка до добавления

| Компонент | Версия (план) | Лицензия | Комм. | Атриб. | Исх. | Совм. P / OSS | Требуемое действие |
|---|---|---|---|---|---|---|---|
| `wasmtime`, `wasmtime-wasi` (+ Cranelift) | =48.0.7 (LTS) | Apache-2.0 WITH LLVM-exception | Да | Да (Apache-2.0 §4; LLVM-exception снимает требование атрибуции для **вкомпилированных** частей из исходной формы, но мы распространяем сам рантайм → атрибуция нужна) | Нет | Да / Да (с LLVM-exception — совместимо и с GPL-2.0) | Notice; транзитивный граф проверить `cargo deny` (UNVERIFIED) |
| QuickJS-ng (`qjs-wasi-reactor.wasm`, собирается в CI) | тег quickjs-ng 0.18.x | MIT (QuickJS и quickjs-ng; включает libregexp/libunicode/dtoa — MIT; данные Unicode — Unicode License) — UNVERIFIED | Да | Да | Нет | Да / Да | Notice (MIT + Unicode); wasi-sdk/wasi-libc (Apache-2.0 WITH LLVM-exception, MIT, частично CC0/BSD) — включить уведомления для вкомпилированного libc |
| CPython `python.wasm` 3.14.x + урезанная stdlib | собирается в CI | **PSF-2.0** + компоненты stdlib под иными лицензиями (напр. libmpdec — BSD-2-Clause, expat — MIT, zlib, HACL*/BLAKE2 — MIT/CC0 и т.п., по `Doc/license.rst`) — UNVERIFIED | Да | **Да** (PSF-2.0 п.2: сохранять уведомление PSF; п.3: краткое описание изменений) | Нет | Да / Да | **LF-5** |
| `rusqlite` (`bundled`) → `libsqlite3-sys` + SQLite | =0.40.2 | MIT (rusqlite, libsqlite3-sys); SQLite — **public domain** | Да | MIT — да; SQLite — нет | Нет | Да / Да | Notice для MIT; для SQLite — упоминание добросовестно (в ряде юрисдикций ЕС отказ от прав спорен; риск пренебрежимо мал) |
| `zeroize`, `sha2`, `serde`, `log` | см. [DEP] §7 | MIT OR Apache-2.0 / Apache-2.0 OR MIT | Да | Да | Нет | Да / Да | Notice |
| `tauri-plugin-single-instance` | =2.5.4 | Apache-2.0 OR MIT | Да | Да | Нет | Да / Да | Notice |
| CodeMirror 6 (`@codemirror/*`, `@lezer/*`) | см. [DEP] §5 | MIT | Да | Да | Нет | Да / Да | Notice |
| Lucide (`lucide-react` или копии SVG) | 1.61.0 | **ISC**; часть иконок происходит из Feather (MIT) — UNVERIFIED | Да | **Да**: при копировании SVG — сохранить файл лицензии Lucide (и Feather) | Нет | Да / Да | Notice; не использовать иконки, изображающие чужие логотипы (бренд-иконки Lucide удалены upstream — проверить) |
| `@fontsource-variable/inter`, `@fontsource-variable/jetbrains-mono` (если дизайнер выберет) | 5.3.0 | **SIL OFL-1.1** (шрифты); пакет-обёртка — MIT | Да | Да (текст OFL с файлами шрифта) | Нет | Да / Да; шрифт нельзя **продавать отдельно** | Включить OFL-1.1; не переименовывать шрифт при модификации (Reserved Font Name) |
| PySide6 / Qt 6 (CI-фикстура S-2) | 6.12.0 | LGPL-3.0-only OR GPL-2.0/3.0 (Qt: LGPL-3.0 / коммерч.) | Да | При распространении — да | При распространении — исходники/возможность замены | Не распространяется → не влияет | **LF-8**; подтверждение Q-B4 — «да» с условием: только CI, без публикации в артефактах |
| Electron (CI-фикстура) | 44.9.3 / 45.0.0 | MIT + Chromium (BSD-3 и множество др.) + FFmpeg (LGPL) | Да | При распространении — да | При распространении — частично | Не распространяется | Не публиковать в артефактах |
| Python 3.12 на раннере, PowerShell/WPF | образ раннера | PSF-2.0 / MIT / Microsoft | — | — | — | — | Не распространяется |

---

## 6. Совместимость с моделями лицензирования KeySwitch (MI-19)

| Модель KeySwitch | Совместимость текущего и планируемого набора | Что потребуется |
|---|---|---|
| Проприетарная (закрытая или «source-available») | **Совместимо**: всё пермиссивно; MPL-2.0 — файловый копилефт (только исходники MPL-файлов); CC-BY-SA — только при изоляции данных под своей лицензией и без ограничений на данные | Notice-файл; раздел «Сторонние компоненты» в «О программе»; изоляция данных (R-7); EULA, не ограничивающая сторонние компоненты сверх их лицензий |
| Пермиссивная OSS (MIT / Apache-2.0) | **Совместимо** | То же; Apache-2.0 даёт явную патентную лицензию — рекомендуется рассмотреть Apache-2.0 OR MIT как стандарт экосистемы Rust |
| Open core (ядро OSS + платные модули) | **Совместимо** | Чёткая граница модулей; CLA/DCO для внешних вкладов; товарный знак — отдельно |
| Copyleft (GPL-3.0 / AGPL) + коммерческая лицензия | Совместимо (Apache-2.0, MPL-2.0 совместимы с GPL-3.0; **Apache-2.0 несовместим с GPL-2.0-only** — не выбирать GPL-2.0-only) | Обязателен CLA с передачей/лицензированием прав, иначе двойное лицензирование невозможно |

Вывод CLO: **ни одна из текущих зависимостей не блокирует ни одну из моделей**; блокирующим фактором является только R-7 (данные) и отсутствие решения MI-19.

---

## 7. Требуемые действия (передаются Project Director)

| ID | Действие | Ответственный | Проверяет | Срок |
|---|---|---|---|---|
| LA-1 | Решение по R-7 (варианты в name-and-ip-review §5) | Владелец (по рекомендации CLO) | CLO + юрист (L-Q4) | до GATE 9 |
| LA-2 | Решение MI-19 о лицензии кода; до решения — добавить в README явный статус «All rights reserved, внешние PR не принимаются» (текст готовит CLO) | Владелец / Project Director | CLO | до GATE 3 |
| LA-3 | Генерация `THIRD_PARTY_NOTICES` (Rust + npm + данные + NSIS + wasm-рантаймы) в CI из фактической Windows-сборки; включение в установщик и окно «О программе» | AGENT 09 Release | CLO | до GATE 9 |
| LA-4 | Ограничить публичность артефакта `keyswitch-windows-unsigned` или сопроводить его notice-файлом и пометкой «тестовая сборка» | AGENT 09 | CLO | при следующем изменении CI |
| LA-5 | `deny.toml`: сохранить allow-list; при добавлении CPython добавить `PSF-2.0` только для конкретных артефактов (они не крейты — учёт вручную в этом реестре); **не добавлять** в allow-list GPL/AGPL/LGPL/SSPL/BUSL/CC-BY-NC без решения CLO | AGENT 03 / 09 | CLO | постоянно |
| LA-6 | Проверить `webviewInstallMode` и условия распространения WebView2 | AGENT 09 + CLO | CISO (сеть в установщике) | до GATE 9 |
| LA-7 | При каждом новом PR с зависимостью — обновлять этот реестр (п. 3 CLAUDE.md: назначение, лицензия, риск) | автор PR | CLO (legal review по триггерам, legal-team-plan §4) | постоянно |

## 8. История изменений
| Дата | Изменение | Автор |
|---|---|---|
| 2026-11-10 | Первая редакция: 403 Rust-пакета (130 в бинаре), 100 npm, данные, планируемые компоненты | AGENT 12 CLO |

---

## Приложение А. Поимённый список Rust-пакетов по лицензиям (сгенерировано 2026-11-10)

Источник: `cargo metadata` + `cargo tree --target x86_64-pc-windows-msvc -e normal,no-proc-macro` по `Cargo.lock` ветки `docs/governance-v3`. Перегенерировать при каждом изменении `Cargo.lock`.

#### A. Попадает в Windows-бинарь (runtime-граф `keyswitch`, без proc-macro и build-зависимостей) — 130 шт.

| Лицензия (SPDX, нормализовано) | Кол-во | Пакеты |
|---|---|---|
| `MIT OR Apache-2.0` | 70 | anyhow@1.0.104, base64@0.23.1, bitflags@2.13.2, cfg-if@1.0.5, cookie@0.18.2, crossbeam-channel@0.5.17, crossbeam-utils@0.8.23, deranged@0.5.9, dirs-sys@0.5.0, dirs@7.0.0, erased-serde@0.4.11, form_urlencoded@1.2.2, getrandom@0.3.4, glob@0.3.4, hashbrown@0.17.1, heck@0.5.0, http@1.5.0, idna@1.1.0, itoa@1.0.19, json-patch@4.2.0, jsonptr@0.7.1, keyboard-types@0.8.3, libc@0.2.191, lock_api@0.4.14, log@0.4.35, mime@0.3.18, num-conv@0.2.2, once_cell@1.21.4, parking_lot@0.12.5, parking_lot_core@0.9.12, percent-encoding@2.3.2, powerfmt@0.2.1, regex-automata@0.4.18, regex-syntax@0.8.11, regex@1.13.1, scopeguard@1.2.0, semver@1.0.28, serde-untagged@0.1.9, serde@1.0.229, serde_core@1.0.229, serde_json@1.0.154, serde_spanned@1.1.3, serde_with@3.24.0, serialize-to-javascript@0.1.2, siphasher@1.0.4, smallvec@1.16.5, softbuffer@0.4.8, stable_deref_trait@1.2.1, thiserror@2.0.21, time-core@0.1.9, time@0.3.55, toml@1.2.1+spec-1.1.0, toml_datetime@1.1.3+spec-1.1.0, toml_parser@1.1.5+spec-1.1.0, toml_writer@1.2.0+spec-1.1.0, typeid@1.0.3, unicode-segmentation@1.13.3, url@2.5.8, web-time@1.1.0, windows-collections@0.3.2, windows-core@0.62.2, windows-future@0.3.2, windows-link@0.2.1, windows-numerics@0.3.1, windows-result@0.4.1, windows-strings@0.5.1, windows-sys@0.61.2, windows-threading@0.2.1, windows-version@0.1.7, windows@0.62.2 |
| `Apache-2.0 OR MIT` | 16 | ctor@1.0.13, equivalent@1.0.3, fnv@1.0.7, idna_adapter@1.2.2, indexmap@2.14.2, muda@0.20.0, pin-project-lite@0.2.17, tauri-runtime-wry@2.12.3, tauri-runtime@2.12.2, tauri-utils@2.10.2, tauri@2.12.5, utf8_iter@1.1.0, utf_types@1.0.0, uuid@1.29.2, window-vibrancy@0.8.1, wry@0.57.1 |
| `MIT` | 15 | bytes@1.12.1, cfb@0.14.0, infer@0.22.0, phf@0.13.1, phf_shared@0.13.1, plist@1.10.3, quick-xml@0.42.0, tokio@1.54.1, tracing-core@0.1.36, tracing@0.1.44, urlpattern@0.6.0, webview2-com-sys@0.39.1, webview2-com@0.39.1, winnow@1.0.5, zmij@1.0.23 |
| `Unicode-3.0` | 15 | icu_collections@2.4.0, icu_locale_core@2.4.0, icu_normalizer@2.4.0, icu_normalizer_data@2.4.0, icu_properties@2.4.0, icu_properties_data@2.4.0, icu_provider@2.4.0, litemap@0.8.3, potential_utf@0.1.6, tinystr@0.8.5, writeable@0.6.4, yoke@0.8.4, zerofrom@0.1.8, zerotrie@0.2.5, zerovec@0.11.9 |
| `Unlicense OR MIT` | 5 | aho-corasick@1.1.5, memchr@2.8.3, same-file@1.0.6, walkdir@2.5.0, winapi-util@0.1.11 |
| `BSD-3-Clause` | 2 | alloc-no-stdlib@3.0.0, alloc-stdlib@0.3.0 |
| `BSD-3-Clause AND MIT` | 1 | brotli@9.1.1 |
| `BSD-3-Clause OR MIT` | 1 | brotli-decompressor@6.3.1 |
| `Apache-2.0 AND MIT` | 1 | dpi@0.1.2 |
| `CC0-1.0 OR MIT-0 OR Apache-2.0` | 1 | dunce@1.0.5 |
| `MPL-2.0` | 1 | option-ext@0.2.0 |
| `MIT OR Apache-2.0 OR Zlib` | 1 | raw-window-handle@0.6.2 |
| `Apache-2.0` | 1 | tao@0.37.2 |

#### B. Windows: только время сборки (proc-macro, build.rs) — в бинарь не входит, не распространяется — 89 шт.

| Лицензия (SPDX, нормализовано) | Кол-во | Пакеты |
|---|---|---|
| `MIT OR Apache-2.0` | 43 | block-buffer@0.10.4, camino@1.2.6, cargo-platform@0.1.9, cc@1.11.1, cpufeatures@0.2.17, crc32fast@1.5.4, crypto-common@0.1.7, digest@0.10.7, displaydoc@0.2.7, dtoa@1.0.11, dyn-clone@1.0.20, fdeflate@0.3.7, find-msvc-tools@0.1.15, flate2@1.1.11, getrandom@0.4.3, hashbrown@0.12.3, html5ever@0.39.0, ident_case@1.0.1, markup5ever@0.39.0, png@0.18.1, proc-macro2@1.0.107, quote@1.0.47, rustc_version@0.4.1, serde_derive@1.0.229, serde_derive_internals@0.29.1, serde_repr@0.1.21, serde_with_macros@3.24.0, serialize-to-javascript-impl@0.1.2, servo_arc@0.4.3, sha2@0.10.9, shlex@2.0.1, string_cache@0.9.0, string_cache_codegen@0.6.1, syn@2.0.119, syn@3.0.10, tendril@0.5.1, thiserror-impl@2.0.21, time-macros@0.2.32, typenum@1.20.1, version_check@0.9.5, web_atoms@0.2.6, windows-implement@0.60.2, windows-interface@0.59.3 |
| `MIT` | 22 | cargo_metadata@0.19.2, darling@0.24.1, darling_core@0.24.1, darling_macro@0.24.1, derive_more-impl@2.1.1, derive_more@2.1.1, dom_query@0.28.0, embed-resource@3.0.12, generic-array@0.14.7, ico@0.6.0, new_debug_unreachable@1.0.6, phf_codegen@0.13.1, phf_generator@0.13.1, phf_macros@0.13.1, precomputed-hash@0.1.1, schemars@0.8.22, schemars_derive@0.8.22, simd-adler32@0.3.10, strsim@0.11.1, synstructure@0.14.0, tauri-winres@0.3.7, webview2-com-macros@0.8.1 |
| `Apache-2.0 OR MIT` | 10 | autocfg@1.5.1, bit-set@0.8.0, bit-vec@0.8.0, cargo_toml@1.0.1, fastrand@2.5.0, indexmap@1.9.3, rustc-hash@2.1.3, tauri-build@2.7.4, tauri-codegen@2.7.4, tauri-macros@2.7.4 |
| `MPL-2.0` | 4 | cssparser-macros@0.7.1, cssparser@0.37.0, dtoa-short@0.3.5, selectors@0.38.0 |
| `Unicode-3.0` | 3 | yoke-derive@0.8.4, zerofrom-derive@0.1.8, zerovec-derive@0.11.7 |
| `MIT OR Zlib OR Apache-2.0` | 2 | miniz_oxide@0.8.9, miniz_oxide@0.9.1 |
| `0BSD OR MIT OR Apache-2.0` | 1 | adler2@2.0.1 |
| `Unlicense OR MIT` | 1 | byteorder@1.5.0 |
| `Zlib` | 1 | foldhash@0.2.0 |
| `MIT OR Apache-2.0 OR Zlib` | 1 | static_vcruntime@3.0.0 |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | 1 | unicode-ident@1.0.27 |

#### C. Только другие платформы (Linux/macOS/iOS/Android) — в Windows-сборку не входят — 184 шт.

| Лицензия (SPDX, нормализовано) | Кол-во | Пакеты |
|---|---|---|
| `MIT OR Apache-2.0` | 82 | android_system_properties@0.1.6, base64@0.21.7, bitflags@1.3.2, bs58@0.5.1, bumpalo@3.20.3, cfg-expr@0.15.8, chrono@0.4.45, core-foundation-sys@0.8.7, core-foundation@0.10.1, core-graphics-types@0.2.0, core-graphics@0.25.0, embed_plist@1.2.2, field-offset@0.3.6, foreign-types-macros@0.2.4, foreign-types-shared@0.3.1, foreign-types@0.5.0, futures-channel@0.3.34, futures-core@0.3.34, futures-executor@0.3.34, futures-io@0.3.34, futures-macro@0.3.34, futures-sink@0.3.34, futures-task@0.3.34, futures-util@0.3.34, heck@0.4.1, hex@0.4.3, httparse@1.10.1, iana-time-zone-haiku@0.1.2, iana-time-zone@0.1.65, ipnet@2.12.2, jni-sys-macros@0.4.1, jni-sys@0.3.1, jni-sys@0.4.1, jni@0.21.1, js-sys@0.3.107, ndk-context@0.1.1, ndk-sys@0.6.0+11769913, ndk@0.9.0, num-traits@0.2.19, pkg-config@0.3.34, proc-macro-crate@1.3.1, proc-macro-crate@2.0.2, proc-macro-crate@3.5.0, proc-macro-error-attr@1.0.4, proc-macro-error@1.0.4, ref-cast-impl@1.0.27, ref-cast@1.0.27, reqwest@0.13.5, rustversion@1.0.23, serde_spanned@0.6.9, socket2@0.6.5, swift-rs@1.0.9, syn@1.0.109, system-deps@6.2.2, tao-macros@0.1.4, thiserror-impl@1.0.69, thiserror@1.0.69, toml@0.8.2, toml_datetime@0.6.3, toml_edit@0.19.15, toml_edit@0.20.2, toml_edit@0.25.19+spec-1.1.0, tray-icon@0.25.1, wasm-bindgen-futures@0.4.80, wasm-bindgen-macro-support@0.2.130, wasm-bindgen-macro@0.2.130, wasm-bindgen-shared@0.2.130, wasm-bindgen@0.2.130, wasm-streams@0.5.0, web-sys@0.3.107, winapi-i686-pc-windows-gnu@0.4.0, winapi-x86_64-pc-windows-gnu@0.4.0, winapi@0.3.9, windows-sys@0.45.0, windows-targets@0.42.2, windows_aarch64_gnullvm@0.42.2, windows_aarch64_msvc@0.42.2, windows_i686_gnu@0.42.2, windows_i686_msvc@0.42.2, windows_x86_64_gnu@0.42.2, windows_x86_64_gnullvm@0.42.2, windows_x86_64_msvc@0.42.2 |
| `MIT` | 61 | atk-sys@0.18.2, atk@0.18.2, block2@0.6.2, cairo-rs@0.18.5, cairo-sys-rs@0.18.2, combine@4.6.9, dlopen2@0.8.2, dlopen2_derive@0.4.3, gdk-pixbuf-sys@0.18.0, gdk-pixbuf@0.18.5, gdk-sys@0.18.2, gdk@0.18.2, gdkwayland-sys@0.18.2, gdkx11-sys@0.18.2, gdkx11@0.18.2, gio-sys@0.18.1, gio@0.18.4, glib-macros@0.18.5, glib-sys@0.18.1, glib@0.18.5, gobject-sys@0.18.0, gtk-sys@0.18.2, gtk3-macros@0.18.2, gtk@0.18.2, http-body-util@0.1.5, http-body@1.1.0, hyper-util@0.1.21, hyper@1.12.0, javascriptcore-rs-sys@1.1.1, javascriptcore-rs@1.1.2, libredox@0.1.25, memoffset@0.9.1, mio@1.2.4, objc2-encode@4.1.0, objc2-foundation@0.3.2, objc2@0.6.5, pango-sys@0.18.0, pango@0.18.3, redox_syscall@0.5.18, redox_users@0.5.3, schemars@0.9.0, schemars@1.2.3, slab@0.4.12, soup3-sys@0.5.0, soup3@0.5.0, tokio-util@0.7.20, tower-http@0.6.11, tower-layer@0.3.3, tower-service@0.3.3, tower@0.5.3, try-lock@0.2.5, version-compare@0.2.1, vswhom-sys@0.1.3, vswhom@0.1.0, want@0.3.2, webkit2gtk-sys@2.0.2, webkit2gtk@2.0.2, winnow@0.5.40, winreg@0.56.0, x11-dl@2.21.0, x11@2.21.0 |
| `Zlib OR Apache-2.0 OR MIT` | 17 | bytemuck@1.25.2, dispatch2@0.3.1, objc2-app-kit@0.3.2, objc2-cloud-kit@0.3.2, objc2-core-data@0.3.2, objc2-core-foundation@0.3.2, objc2-core-graphics@0.3.2, objc2-core-image@0.3.2, objc2-core-location@0.3.2, objc2-core-text@0.3.2, objc2-exception-helper@0.1.1, objc2-io-surface@0.3.2, objc2-quartz-core@0.3.2, objc2-ui-kit@0.3.2, objc2-user-notifications@0.3.2, objc2-web-kit@0.3.2, tinyvec@1.13.3 |
| `Apache-2.0 OR MIT` | 8 | atomic-waker@1.1.2, cesu8@1.1.0, dbus@0.9.12, libappindicator-sys@0.9.0, libappindicator@0.9.0, libdbus-sys@0.2.7, portable-atomic-util@0.2.8, portable-atomic@1.15.0 |
| `Unlicense OR MIT` | 5 | jiff-core@0.1.4, jiff-static@0.2.41, jiff-tzdb-platform@0.1.3, jiff-tzdb@0.1.9, jiff@0.2.41 |
| `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | 3 | wasi@0.11.1+wasi-snapshot-preview1, wasip2@1.0.4+wasi-0.2.12, wit-bindgen@0.57.1 |
| `BSD-3-Clause OR MIT OR Apache-2.0` | 2 | num_enum@0.7.6, num_enum_derive@0.7.6 |
| `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | 2 | r-efi@5.3.0, r-efi@6.0.0 |
| `ISC` | 1 | libloading@0.7.4 |
| `Apache-2.0` | 1 | sync_wrapper@1.0.2 |
| `Apache-2.0 WITH LLVM-exception` | 1 | target-lexicon@0.12.16 |
| `Zlib` | 1 | zlib-rs@0.6.8 |
