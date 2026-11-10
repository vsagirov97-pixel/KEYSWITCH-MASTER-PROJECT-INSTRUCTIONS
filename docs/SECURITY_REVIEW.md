# SECURITY_REVIEW — единый реестр безопасности KeySwitch

Владелец документа: **AGENT 11 CISO** · Редакция 2 · 2026-11-10 · ветка `docs/governance-v3`
Редакция 2 (директива V7, D-24…D-26): в область добавлены **macOS** и **автоматические обновления безопасности**; новые риски SEC-40…SEC-57; номера gates приведены к V7 (16 gates); записаны условия CISO к изменению сетевого принципа (D-25, §4.8). Связанные политики: `docs/EMERGENCY_PATCH_POLICY.md`, `docs/SIGNING_KEY_MANAGEMENT.md`. Проект `docs/UPDATE_SECURITY_ARCHITECTURE.md` (AGENT 03) на момент редакции **CISO не рецензирован** — ревью отдельно.
Статус: **действующий источник истины по безопасности.** Заменяет как реестр: `docs/03-threat-model.md` (T-01…T-16, черновик оркестратора, авторство «Security Analyst» в нём неверно), а также списки находок в `docs/research/sec-threat-model.md` (ST-xx), `sec-architecture-audit.md` (AF-xx, C-1…C-9), `review-sec-on-architecture.md` (XR-xx, C-1′…C-10), `review-arch-on-security.md`. Эти документы остаются **обоснованием** (сценарии, DFD, SR-xx, план тестов H-01…H-40), но статусы и решения ведутся только здесь.
Процедура ревью и состав команды — `docs/security/security-team-plan.md`.

**Область CISO (V7):** интеграция с Windows (10/11) и **macOS** (TCC/Accessibility, Secure Event Input, NSPasteboard, Input Sources, Login Items, hardened runtime, подпись и нотаризация); буфер обмена; глобальные горячие клавиши; активные поля ввода; защита текста пользователя; Tauri capabilities; Rust-компоненты; песочница Python/JS/WebAssembly; сторонние зависимости; локальные данные; журналы и аварийные дампы; **автоматические обновления, каналы STABLE/EMERGENCY, подписанные emergency-политики, ключи подписи**; установщик и распространение; privacy engineering; secure SDLC.

**Нумерация gates (V7, D-24):** 1 Audit · 2 Team · 3 Requirements · 4 Architecture · 5 Design · 6 Core · 7 Windows Integration · 8 macOS Integration · 9 Macro Studio · 10 Automatic Update Security · 11 QA · 12 Security · 13 Legal · 14 Financial & Commercial · 15 Windows Release · 16 macOS Release. В записях редакции 1 номера были по V3; соответствие: V3 GATE 8 (Macro Studio) → **9**, V3 GATE 9 (QA) → **11**, V3 GATE 10 (Security) → **12**, V3 GATE 11 (Legal) → **13**, V3 GATE 12 (Release) → **15/16**. Ниже в таблицах вердиктов и приоритетах — уже V7; в тексте отдельных строк §3–§5 упоминания «GATE 8/10/12» редакции 1 читать по этому соответствию.

Пометки: **[проверено CISO]** — проверено мной в этой сессии (команда указана); **[док]** — по документам проекта, не по коду; **UNVERIFIED** — проверить невозможно сейчас (причина указана).

---

## 1. Резюме для владельца

- Реального продуктового кода, затрагивающего безопасность, **ещё нет**: есть только `ks-core` (чистое ядро, без I/O и `unsafe`) и пустая Tauri-оболочка с одной командой `core_version`. Все Critical/High риски — **проектные**; закрыть их можно только реализацией и тестами на Windows (спайки S-2, S-3).
- Проектные решения в консолидированной архитектуре (`docs/SYSTEM_ARCHITECTURE.md` v2.1, ещё не закоммичена) **в основном соответствуют** требованиям Security, но детальные документы (`arch-architecture-v2.md`, `arch-windows-field-replacement.md`, `01-prd.md`, `00-decisions.md` D-07) ещё содержат отменённые позиции (preopen, кадр 1 МиБ, 100 000 символов, `allows_paste = true`, Enter/Tab без разрешения) — **проверено grep, см. §2**.
- Конфигурационные «быстрые исправления» из аудита 2026-11-09 **не выполнены** (capabilities `core:default`, caret-версии, нет `--locked`, нет бан-листа `deny.toml`, `npm ci` без `--ignore-scripts` в Windows-job, autofill WebView2).
- Новая находка: ветка `main` публичного репозитория **не защищена**, secret scanning / push protection / Dependabot alerts **выключены** (SEC-39).
- **Редакция 2 (V7):** D-25 меняет принцип «нет сети в runtime» — разрешён единственный канал обновлений; мои обязательные условия — §4.8. Добавлены 10 рисков канала обновлений (SEC-40…49, из них 2 Critical: компрометация ключа подписи и компрометация конвейера сборки/подписи при автоустановке) и 8 рисков macOS (SEC-50…57). SEC-32 («нет механизма обновлений») заменён этой группой.
- Открыто: **Critical 5, High 28, Medium 23** (ни один Critical/High не в статусе Verified — нечем проверять). Частично принят мной остаточный риск по двум Medium (SEC-12-R, SEC-24-R).

### Вердикты

| Gate | Вердикт CISO | Кратко |
|---|---|---|
| **GATE 4 Architecture (security-часть)** | **BLOCK** (снимается поэтапно: → PASS WITH CONDITIONS → PASS) | Проектные решения приняты, но не внесены в детальные документы; S-2/S-3 не выполнены. **Не блокирует** запуск S-2, S-3, BL-CFG-01 и работу над `ks-core` — наоборот, условия ниже выполняются именно в них. |
| **GATE 8 macOS Integration (security-часть)** | **BLOCK** | Нет macOS-адаптера, macOS-CI и специалиста по безопасности macOS; открыты SEC-50…57. Условия — §3.7. |
| **GATE 10 Automatic Update Security** | **BLOCK** | Проект `UPDATE_SECURITY_ARCHITECTURE.md` мной не рецензирован; нет ключей, релизного workflow, апдейтера; открыты SEC-40…49. Условия — ниже. |
| **GATE 12 Security** (V3: GATE 10) | **BLOCK** | Нет реализации для проверки; открыты все Critical/High; нет подписи, provenance, runtime-проверки сети. |

#### Условия GATE 4 → PASS WITH CONDITIONS (все обязательны; разрешает массовую разработку `ks-engine`/`ks-platform`/UI)
| № | Условие | Закрывает | Исполнитель |
|---|---|---|---|
| G4-C1 | 13 исправлений `review-arch-on-security.md` §2 внесены в AV2/FR/DEP (BL-ARCH-01); `docs/02-architecture.md` помечен «заменён `SYSTEM_ARCHITECTURE.md`»; `SYSTEM_ARCHITECTURE.md` закоммичен | SEC-02, 06, 10, 11, 14 | AGENT 03 |
| G4-C2 | В `SYSTEM_ARCHITECTURE.md` дописаны пробелы, найденные мной: политика окна для макросов (D-05 + строгий список, SR-48); `clipboard = null` при метках приватности (SR-30); `\n`/`\t` в тексте макроса — под тем же разрешением, что Enter/Tab; многосигнальное распознавание паролей и клиенты RDP/VM в исключениях (SR-23); проверка модификаторов перед **каждым** пакетом (SR-25); утверждённый список импортов WASI (SR-43) и минимальные фичи Wasmtime (SR-41); `JOB_MEMORY`, WER для `ks-sandbox.exe`; нативное подтверждение опасных настроек (SR-71); отсутствие обработки argv/deep links (SR-75) | SEC-04, 07, 08, 15, 30 | AGENT 03 |
| G4-C3 | `00-decisions.md`: D-07 изменён (Enter/Tab — отдельное разрешение, решение владельца V3); записаны решения CISO §4 (Q-1, Q-3, Q-8, Q-9, X-4, XR-07) | SEC-07, 10, 21, 22 | AGENT 01 |
| G4-C4 | `01-prd.md` FR-MACRO-4: «100 000 символов вывода» → 5 000 символов действия, ≤ 64 КиБ stdout/stderr; FR-CLIP-2 — исключение «нет сигнала → не восстанавливать» | SEC-06, 10 | AGENT 02 |
| G4-C5 | `docs/03-threat-model.md` заменён ссылкой на этот документ | реестр | AGENT 01 |

#### Условия GATE 4 → PASS (дополнительно)
| № | Условие | Проверка |
|---|---|---|
| G4-P1 | S-3 на `windows-latest`: H-01…H-40 (оба рантайма) — все PASS; самопроверка SR-56 PASS; пустой список preopen (unit); импорты ⊆ списка; дедлайн ≤ 2,5 с; fuzz протокола; замеры матрицы Cranelift/Pulley × LPAC × ACG | логи CI, отчёт спайка «проверено/не проверено», ревью Runtime/WASM-специалиста |
| G4-P2 | S-2 на `windows-latest`: сценарии буфера (задержки 0/50/500/2000 мс, сторонний слушатель, гонка seq, зависший владелец, метки), фокус/поколение операций, окно выбора `WS_EX_NOACTIVATE`, пароли (`GetText` = 0 вызовов), crash/WER | логи CI, ревью Windows-специалиста |
| G4-P3 | BL-CFG-01 выполнен и зелёный в CI: явные capabilities, `=`-версии, `--locked`, бан-лист `deny.toml` (тестовая ветка с запрещённым crate → FAIL), `--ignore-scripts` во всех job, `generalAutofillEnabled: false` | CI + повторная проверка CISO |
| G4-P4 | Решение владельца по SEC-39 (защита `main`, secret scanning) | `gh api repos/{owner}/{repo}/branches/main` → `protected: true` |

#### Условия GATE 12 Security → PASS (в редакции 1 — «GATE 10»)
1. Нет открытых Critical/High; каждый остаточный Critical/High **письменно принят владельцем** (в т.ч. итог ST-22 после S-3).
2. Все строки §3 со статусом Verified по указанному методу, на сборке-кандидате.
3. SR-90 (изменено D-25): за ≥ 60 с работы — 0 исходящих не-loopback соединений всех процессов дерева KeySwitch, **кроме компонента обновлений к утверждённым хостам** (порт 443, только по расписанию проверки); при отключённых администратором обновлениях — строго 0. Windows CI и macOS CI + ручная проверка владельцем.
4. Crash-тест: нет дампов с текстом (оба exe).
5. H-01…H-40 на релизной конфигурации Wasmtime, версия Wasmtime без открытых advisory.
6. Релизный workflow: тег из защищённой ветки, provenance-аттестация, `SHA256SUMS`; решение по подписи (D-11).
7. `cargo deny`, `npm audit`, `npm audit signatures` — без исключений без срока.
8. Ручная матрица владельца (D-10) пройдена в части безопасности (пароли, менеджеры паролей, терминалы, Win+V, RDP).
9. README/«Приватность» честно описывают остаточные риски (SEC-12-R, WebView2, корпоративный WER) и сетевой канал обновлений (что передаётся, куда, как отключить).
10. GATE 10 и (для macOS-релиза) GATE 8 — PASS.

#### Условия GATE 10 Automatic Update Security → PASS (все обязательны)
| № | Условие | Проверка |
|---|---|---|
| G10-1 | `UPDATE_SECURITY_ARCHITECTURE.md` одобрен CISO и CLO; выполнены все условия §4.8 | ревью CISO (письменно), заключение CLO |
| G10-2 | Решения владельца LEVEL 3 по `SIGNING_KEY_MANAGEMENT.md` §11 п. 1–5, 7 приняты и записаны в `00-decisions.md` | D-xx |
| G10-3 | Церемония корня проведена (≥ 2 независимых корня, протокол без секретов в `docs/security/`); публичные ключи встроены PR с ревью CISO | протокол + сверка отпечатков |
| G10-4 | Релизный workflow: тег из защищённой ветки, environments с обязательным ревьюером, секреты только в environment, OIDC где возможно, `id-token` только в job подписи, provenance, `SHA256SUMS`, SBOM; SEC-39 закрыт | ревью workflow, `gh api` настроек, `gh attestation verify` |
| G10-5 | Негативные тесты апдейтера в CI (Windows и macOS): неподписанный пакет, чужой ключ, изменённый байт, downgrade, повтор старых метаданных, истёкшие метаданные, чужая платформа/архитектура, смешанные метаданные, ответ > лимита, бесконечный поток, редирект на чужой хост, TLS-ошибка → **все отвергнуты**, текущая версия цела | логи CI |
| G10-6 | Тест ротации и отзыва: K-2 сменён через подписанный корнем список; отозванный ключ отвергается; переход на резервный корень K-1b | логи CI |
| G10-7 | Emergency-политика: только закрытый перечень ограничений; попытка включить функцию/неизвестное поле/истёкшая/с меньшим номером/подписанная K-2 вместо K-3 → отвергнута; fuzz разборщика метаданных и политики | логи CI + fuzz-отчёт |
| G10-8 | Безопасный момент: автоустановка не применяется при активной замене, удерживаемом снимке буфера, выполняемом макросе, несохранённом черновике Macro Studio; откат при неудачной самопроверке | автотесты + сценарий QA |
| G10-9 | Приватность: запрос не содержит идентификаторов устройства/пользователя, cookies, текста; User-Agent без уникальных данных; журнал апдейтера без текста пользователя | перехват трафика в CI (тестовый хост) + ревью кода |
| G10-10 | Корпоративное отключение (Windows HKLM-политика; macOS — управляемые настройки) работает: 0 соединений, статус виден в «Диагностике» | Windows CI + macOS CI |
| G10-11 | Сетевой стек и разбор ответов — только в компоненте обновлений; `cargo deny` запрещает сетевые crate в остальных crate | `cargo tree`, `deny.toml` (wrappers), тестовая ветка → FAIL |
| G10-12 | Учебный прогон EMERGENCY-процесса (`EMERGENCY_PATCH_POLICY.md`) на тестовом канале, включая все проверки §9 и отчёт владельцу | протокол учения |

---

## 2. Что проверено мной (2026-11-10)

| Проверка | Команда / место | Результат |
|---|---|---|
| Все локальные проверки | `scripts/check.sh` | `PASS rustfmt · PASS clippy ks-core · PASS tests ks-core · PASS frontend typecheck · PASS frontend build · PASS clippy app (windows target, check only) · PASS cargo deny · PASS npm audit`; Windows runtime, установщик, UI — **не выполнялись** (Linux-хост) |
| Тесты ядра | `cargo test -p ks-core` | 4 + 26 passed, 0 failed |
| Поставки | `cargo deny --all-features check` | `advisories ok, bans ok, licenses ok, sources ok`; база RustSec — снимок `d9cb059f` от 2026-11-09 17:13 +0100; `cargo deny fetch db` новых данных не принёс (**UNVERIFIED**, есть ли advisory новее) |
| npm | `npm audit --audit-level=high` | 0 уязвимостей |
| CI на GitHub | `gh run list` / `gh run view` | `docs/governance-v3` run 42787897293 и `research/phase-1` run 42786382364 — все 4 job ✓ (Core Linux, Frontend, Windows build, Supply chain) |
| Граф | `Cargo.lock` | 405 пакетов; `reqwest`, `hyper`, `tokio` — по 1 (через `tauri 2.12.5`, `cargo tree -i reqwest --target all`); `rustls`, `native-tls`, `openssl` — 0. Что `reqwest` — мёртвый код на desktop (вывод AGENT 07) — **мной не перепроверено** |
| `unsafe` | `Cargo.toml` `[workspace.lints.rust] unsafe_code = "forbid"`; grep | `unsafe` в коде нет |
| Текст пользователя | `crates/ks-core/src/convert.rs:48–66` | `Conversion { pub text: String }`, `Clone`, ручной `Debug` без `text` ✔; `UserText` не существует (допустимо для ядра, обязательно для `ks-engine`/`ks-platform`) |
| Capabilities | `app/src-tauri/capabilities/main.json` + `gen/schemas/acl-manifests.json` | `core:default` → 9 наборов, в т.ч. `core:image:allow-from-path`, `core:event:allow-emit(-to)`, `core:webview:allow-internal-toggle-devtools`, `core:tray:allow-set-temp-dir-path`; описание «only KeySwitch's own commands» неверно |
| CSP / IPC | `tauri.conf.json` | строгая CSP без `unsafe-*`, `freezePrototype: true`, `withGlobalTauri: false` ✔; единственная команда `core_version` (`app/src-tauri/src/main.rs`) ✔ |
| Autofill WebView2 | `tauri.conf.json`; `tauri-utils-2.10.2/src/config.rs:2380–2397` | ключ `generalAutofillEnabled` не задан → по умолчанию `true`; исправляется конфигом (не отключает автозаполнение паролей — достаточно, т.к. полей паролей в UI нет) |
| Версии | `app/src-tauri/Cargo.toml` | `tauri-build = "2.7.3"`, `tauri = "2.12.4"` — caret (lock: 2.12.5) |
| `deny.toml` | файл | нет `[bans] deny`; `unmaintained = "workspace"` |
| CI | `.github/workflows/ci.yml` | actions по SHA с комментарием ✔, `permissions: contents: read` ✔, нет `pull_request_target` ✔; Windows-job `npm ci` **без** `--ignore-scripts`; нигде нет `--locked`; `branches: ["**"]`; артефакт хранится 14 дней; нет `timeout-minutes`; нет запуска по расписанию (advisory проверяются только при push) |
| Установщик | `tauri.conf.json` | `installMode: currentUser`; `webviewInstallMode` не задан (по умолчанию загрузчик из сети при установке) |
| Release-профиль | `Cargo.toml` | `panic = "abort"` → путь в WER (SEC-18) |
| Репозиторий | `gh api repos/{o}/{r}`, `/branches`, `/rulesets` | публичный; `main` `protected=false`, rulesets `[]`; secret scanning, push protection, Dependabot security updates — `disabled`. PR #1, #2 слиты владельцем 2026-11-09 (соответствует правилу 6) |
| Детальные документы | grep | `01-prd.md:85` «100 000 символов»; AV2:277 `MAX_FRAME = 1 << 20`; AV2:383/512 и DEP:51 — read-only preopen; FR:390 `allows_paste = true`; `00-decisions.md` D-07 — Enter/Tab без отдельного разрешения |

| Редакция 2: обновления/ключи | grep `updater`, `pubkey`, `secrets.`, `environment`, `id-token`, `schedule` в `app/src-tauri/`, `.github/workflows/ci.yml` | 0 совпадений: апдейтера, ключей, секретов, environments, расписания нет; триггеры CI — `push` (`**`) и `workflow_dispatch`; `permissions: contents: read` |
| Редакция 2: директива | `docs/master-instructions-v7.md` | краткое изложение из 12 пунктов; разделов §5/§12/§13 нет — ссылки задания на §12.1/§13.5 **UNVERIFIED**, использованы п. 2, 3, 11 и D-24…D-26 |

**Не проверено (почему):** любое поведение Win32/UIA/буфера/SendInput/WER/AppContainer/Job — кода нет, Windows-хоста нет (только CI); настройки Actions репозитория — прокси отвечает 403; официальные сайты Microsoft/Wasmtime/Tauri в этой сессии не открывались; эксплуатабельность advisory Wasmtime для нашей конфигурации — конфигурации нет.

---

## 3. Реестр уязвимостей и рисков

Уровень — до митигации (шкала `sec-threat-model.md` §0). Статусы: **Open** — решения нет или не начато; **In remediation** — решение принято, реализация/документы/проверка не завершены; **Verified** — проверено указанным методом; **Accepted** — остаточный риск принят уполномоченным лицом.
Владельцы (роли `docs/TEAM_STRUCTURE.md`): 01 PD, 02 PA, 03 ARCH, 05 WIN, 06 FE, 07 SA, 09 REL, 12 CLO, 13 SBX. Приоритет (V7): **P0** — блокирует GATE 4 / S-2 / S-3 (для macOS — GATE 4 и macOS-спайки); **P1** — блокирует GATE 7/8/9 (интеграция Windows/macOS, Macro Studio) и GATE 10 (обновления); **P2** — блокирует GATE 12/15/16.

### 3.1 Critical

| ID | Название | Ссылки | Владелец | P | Решение / исправление | Проверка (метод → результат) | Остаточный риск | Статус |
|---|---|---|---|---|---|---|---|---|
| SEC-01 | Выход из Wasm через баг Wasmtime/Cranelift | ST-22, T-01, AF-08, XR-08, X-14 | 13 (реализация), 03 | P0 | Отдельный процесс `ks-sandbox.exe` + LPAC + Job + win32k lockdown (SEC-04); Wasmtime LTS 48 **≥ 48.0.8** (48.0.7 уязвим: RUSTSEC-2026-0422/0423/0424, [проверено CISO] в локальной базе RustSec); `default-features = false`, только `runtime`/`std`/`cranelift` или `pulley`, без `cache`/`component-model`/`gc`/`threads`/`pooling-allocator`; security-патчи по процедуре Q-3 (§4.2); ACG — если Pulley уложится в бюджет (S-3) | S-3: H-01…H-40 на `windows-latest`; ревью фич в `Cargo.toml` → **не выполнено** (кода нет) | После всех мер — **ожидается High** (обход одного слоя при живом втором); окончательно — после S-3. Принять может **только владелец** | Open |
| SEC-02 | Мисконфигурация WASI: preopen каталогов stdlib, сокеты, env, часы | ST-23, T-01, AF-01, XR-07, C-3/C-3′, X-2 | 13, 03 | P0 | **Решение CISO XR-07 (§4.6): ноль preopen обязательно по умолчанию**; stdlib — `include_bytes!` zip + собственный read-only слой WASI в памяти; без `wasi:sockets/http`, env/args пустые, часы ≥ 1 мс; утверждённый список импортов (SR-43) | unit: пустой список preopen, импорты ⊆ списка; H-01…H-12 → **не выполнено**. Документы: AV2/DEP ещё содержат preopen (§2) | Low после реализации | In remediation |
| SEC-03 | Встроенный (in-process) интерпретатор Python/JS | ST-29, T-01, AF-23 | 09 (бан-лист), 03 | P0 | Архитектурно исключён (D-08, `check_macro`/`test_macro` тоже в песочнице); принудительно — `[bans] deny` (`pyo3`, `rustpython-vm`, `rquickjs`, `boa_engine`, `deno_core`, `v8`, `wasmtime-wasi-http` и др., SR-62); гостевые crate — вне workspace со своим `deny.toml` | тестовая ветка с запрещённым crate → `cargo deny` FAIL → **не выполнено**: бан-листа нет ([проверено CISO] `deny.toml`) | Low | In remediation |

### 3.2 High

| ID | Название | Ссылки | Владелец | P | Решение / исправление | Проверка (метод → результат) | Остаточный риск | Статус |
|---|---|---|---|---|---|---|---|---|
| SEC-04 | Слабый второй слой песочницы (AppContainer/Job/mitigations) | ST-30, AF-02, C-4/C-4′ | 13, 03 | P0 | LPAC (без `ALL APPLICATION PACKAGES`), 0 capabilities, явный handle list, child-process policy, mitigations (win32k lockdown, strict handles, extension points, image load, font), Job: `KILL_ON_JOB_CLOSE`, `ACTIVE_PROCESS=1`, `PROCESS_MEMORY`+`JOB_MEMORY`, все `UILIMIT_*`, `DIE_ON_UNHANDLED_EXCEPTION`; самопроверка токена и Job при старте (SR-56); без `user32`/`gdi32` в импортах; ACL каталога для SID контейнера; безопасность **не зависит** от ACG | S-3: `QueryInformationJobObject`, самопроверка, `dumpbin /imports` → **не выполнено** | Medium | In remediation |
| SEC-05 | Ошибки в хост-функциях и собственном слое WASI | ST-24 | 13 | P0 | Хост-функции без `unsafe`, доступ к памяти гостя только через проверенные срезы; fuzz-цель на каждую функцию (собственный WASI увеличивает поверхность) | cargo-fuzz/proptest в CI → **не выполнено** | Low–Medium | Open |
| SEC-06 | Протокол действий песочницы: размеры, символы, десериализация | ST-25, AF-04, C-2, X-8 | 13, 03, 02 (PRD) | P0 | Кадр гость→хост ≤ 64 КиБ, хост→гость ≤ 256 КиБ, исходник ≤ 64 КиБ; `request_id`; одно `Done/Failed` на запуск; `deny_unknown_fields`; глубина ≤ 8; `insert/replace` ≤ 5 000 символов, без C0/C1 (кроме `\n` под разрешением SEC-07), без bidi-override/zero-width/U+0000; `keys` ≤ 16; повторная валидация в `ks-engine` | proptest/fuzz приёмника 10⁶ сообщений → **не выполнено**; документы: PRD:85 и AV2:277/388 противоречат (§2) | Low | In remediation |
| SEC-07 | Макрос выполняет команды: текст + Enter/Tab, удерживаемые модификаторы | ST-26, ST-11, AF-05, C-5, X-10, Q-2 | 13, 05, 03 | P0 | **Решение владельца (V3):** Enter и Tab — отдельное разрешение слота, по умолчанию выкл. **Решение CISO (толкование):** под то же разрешение подпадают `\n`/`\r`/`\t` в тексте `insert/replace`. Политика окна × действие (`ks-engine::macro_policy`): исключения D-05 + строгий список (консоли, «Выполнить», адресные строки, табличные процессоры) до запуска и перед применением; проверка модификаторов перед каждым пакетом; Esc/повторный хоткей прерывает; разрешения привязаны к хэшу исходника | unit политики; Windows CI с окном класса консоли → **не выполнено**; D-07 в `00-decisions.md` ещё старый | Medium (соцнженерия при выданном разрешении) | In remediation |
| SEC-08 | Чтение буфера макросом как канал эксфильтрации | ST-27, ST-04, AF-06 | 13, 05 | P0 | Разрешение per-slot, выкл. по умолчанию, `granted_for = хэш исходника`; при метках исключения/данных менеджера паролей → `clipboard = null`; только `CF_UNICODETEXT` ≤ 5 000; отдельный процесс на запуск (Q-9) | unit: без флага `null`, смена хэша → `null`; Windows CI: буфер с меткой → `null` → **не выполнено** | Medium (пользователь сам выдал разрешение) | In remediation |
| SEC-09 | Происхождение `quickjs.wasm` / `python.wasm` / stdlib | ST-49, ST-32, AF-07, C-8′ | 13, 09, 12 (лицензии) | P0 | Сборка в CI из закреплённых **commit SHA**, WASI SDK по хэшу; `include_bytes!` в `ks-sandbox.exe`; хэши публикуются; пересборка сравнивает хэш | CI-джоб пересборки → **не выполнено** | Low | In remediation |
| SEC-10 | Вставка **прежнего** буфера (пароля) из-за раннего/слепого восстановления | ST-01, AF-09, XR-01, XR-02, X-1, Q-1 | 05, 03, 02 (FR-CLIP-2) | P0 | **Решение CISO Q-1 (§4.1).** W1 → W3 → W2 (только по профилю); `WM_RENDERFORMAT` до Ctrl+V → прервать W2; после Ctrl+V сигнал только от PID цели/брокера из профиля; restore только при сигнале **и** пост-проверке `Yes`; иначе не восстанавливать | S-2: цель читает с задержкой 0/50/500/2000 мс и сторонний слушатель, 100 прогонов — прежний буфер ни разу не в поле → **не выполнено** | Medium (утрата прежнего буфера, см. §4.1) | In remediation |
| SEC-11 | Snapshot/restore: потеря данных буфера, TOCTOU seq, зависший владелец | ST-02, ST-05, ST-06, AF-10, XR-04, XR-10 | 05, 03 | P0 | Перечисление без чтения; чтение только белого списка (текст/Unicode/HTML/RTF/локаль), ≤ 16 МБ; иные/OLE/приватные форматы или `IsHungAppWindow(owner)` → W2 запрещён; дедлайн 500 мс, «отравленный» поток → отказ; seq сравнивается под `OpenClipboard`; снимок в обнуляемом буфере | S-2 фикстуры форматов, гонка второго писателя ×1000, владелец со `Sleep(5000)` → **не выполнено** | Medium (зависание владельца блокирует буфер системы) | In remediation |
| SEC-12 | Утечка временного текста в Win+V/облако/менеджеры/RDP | ST-03, T-04, D-13, Q-8 | 05 | P1 | Метки `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory=0`, `CanUploadToCloudClipboard=0` в той же транзакции; W2 запрещён при `SM_REMOTESESSION`; W2 только по профилю | Windows CI: форматы присутствуют; ручная Win+V → **не выполнено** | **SEC-12-R** (сторонние слушатели игнорируют метки) — Medium, **принят CISO** (§4.3) | In remediation (остаток — Accepted) |
| SEC-13 | Restore снимает метки приватности исходных данных (пароль из менеджера попадает в историю) | ST-04 | 05 | P1 | Метки исходных данных сохраняются в snapshot и восстанавливаются 1:1 (+ дополнительный запрет истории) | S-2 фикстура «буфер с метками» → **не выполнено** | Low | In remediation |
| SEC-14 | Ввод уходит не в то окно (TOCTOU фокуса, диалог, «отравленный» поток, модификаторы) | ST-07, ST-08, ST-11, AF-11, XR-05, XR-06, X-4, X-6 | 05, 03 | P0 | Отпечаток цели (HWND, PID + время старта, RuntimeId); пакеты ≤ 64 символа с проверкой foreground/отпечатка/модификаторов перед каждым; прерывание при блокировке сессии; атомарное поколение операций и единый шлюз побочных эффектов; `SelfTarget`; окно выбора — **решение CISO X-4 (§4.5)**. SR-21 «физический ввод» заменён проверкой между пакетами (без хуков, правило 8) | S-2: смена foreground через N мс → в чужое окно ≤ 1 пакета; журнал вызовов fake без старого поколения → **не выполнено** | Low–Medium (посторонний ввод в то же поле) | In remediation |
| SEC-15 | Неполное распознавание полей паролей/OTP | ST-09, T-08, AF-12 | 05, 03 | P0 | Отказ **до чтения**: `IsPassword`, `ES_PASSWORD`, эвристика имени/AutomationId/autocomplete (`password, pin, otp, cvv, cvc, пароль, код`), исключения D-05 + клиенты RDP/VM; неизвестный framework с непрозрачным полем — отказ | S-2: `type=password`, CSS-маска, `autocomplete=one-time-code` → `GetText` 0 вызовов → **не выполнено**; многосигнальность пока не в `SYSTEM_ARCHITECTURE.md` | Medium (кастомные поля без признаков) | Open |
| SEC-16 | Массовое изменение документа, случайная отправка (Enter), Excel | ST-16, ST-19, ST-20, T-07, AF-13, X-5, Q-4 | 05, 03 | P0 | Ctrl+A/«всё поле» только при условиях SR-31; браузеры — только выделение/фрагмент до S-2 (решение владельца Q-4); `\r\n\t` никогда через SendInput; Excel вне правки ячейки — отказ; лимит 5 000 | S-2 + ручная матрица D-10 → **не выполнено** | Medium | In remediation |
| SEC-17 | `EM_REPLACESEL` в RichEdit: интерпретация RTF, запись мимо переднего плана | XR-03 | 05 | P0 | W1 только для класса `Edit`; RichEdit — после теста S-2 и если текст не начинается с `{\rtf`/`{urtf`; вывод макросов через W1 в RichEdit запрещён; foreground и отпечаток перед `SendMessageTimeout` | S-2: `RICHEDIT50W` + `{\rtf1 \b x}` вставлен литералом → **не выполнено** (UNVERIFIED-гипотеза) | Low | In remediation |
| SEC-18 | Crash-дампы с текстом и буфером (WER) | ST-43, T-09, AF-18 | 05, 13 | P1 | `WerAddExcludedApplication` для обоих exe (основной процесс/установщик; деинсталлятор удаляет), `WerSetFlags(WER_FAULT_REPORTING_FLAG_NOHEAP)` в каждом процессе, `SetErrorMode(SEM_NOGPFAULTERRORBOX)`, panic hook только `file:line` (в бинарях), песочница — `DIE_ON_UNHANDLED_EXCEPTION`; README — корпоративные `LocalDumps` сильнее | Windows CI crash-тест (фича `test-hooks`): `panic=abort`/`__fastfail`/трап Wasmtime → нет дампа → **не выполнено**; `panic = "abort"` в `Cargo.toml` [проверено CISO] | Medium (корпоративная политика WER) | Open |
| SEC-19 | Неподписанные бинарии, подделка дистрибутива, ложные срабатывания АВ | ST-50, D-11 | 09, владелец | P2 | Релиз только подписанный (D-11, платно → решение владельца); до этого — артефакты «не для распространения» | Решение владельца → **нет** | — | Open (требует владельца) |

### 3.3 Medium (открытые)

| ID | Название | Ссылки | Владелец | P | Решение / исправление | Проверка → результат | Остаточный риск | Статус |
|---|---|---|---|---|---|---|---|---|
| SEC-20 | DoS скриптом; fuel обходится через `poll_oneoff` | ST-28, T-12, AF-03 | 13 | P0 | epoch + дедлайн 2 с в основном процессе → `TerminateJobObject`; `poll_oneoff`/sleep реализует хост с суммарным бюджетом; `StoreLimits` 64 МБ | H-20…H-25: завершение ≤ 2,5 с, процесса нет → не выполнено | Low | In remediation |
| SEC-21 | Security-патчи моложе 2 недель (правило CLAUDE.md №3) | AF-08, XR-08, X-9, Q-3, SR-60 | 07 (ревью), 09, 01 | P0 | **Процедура CISO Q-3 (§4.2)** | первое применение — `wasmtime =48.0.8` в S-3 | Low | In remediation (текст CLAUDE.md — владелец) |
| SEC-22 | «Тёплый» процесс песочницы разделяет состояние между запусками | XR-09, ST-27, ST-34, Q-9, SR-51 | 13 | P0 | **Решение CISO Q-9 (§4.4)** | unit: два запуска не делят глобальные переменные; Windows: новый PID для слота с буфером → не выполнено | Low | In remediation |
| SEC-23 | UIPI: молчаливый частичный ввод; KeySwitch запущен с повышенными правами | ST-10, T-15, AF-16 | 05 | P1 | Манифест `asInvoker`, `uiAccess=false`; отказ работать при собственном повышенном токене (`SelfElevated`); сравнение IL цели до записи | unit функции токена; ручная проверка elevated → не выполнено | Low | In remediation |
| SEC-24 | Хоткеи: AltGr-конфликты, Win, синтетический запуск | ST-12, AF-14 | 05 | P1 | Валидатор: обязательна не-модификаторная клавиша, Win запрещён, `VkKeyScanEx` по всем HKL (AltGr-символ → отклонить) | unit + Windows CI (немецкая раскладка) → не выполнено | **SEC-24-R** синтетический запуск хоткея процессом того же пользователя — Medium, **принят CISO** (§4.7) | In remediation (остаток — Accepted) |
| SEC-25 | UIA-клиент: недоверенный провайдер, чрезмерное чтение, зависание | ST-14, AF-15 | 05 | P1 | `ConnectionTimeout/TransactionTimeout`, `GetText(5001)`, сторож T3 | Windows CI фейковый провайдер → не выполнено | Low | In remediation |
| SEC-26 | Потеря Undo | ST-17, D-12, Q-6 | 05 | P1 | Ctrl+Z возвращает исходный текст, возможно за 2 шага (решение владельца Q-6), задокументировать | S-2 матрица Undo → не выполнено | Low (принято владельцем по Q-6) | In remediation |
| SEC-27 | React controlled inputs / перехват клавиш страницей | ST-18 | 05 | P1 | Пост-проверка SR-12; расхождение → стоп без повторов | S-2 Edge/WebView2/Electron → не выполнено | Low | Open |
| SEC-28 | Соцнженерия: вставка чужого скрипта в слот | ST-31 | 13, 04 | P1 | Импорта нет в v1; при изменении скрипта — сводка возможностей и сброс разрешений (хэш); опасные разрешения — нативный диалог (SR-71) | ревью UI + unit → не выполнено | Medium (пользователь вставил и разрешил) | Open |
| SEC-29 | Вывод скрипта: XSS в UI, текст в журналах | ST-33, SR-58, SR-74 | 06, 13 | P1 | Вывод только в памяти ≤ 64 КиБ, не сохраняется; в UI — только как текст (без `innerHTML`/`dangerouslySetInnerHTML`) | unit + ревью фронтенда → не выполнено | Low | Open |
| SEC-30 | IPC как рычаг XSS (включение буфера/Enter/Tab, исключения, хоткей) | ST-36, SR-71, SR-75 | 05, 06 | P1 | Опасные настройки — только через нативный диалог, не управляемый WebView; нет IPC «ввести текст»; нет argv/deep links | ревью команд + тест → не выполнено (команд пока нет) | Low | Open |
| SEC-31 | WebView2: autofill, проверка орфографии, crash-отчёты | ST-38, AF-21 | 09, 06 | P1 | `generalAutofillEnabled: false` для окна `main` (конфиг, [проверено CISO] ключ существует); devtools выкл. в релизе ✔; проверка орфографии/отчёты WebView2 — проверить вручную | ручная проверка профиля `EBWebView` → не выполнено | Medium (поведение WebView2 Runtime вне контроля) — **не принят**, решение на GATE 10 по данным | Open |
| SEC-32 | Нет механизма обновлений → у пользователей устаревший Wasmtime | ST-39, SR-63 | 01, 09 | P2 | **Редакция 2:** D-25 вводит автоматические обновления безопасности — риск доставки исправлений переходит в SEC-40…49 и `EMERGENCY_PATCH_POLICY.md`; версия и дата сборки в UI + предупреждение о возрасте остаются обязательными (для случая отключения обновлений администратором) | закрывается проверкой GATE 10 | Medium | **Superseded** → SEC-40…49 |
| SEC-33 | Хранение: Roaming-профиль, данные в открытом виде | ST-41, AF-17, X-7 | 05 | P1 | `%LOCALAPPDATA%`, `secure_delete`, `journal_mode=DELETE`; HMAC флагов — GATE 8 (заменено привязкой к хэшу) | Windows CI путь БД → не выполнено | Low | In remediation |
| SEC-34 | Журналы/паники раскрывают активность; `UserText` не реализован | ST-42, ST-44, T-09, AF-19, SR-82, SR-83 | 05 | P1 | `UserText` без `Debug/Display/Serialize/Clone` (явный `duplicate()`), zeroize; proto-типы песочницы оборачиваются сразу; `diag_log` — закрытая схема без заголовков/URL/длины | `trybuild` compile-fail + clippy `disallowed_types` → не выполнено (BL-CORE-02) | Low | In remediation |
| SEC-35 | Политика версий: caret, нет `--locked`, нет бан-листа | ST-46, AF-22, AF-23 | 09 | P0 | `=`-версии прямых зависимостей; `--locked` во всех cargo-вызовах CI; `[bans] deny` по SR-62; отдельный не-блокирующий отчёт `unmaintained = "all"` | [проверено CISO] не исправлено; после BL-CFG-01 — CI + тестовая ветка | Low | Open |
| SEC-36 | Фронтенд-цепочка поставок | ST-47, AF-24 | 09 | P0 | `npm ci --ignore-scripts` во **всех** job (Windows-job — нет, [проверено CISO]); `npm audit signatures`; в `dist/` нет внешних URL | CI → не выполнено | Low | Open |
| SEC-37 | CI: происхождение артефактов, широкие триггеры, нет мониторинга advisory | ST-48, AF-24 | 09 | P2 (часть P1) | Отдельный релизный workflow по тегу из защищённой ветки с environment-подтверждением владельца, `attest-build-provenance`, `SHA256SUMS`; артефакты веток ≤ 7 дней, «не для распространения»; `timeout-minutes`; **еженедельный `schedule` для `cargo deny`/`npm audit`** (P1) | ревью workflow, `gh attestation verify` → не выполнено | Low | Open |
| SEC-38 | Сетевая активность в runtime | ST-53, T-11, AF-28, SR-90 | 09, 08 | P1 | **Редакция 2 (D-25):** исключение — только компонент обновлений к утверждённым хостам (§4.8); для всех прочих crate и процессов (основной, WebView, песочница) бан сохраняется. Бан TLS-стеков/сетевых плагинов (SR-62); runtime-тест соединений всех процессов дерева (включая `msedgewebview2.exe`, песочницу); `webviewInstallMode` выбран явно (SR-91) | Windows CI ≥ 60 с, 0 не-loopback → не выполнено | Medium (WebView2 Runtime) | Open |
| SEC-39 | **Новое.** Репозиторий: `main` не защищена, нет rulesets; secret scanning, push protection, Dependabot alerts выключены (публичный репозиторий) | правило 6, D-09 | **владелец** (настройки GitHub), 09 | P0 | Защита `main`: запрет прямого push и force-push, обязательный PR, обязательные статусы CI (4 job); включить secret scanning + push protection, Dependabot alerts (security updates — по процедуре Q-3) | `gh api .../branches/main` → `protected: true`; `security_and_analysis` → `enabled` | Low | Open (требует владельца) |

### 3.4 Low / Informational (кратко, ведутся здесь же)
| ID | Суть | Ссылки | Владелец | Статус |
|---|---|---|---|---|
| SEC-L1 | `core:default` шире минимума — исправить вместе с BL-CFG-01, snapshot развёрнутых разрешений в CI (SR-70) | ST-35, AF-20 | 09 | Open |
| SEC-L2 | `unsafe_code = "forbid"` на весь workspace — `ks-platform-win` (и запуск процесса в `ks-sandbox-host`) получают собственный `[lints]` `deny` + `#[allow]` на модулях с `// SAFETY:`, clippy `undocumented_unsafe_blocks = "deny"`; CI grep `unsafe` вне разрешённых crate → FAIL. Ослабление на уровне workspace запрещено | AF-27, SR-85 | 03, 05 | Open |
| SEC-L3 | `installMode: currentUser`, загрузчик WebView2 при установке — явное решение `webviewInstallMode` | ST-52, AF-26, SR-91 | 09 | Open |
| SEC-L4 | single-instance: аргументы игнорируются, deep links нет | ST-40, SR-75 | 05 | Open |
| SEC-L5 | Целостность wasm на диске — снято `include_bytes!` (SEC-09) | ST-32, T-13 | 13 | In remediation |
| SEC-L6 | Spectre внутри песочницы; переключение раскладки; secure desktop/RDP посреди операции; IME | ST-34, ST-13, ST-15, ST-21 | 13, 05 | Open (по S-2/S-3) |
| SEC-L7 | Сетевые crate в графе через `tauri` (мёртвый код на desktop — по AGENT 07) | ST-45, AF-28 | 09 | Open (см. SEC-38) |
| SEC-I1 | Данные CC-BY-SA встроены в бинарь вопреки README — передано CLO | ST-51, AF-25 | 12 | Open (CLO) |

### 3.5 Соответствие прежних идентификаторов (что заменено)
- **T-01…T-16** (`03-threat-model.md`) — заменены: T-01→SEC-01/02/03/04, T-02→SEC-06/07, T-03→SEC-08, T-04→SEC-12, T-05→SEC-11, T-06→SEC-14, T-07→SEC-16, T-08→SEC-15, T-09→SEC-18/34, T-10→SEC-29/30/31/L1, T-11→SEC-38, T-12→SEC-20, T-13→SEC-09/L5, T-14→SEC-35/36/37, T-15→SEC-23, T-16→SEC-33.
- **Условия C-1…C-10, C-1′…C-8′** (аудит и перекрёстное ревью) — заменены условиями G4-C1…C5 и G4-P1…P4 (§1).
- **XR-01, XR-02** объединены в SEC-10; **XR-04, XR-10** — в SEC-11; **XR-05, XR-06** — в SEC-14; **XR-07** — SEC-02; **XR-08** — SEC-01/SEC-21; **XR-09** — SEC-22; **XR-03** — SEC-17.
- **ST-05, ST-06** объединены в SEC-11; **ST-08, ST-11** — в SEC-14 (ST-11 также в SEC-07); **ST-19, ST-20** — в SEC-16; **ST-42, ST-44** — в SEC-34.
- Вердикт «BLOCK GATE 2» (`sec-architecture-audit.md` §4, `review-sec-on-architecture.md` §4) заменён вердиктом GATE 4 (V3) в §1.
- **SR-21** (прерывание при физическом вводе) — исключено как невыполнимое без хуков; заменено проверкой между пакетами (SEC-14). **SR-10** — заменено порядком W1 → W3 (порог профиля, по умолчанию 200) → W2 → отказ. **SR-22** — заменено решением X-4. **SR-81** (HMAC) — перенесено на GATE 8.

### 3.6 Автоматические обновления безопасности (новое, V7 п. 3, D-25)

Все записи — проектные: кода апдейтера, ключей и релизного workflow нет ([проверено CISO]: grep `updater`/`pubkey`/`secrets`/`environment` в `app/src-tauri`, `.github/workflows` — 0 совпадений). Детальные требования — `docs/EMERGENCY_PATCH_POLICY.md`, `docs/SIGNING_KEY_MANAGEMENT.md`, условия §4.8 и G10-1…G10-12. Владельцы: 03 ARCH (проект), 09 REL (конвейер, ключи), 05/19 (апдейтер на платформах), 08 QA, 12 CLO (приватность), владелец (LEVEL 3).

| ID | Название | Уровень | Владелец | P | Решение / исправление | Проверка (метод → результат) | Остаточный риск | Статус |
|---|---|---|---|---|---|---|---|---|
| SEC-40 | Компрометация/утрата ключа подписи обновлений или корня доверия → вредоносное обновление всем пользователям с автоустановкой | **Critical** | 09, владелец (LEVEL 3) | P1 | Разделение K-1 (офлайн, ≥ 2 корня) / K-2 / K-3; секреты только в environment с обязательным ревьюером; журнал подписей и сверка; отзыв и переустановка доверия (`SIGNING_KEY_MANAGEMENT.md` §9–10) | G10-3, G10-4, G10-6 → **не выполнено** (ключей нет) | High до появления второго лица и KMS; при утрате обоих корней — только внеполосная переустановка. Принять может **только владелец** | Open |
| SEC-41 | Атаки на метаданные: downgrade/rollback, freeze/replay, смешивание выпусков, чужая платформа/архитектура, бесконечные данные | High | 03, 05, 19 | P1 | Подписанные метаданные с версией, минимально допустимой версией, сроком годности, платформой/арх., хэшем; монотонность; лимиты размера/времени; ориентир — роли TUF | G10-5 → не выполнено | Low | Open |
| SEC-42 | Компрометация хоста раздачи/DNS/TLS, MITM, подмена пакета, «отказ в обновлении» | High | 03, 09 | P1 | Доверие только подписи (TLS — вторичный слой); утверждённые хосты, без редиректов на чужие; ошибка → оставить текущую версию; бюллетень по второму каналу | G10-5 → не выполнено | Medium (заморозка обновлений злоумышленником в сети; обнаруживается сроком годности метаданных и предупреждением) | Open |
| SEC-43 | Emergency-политика как канал удалённого управления: включение функций, ослабление исключений, исполнение кода, «удалённый kill» | High | 03, CISO | P1 | Только ограничения из закрытого перечня, отдельный ключ K-3, срок ≤ 30 дней, монотонный номер, `deny_unknown_fields`, ≤ 16 КиБ, видимость пользователю (`EMERGENCY_PATCH_POLICY.md` §6) | G10-7 → не выполнено | Low (злоумышленник с K-3 может лишь временно выключить функции — DoS) | Open |
| SEC-44 | Приватность канала: IP, версия, отпечаток устройства, идентификаторы, попутная телеметрия; противоречие обещанию «всё локально» | Medium | 03, 12 CLO | P1 | Статический файл метаданных без параметров запроса, без cookies и ID; общий User-Agent; никакой телеметрии/отчётов о сбоях через канал; журналы хоста — решение CLO; раскрытие в «Приватность»; отключаемо администратором | G10-9, G10-10 → не выполнено | Low (IP и время запроса видны хосту — раскрывается) — **CISO примет после G10-9 и заключения CLO** | Open |
| SEC-45 | Автоустановка прерывает активную операцию → порча документа, утрата снимка буфера (Q-1/D-26), потеря черновика макроса; неудачное обновление без отката | High | 05, 19, 08 | P1 | Безопасный момент, отсрочка, сохранение черновика, сохранение предыдущей версии, самопроверка и авто-откат (`EMERGENCY_PATCH_POLICY.md` §8.2, `UPDATE_FAILURE_RECOVERY.md`) | G10-8 → не выполнено | Low | Open |
| SEC-46 | Установка пакета: TOCTOU между проверкой и запуском, DLL planting в каталоге загрузки, запуск установщика из общедоступного каталога, запрос повышения прав | High | 05, 19, 09 | P1 | Загрузка в каталог пользователя с ограниченным ACL; проверка подписи над тем же открытым дескриптором/копией, которая исполняется; установщик `currentUser` без повышения (правило 8); проверка Authenticode/`codesign` + Team ID дополнительно к K-2; без поиска DLL в каталоге загрузки | ревью кода + Windows/macOS CI → не выполнено | Low | Open |
| SEC-47 | Компрометация конвейера сборки и подписи (Actions, стороннее action, кэш, зависимость в сборке) → легитимно подписанный вредоносный выпуск, автоустановка Critical/High | **Critical** | 09, CISO | P1 | Отдельный релизный workflow; actions по SHA из утверждённого списка; подпись отделена от сборки; `--locked`, `--ignore-scripts`; без кэшей в релизе; provenance + публичные хэши; environment с ревьюером; защита `main` (SEC-39); ориентир на воспроизводимость | G10-4 → не выполнено | High до SEC-39 и выделенного workflow; принять может **только владелец** | Open |
| SEC-48 | Корпоративные среды: невозможность отключить канал или, наоборот, бессрочно отложенные Critical | Medium | 03, 17 (B2B), 05, 19 | P1 | Отключение/отсрочка политикой администратора (HKLM, управляемые настройки macOS) без обхода; пределы отсрочки 7/14 дней; статус в «Диагностике» | G10-10 → не выполнено | Low — **CISO примет после G10-10** | Open |
| SEC-49 | Сетевой код как новая поверхность атаки (TLS, HTTP, распаковка, разбор метаданных) в процессе с доступом к тексту пользователя | High | 03, 09 | P1 | Сетевой стек и разбор — только в отдельном компоненте/процессе обновлений без доступа к тексту, буферу и UIA; лимиты размера; распаковка с лимитами; fuzz разборщиков; бан сетевых crate в остальных crate (`deny.toml` wrappers) | G10-11, fuzz → не выполнено | Low | Open |

### 3.7 macOS — интеграция с ОС (новое, V7 п. 2, D-24)

Кода macOS-адаптера нет; специалиста по безопасности macOS в команде нет (в `docs/TEAM_STRUCTURE.md` есть AGENT 19 macOS Platform Lead — разработка, не проверка). **Требую** от PD назначить проверяющего по безопасности macOS (аналог AGENT 15) до начала macOS-спайков. Поведение API ниже — по документации Apple в моей памяти, в этой сессии не перепроверено — **UNVERIFIED** до macOS-спайков на `macos-latest`. Условия **GATE 8 (security-часть) → PASS**: все SEC-50…57 не выше Medium или Verified; macOS-CI зелёный; ручная приёмка владельцем на реальном Mac по чек-листу (аналог D-10).

| ID | Название | Уровень | Владелец | P | Решение / исправление | Проверка (метод → результат) | Остаточный риск | Статус |
|---|---|---|---|---|---|---|---|---|
| SEC-50 | Разрешение Accessibility (TCC) даёт KeySwitch возможность читать и вводить текст во всех приложениях → приложение и его обновления становятся ценной целью; сброс/наследование разрешения при смене подписи | High | 19, 03 | P0 | Запрос разрешения только при первом использовании, с объяснением; без Input Monitoring и Screen Recording; чтение — только сфокусированного элемента по горячей клавише; стабильные Team ID и bundle id (смена = LEVEL 3, `SIGNING_KEY_MANAGEMENT.md` §6.2); hardened runtime без `disable-library-validation`; деградация до «нет разрешения → отказ» | macOS-спайк: без разрешения — отказ; ревью entitlements → не выполнено | Medium (разрешение по природе широкое) | Open |
| SEC-51 | Поля паролей и Secure Event Input на macOS | High | 19 | P0 | Отказ **до чтения**, если `IsSecureEventInputEnabled()` или роль/подроль элемента `AXSecureTextField`, плюс эвристики SEC-15 и исключения D-05 (менеджеры паролей, терминалы, клиенты удалённого доступа) | macOS CI: `NSSecureTextField` → 0 вызовов чтения значения → не выполнено | Medium (кастомные поля без признаков) | Open |
| SEC-52 | NSPasteboard: утечка временного текста в Universal Clipboard (другие устройства), менеджеры истории буфера; уведомления о программном чтении буфера в новых версиях macOS; нет аналога `WM_RENDERFORMAT`-сигнала | High | 19, 03 | P0 | Временные данные — с `prepareForNewContents(with: .currentHostOnly)` и маркерами `org.nspasteboard.TransientType`/`ConcealedType` (соглашение nspasteboard.org, не гарантия Apple); стратегия через буфер — только по профилю; Q-1/D-26: без подтверждения потребления не восстанавливать, «Вернуть буфер»; чтение буфера макросом — по-прежнему по разрешению слота; поведение запросов приватности буфера в macOS 15/26 — **UNVERIFIED**, проверить в спайке | macOS CI: маркеры присутствуют; ручная проверка Handoff на двух устройствах → не выполнено | Medium (сторонние менеджеры буфера игнорируют маркеры — как SEC-12-R) | Open |
| SEC-53 | Login Items / автозапуск: подмена плиста LaunchAgent, устойчивость чужого кода от имени KeySwitch | Medium | 19 | P1 | Только `SMAppService.mainApp` (macOS 13+), без записи плистов в `~/Library/LaunchAgents`; без аргументов запуска (как SR-75) | ревью кода + macOS CI → не выполнено | Low | Open |
| SEC-54 | Без Developer ID и нотаризации: Gatekeeper блокирует запуск, пользователей приучают обходить защиту; подделка дистрибутива | High | 09, владелец (LEVEL 3) | P2 | Developer ID + hardened runtime + нотаризация + staple для бандла и каждого вложенного исполняемого файла; раздельные минимальные entitlements; инструкции «обойти Gatekeeper» не публикуются | `codesign --verify --deep --strict`, `spctl -a -vv`, `stapler validate` в CI → не выполнено | — (решение владельца по оплате Apple Developer Program) | Open (требует владельца) |
| SEC-55 | Второй слой песочницы на macOS: нет аналога AppContainer/Job; JIT Cranelift требует `allow-jit` | High | 13, 03, 19 | P0 | Помощник `ks-sandbox` — отдельный исполняемый файл с App Sandbox без entitlements (нет сети, нет файлов вне контейнера), hardened runtime; `allow-jit` — только у помощника и только при Cranelift (Pulley — без JIT); лимиты `setrlimit`, сторожевой таймер и убийство процесса основным; ноль preopen (§4.6) | macOS-спайк S-3m: H-01…H-40 на `macos-latest`, самопроверка sandbox-статуса → не выполнено | High до S-3m (как SEC-01); принять может **только владелец** | Open |
| SEC-56 | Горячие клавиши и синтетический ввод на macOS: соблазн `CGEventTap` (аналог LL-хука), ввод не в то приложение | High | 19 | P0 | Горячие клавиши — только `RegisterEventHotKey` (Carbon) или эквивалент без Input Monitoring; **`CGEventTap`/Input Monitoring запрещены** (аналог правила 8 и D-14 — предлагаю PD записать D-xx); `CGEventPost` — пакетами с проверкой frontmost-приложения (PID + время запуска) и модификаторов перед каждым пакетом, как SEC-14; прерывание при блокировке экрана/смене пользователя | ревью + macOS CI → не выполнено | Low–Medium | Open |
| SEC-57 | Отчёты о сбоях и журналы на macOS: ReportCrash (`.ips`), «Отправить в Apple/разработчику», unified logging | Medium | 19, 05 | P1 | Panic hook только `file:line`; `os_log` — только закрытая схема с `%{private}` по умолчанию, без текста; `UserText` на всех путях; README — раскрытие поведения системных отчётов | crash-тест на `macos-latest`: в `.ips` нет синтетического текста → не выполнено | Low | Open |

---

## 4. Решения CISO (в пределах полномочий)

Ни одно решение ниже не принимает остаточный риск Critical/High.

### 4.1 Q-1 — нет восстановления буфера без подтверждённого сигнала потребления
**Решение:** восстанавливать прежний буфер **только** если (а) после нашего Ctrl+V пришёл `WM_RENDERFORMAT` от процесса цели (или брокера, подтверждённого для профиля в S-2) **и** (б) пост-проверка поля дала `Yes`. При `Unknown` — восстановление только для профиля, где сигнал подтверждён в S-2. Во всех остальных случаях (таймаут, NULL/чужой PID, `WM_RENDERFORMAT` до Ctrl+V) — **не восстанавливать**: подставить реальные данные вместо отложенных (с метками D-13), уведомить пользователя, дать действие «Вернуть буфер». Снимок хранится в обнуляемой памяти **не дольше 120 с или до следующей операции**, затем уничтожается.
**Обоснование:** вставка прежнего буфера (часто пароля из менеджера) в чат — утечка третьей стороне (High, ST-01); утрата прежнего содержимого буфера — восстановимая потеря данных (Medium). Без надёжного сигнала «нет сигнала» ≠ «вставки не было» (XR-02). Частота мала: W2 — только по профилю, после W1/W3.
**Остаточный риск:** Medium — пользователь может потерять прежнее содержимое буфера, если не нажмёт «Вернуть буфер» за 120 с. Принят CISO с точки зрения безопасности; поскольку это риск утраты данных (V3 ч. 11), владелец **информируется** (§5, п. 6). Требует правки FR-CLIP-2 (AGENT 02).

### 4.2 Q-3 — процедура исключения из правила «версия не моложе 2 недель» для security-патчей
Исключение применяется, только если выполнены **все** условия:
1. Обновление устраняет опубликованное advisory (RUSTSEC/GHSA/CVE) в пакете из нашего графа (Wasmtime-семейство, Tauri/wry/WebView2-обвязка, SQLite, npm-пакеты сборки) — идентификатор указан в PR.
2. Это **патч-версия той же поддерживаемой линии** (например, 48.0.x LTS, Tauri 2.x.y); смена minor/major — по обычному правилу.
3. Издатель не изменился: владельцы crate на crates.io / npm те же, версия соответствует тегу в официальном репозитории; версия не yanked.
4. Возраст версии **≥ 72 ч** (защита от отозванных/вредоносных публикаций); для Critical, достижимого в нашей конфигурации, CISO может сократить до 24 ч письменно в PR.
5. Diff `Cargo.lock`/`package-lock.json` ограничен этим пакетом и его семейством; новые транзитивные зависимости — только с отдельным обоснованием по правилу 3.
6. Зелёные: `scripts/check.sh`, весь CI, включая Windows-job и (когда появятся) H-01…H-40; `cargo deny` без новых `ignore`.
7. PR с меткой `security-patch`: advisory, возраст версии, достижимость для нашей конфигурации, оценка AGENT 07; **одобрение CISO** обязательно.
8. SLA (от публикации advisory): Critical/High, достижимые в нашей конфигурации — PR готов ≤ 72 ч, сборка ≤ 7 дней; Medium — ≤ 30 дней; недостижимые (выключенные фичи) — обычное правило, допускается `ignore` в `deny.toml` с обоснованием и сроком ≤ 30 дней.
9. Слияние в `main` и релиз — по-прежнему только с подтверждения владельца (правило 6); процедура это не меняет.
Первое применение: `wasmtime =48.0.8` (опубликован 2026-10-29, закрывает RUSTSEC-2026-0422/0423/0424) — допустим в S-3 по этой процедуре уже сейчас.
**Формальность:** текст правила 3 в `CLAUDE.md` меняет только владелец — нужна запись D-xx (§5, п. 5). До неё процедура действует как политика CISO для спайков и веток, не для `main`.

### 4.3 Q-8 — сторонние слушатели буфера, игнорирующие метки приватности
**Решение:** остаточный риск **SEC-12-R принят CISO как Medium.**
**Обоснование:** временные данные — уже видимый пользователю текст из обычного поля (поля паролей исключены до чтения, SEC-15); W2 используется только по профилю после W1/W3; метки D-13 ставятся; `WM_RENDERFORMAT` до Ctrl+V прерывает W2 (ранний слушатель обнаруживается); при RDP W2 запрещён. Контролировать чужие процессы того же пользователя невозможно (вне модели).
**Условия принятия:** раскрытие в README/«Приватность»; пересмотр, если S-2 покажет, что W2 нужен большинству целевых приложений, или если появится запись в буфер текста не из поля (макросы).

### 4.4 Q-9 — процесс песочницы на каждый запуск при доступе к буферу
**Решение:** для слота с разрешением чтения буфера — **новый процесс на каждый запуск**, `TerminateJobObject` сразу после результата. Для слотов без буфера допускается «тёплый» процесс: простой ≤ 60 с; новый `Store`/инстанс на каждый запуск (без общего гостевого состояния); перезапуск при любом изменении скрипта, блокировке/смене сессии; «тёплый» процесс **никогда** не получает содержимое буфера и не переиспользуется для слота с буфером.
**Обоснование:** данные буфера — наиболее чувствительный вход песочницы; общий процесс создаёт канал между запусками/слотами при ошибке хоста (ST-27, ST-34). Для прочих слотов изоляция инстанса достаточна, выигрыш старта CPython существенен.

### 4.5 X-4 — окно выбора языка смешанного текста
**Решение:** по умолчанию — **`WS_EX_NOACTIVATE`** (фокус поля не меняется) + временные `RegisterHotKey` для `1`/`2`/`Esc`, зарегистрированные потоком T1 **только на время показа**, со снятием RAII-guard на любом выходе и по таймауту 10 с. Если регистрация любого хоткея не удалась — операция отменяется (никаких хуков как запасного пути). Окно не показывает текст пользователя; после выбора — перечитать, сравнить хэш, перепроверить отпечаток/foreground.
**Запасной вариант (активация окна)** — только если S-2 покажет неработоспособность основного, и только после повторного одобрения CISO по данным S-2, при условиях: не возвращать фокус, если передний план сменился на чужое окно; снять `WS_EX_TOPMOST` после выбора; S-2 подтверждает, что blur в веб-композерах ничего не отправляет.
**Обоснование:** без активации исчезают побочные эффекты blur и гонка возврата фокуса (ST-08); временные хоткеи соответствуют D-14/правилу 8. Кратковременный (≤ 10 с) перехват `1/2/Esc` в других окнах — приемлемая плата.

### 4.6 XR-07 — stdlib Python из памяти, ноль preopen — обязательно по умолчанию
**Решение:** `WasiCtx`/собственный слой WASI **не содержит ни одного host-каталога** для обоих рантаймов; stdlib — встроенный zip (`include_bytes!`) через неизменяемое дерево в памяти (только чтение; изменяющие вызовы → `EROFS/ENOTCAPABLE`); ускорение — Wizer-снимок в CI с пересевом PRNG/хэшей при старте. Обязательный unit-тест «список preopen пуст» и проверка импортов в CI.
**Запасной вариант** (preopen каталога с единственным файлом `python314.zip`) — только письменным решением CISO по итогам S-3, если вариант A доказанно не укладывается, и только при всех условиях: `FilePerms::READ`, ACL только чтение для SID контейнера, нет symlink/junction, SHA-256 до запуска, H-01…H-12 + тесты symlink/hardlink на `windows-latest`, последний патч 48.x.
**Обоснование:** 5 из 9 advisory `wasmtime-wasi` за 2026 — в коде ФС (по данным AGENT 07); preopen делает его достижимым, каталог в `%LOCALAPPDATA%` доступен пользователю на запись (TOCTOU). Критическая угроза ST-23 устраняется построением, а не настройкой.

### 4.7 Прочие решения CISO
- **SEC-24-R** (синтетический запуск хоткея процессом того же пользователя Medium IL) — **принят как Medium**: без LL-хука (запрещён D-14) неотличим; процесс того же пользователя и так может вызвать то же действие; процессы Low IL блокируются UIPI; каждая операция всё равно проходит все проверки поля.
- **SR-21** заменено (см. §3.5); **порог W3** — параметр профиля, по умолчанию 200 символов, повышение только по замерам S-2 с ревью CISO.

### 4.8 D-25 — изменение сетевого принципа: условия CISO для канала обновлений
Владелец (V7, D-25) заменил D-22(4): обновления безопасности **включены по умолчанию**, правило «нет сети в runtime» уточнено — разрешён **единственный** канал. Это решение владельца; CISO его не оспаривает, но одобрит реализацию (GATE 10) только при выполнении **всех** условий:
1. **Единственность:** сетевые соединения устанавливает только компонент обновлений (рекомендуется отдельный процесс без доступа к тексту, буферу, UIA/AX); основной процесс, WebView и песочница — 0 соединений (SR-90 в редакции §1, условие 3 GATE 12). Иные сетевые функции (телеметрия, crash-reporting, онлайн-лицензия, загрузка словарей) этим решением **не разрешены** — каждая требует отдельного решения владельца и нового ревью.
2. **Утверждённые хосты** зашиты в бинарь; только HTTPS; редиректы — только в пределах списка; системный прокси допускается (корпоративные сети), так как доверие строится на подписи, не на TLS.
3. **Без пользовательских данных:** запрос статического файла метаданных без параметров, cookies, идентификаторов устройства/установки; общий User-Agent; выбор версии — локально в клиенте.
4. **Доверие — только подписи** встроенными ключами (`SIGNING_KEY_MANAGEMENT.md`); проверка до разбора содержимого сверх фиксированного лимита размера; защита от downgrade/freeze/подмены платформы; в релизе нет обхода проверки.
5. **Fail-safe:** любая ошибка → текущая версия сохраняется, повтор с отсрочкой; никогда «установить без проверки».
6. **Отключаемо администратором** (Windows HKLM-политика, macOS управляемые настройки) без обхода; статус виден пользователю.
7. **Emergency-политики** — только ограничения из закрытого перечня, отдельный ключ, без кода (`EMERGENCY_PATCH_POLICY.md` §6).
8. **Автоустановка Critical/High** — только по `EMERGENCY_PATCH_POLICY.md` §8 (безопасный момент, отсрочка, откат, без повышения прав).
9. **Зависимости:** сетевые crate (TLS, HTTP) — только в компоненте обновлений, закреплены в `deny.toml` (wrappers); `tauri-plugin-updater` или иной апдейтер — по правилу 3 `CLAUDE.md` и только после одобрения проекта; до этого бан §6 п. 3 действует.
10. **Прозрачность:** «Приватность»/README описывают хост, что передаётся (IP, время, версия в пути файла, если есть), как отключить; журнал апдейтера локальный, без текста.
11. **Приватность** — заключение CLO (IP как персональные данные, ePrivacy ст. 5(3), `LEGAL_COMPLIANCE_REGISTER.md`).
**Остаточный риск** изменения принципа (новая поверхность атаки и цепочка доверия) — Critical/High (SEC-40, SEC-47), принимается **только владельцем** после G10-1…G10-12.

---

## 5. Требует решения владельца

| № | Вопрос | Позиция CISO | Когда |
|---|---|---|---|
| 1 | **SEC-01 / X-14:** итоговый остаточный риск выхода из Wasm (ожидается High даже при LPAC+Job; без ACG — выше) | Не принимаю. Представлю данные S-3 (Pulley+ACG vs Cranelift) с рекомендацией | после S-3, до GATE 8 |
| 2 | **SEC-19:** подпись кода (D-11, платно) | Публичный релиз без подписи — не пропущу на GATE 10 без письменного решения владельца | до GATE 12 |
| 3 | **SEC-39:** включить защиту `main` (обязательный PR + статусы CI, запрет force-push), secret scanning, push protection, Dependabot alerts — бесплатно для публичного репозитория, делает только администратор | Рекомендую включить сейчас | сейчас (условие G4-P4) |
| 4 | **SEC-31/SEC-38:** поведение WebView2 Runtime (сеть, отчёты о сбоях) вне контроля проекта | Решение по данным SR-90 и ручной проверки; если будет сеть — это противоречит обещанию «всё локально» | GATE 10 |
| 5 | **Q-3:** внести в `CLAUDE.md` правило 3 исключение по процедуре §4.2 (запись D-xx) | Рекомендую | сейчас |
| 6 | **Q-1 (информирование):** редкая утрата прежнего содержимого буфера (действие «Вернуть буфер» 120 с) вместо риска вставки пароля в чат | Решение принято CISO; сообщаю как риск утраты данных (V3 ч. 11) | к сведению |
| 7 | ~~SEC-32: механизм уведомления об обновлениях без сети~~ — **решено D-25** (автообновления); вопрос заменён п. 8–11 | — | — |
| 8 | **SEC-40/SEC-47 (Critical) и LEVEL 3 по ключам:** корень доверия (≥ 2 корня), механизм подписи, KMS/HSM или аппаратные токены, сертификат Windows, Apple Developer Program, второе доверенное лицо, хост раздачи (`SIGNING_KEY_MANAGEMENT.md` §11) | Без этих решений GATE 10 не пройдёт; остаточный Critical/High принимает только владелец | до GATE 10 |
| 9 | **Заранее одобренный EMERGENCY-процесс (V7 п. 11):** рекомендую оставить владельца обязательным ревьюером environment `release-emergency` (одно действие), остальное — автоматически; `SECURITY.md` и канал приёма сообщений (GitHub Private Vulnerability Reporting) | Рекомендую | до GATE 10 |
| 10 | **macOS:** назначить проверяющего по безопасности macOS; записать D-xx «на macOS без `CGEventTap`/Input Monitoring» (аналог D-14) | Требую до macOS-спайков | до старта M-спайков |
| 11 | **SEC-55:** остаточный риск песочницы на macOS (как SEC-01) | Не принимаю до данных S-3m | до GATE 9 |

---

## 6. Базовые контроли безопасности для каждого PR

Проверяются автором (самопроверка в описании PR) и ревьюером по `docs/security/security-team-plan.md`. Невыполнение любого пункта — **блокирует**.
1. `scripts/check.sh` — Summary в описании PR; CI на ветке зелёный (все 4 job); утверждения разделены на «проверено (как, где)» / «не проверено (почему)».
2. Текст пользователя: вне `ks-core` — только `UserText`; нет `Debug`/`Display`/`Serialize`/`Clone` у типов с текстом; текст не попадает в логи, `panic!`/`expect`-сообщения, события/команды WebView, файлы, тестовые снапшоты (только синтетические данные).
3. Сеть: нет crate из бан-листа SR-62, нет Tauri-плагинов `fs`/`shell`/`http`/`updater`/`websocket`/`upload`; изменения `capabilities/*.json`, CSP, `tauri.conf.json` — только с ревью CISO. **Исключение D-25:** сетевой код и апдейтер — только в компоненте обновлений после одобрения проекта на GATE 10 (§4.8); до этого запрет действует полностью. Любые ключи, сертификаты, пароли, токены — никогда в репозитории, коде, фронтенде, журналах и у ИИ-агентов (`SIGNING_KEY_MANAGEMENT.md` §1).
11. macOS: нет `CGEventTap`, Input Monitoring, Screen Recording, `disable-library-validation`; entitlements меняются только с ревью CISO.
4. `unsafe` — только в разрешённых crate (SEC-L2), каждый блок с `// SAFETY:`; workspace-lint `forbid` не ослабляется.
5. Запрещено: `SetWindowsHookEx`/LL-хуки, `RIDEV_INPUTSINK`, `CreateRemoteThread`/`WriteProcessMemory`/инъекции, запрос повышения прав (`runas`, `requireAdministrator`, `uiAccess=true`), `ValuePattern.SetValue` для замены.
6. Пользовательский код исполняется только в `ks-sandbox.exe`; каждая новая хост-функция или импорт WASI — fuzz-цель + ревью CISO; preopen запрещён (§4.6).
7. Новая/обновлённая зависимость: назначение, лицензия, риск, точная версия `=`, возраст ≥ 14 дней (или процедура §4.2), diff lock-файлов перечислен, `cargo deny` и `npm audit` проходят, `ignore` — только с обоснованием и сроком.
8. GitHub Actions: только по SHA с комментарием версии; `permissions` минимальные; нет `pull_request_target`, новых секретов, `--ignore-scripts`/`--locked` не удаляются.
9. Изменения буфера, фокуса, записи в поле, песочницы — с доказательством на `windows-latest` (лог CI) или явной пометкой «не проверено на Windows», тогда статус задачи не выше Review.
10. Один поток = одна ветка = один PR; нет прямых push в `main`; слияние в `main` — только с подтверждения владельца.
