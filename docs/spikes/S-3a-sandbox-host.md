# S-3a — Песочница макросов: слой Wasmtime (хост, протокол, враждебные модули)

**Вердикт: слой Wasmtime подтверждён на Linux, Windows и macOS (CI); изоляция процесса ОС (LPAC/Job, App Sandbox) — не проверялась, это S-3b/S-3c.** Остаточный риск SEC-01/SEC-55 не меняется и не принимается этим спайком.

Ветка: `spike/sandbox-host`. Владелец потока: AGENT 13. Ревью: CISO (SEC-01, SEC-21, SR-40…SR-60).

## Что сделано
- `crates/ks-sandbox-proto` — протокол хост↔гость (JSON), без I/O и без `unsafe`:
  - кадр гость→хост ≤ 64 КиБ, хост→гость ≤ 256 КиБ; проверка размера и глубины (≤ 8, линейный сканер до serde) **до** парсинга;
  - ровно одно сообщение-результат с `request_id`; чужой/устаревший id → `RequestIdMismatch`; версия `v` = 1;
  - `deny_unknown_fields` везде, дубликаты полей → отказ; `None {}` — структурный вариант, чтобы и он отвергал лишние поля;
  - `insert`/`replace` ≤ 5 000 символов (скалярных значений Unicode); запрещены C0 (кроме `\n`/`\t`), DEL, C1, U+2028/2029, U+FFFE/FFFF, bidi (U+061C, U+200E/F, U+202A–E, U+2066–9), невидимые (U+00AD, U+180E, U+200B–D, U+2060–4, U+FEFF, U+FFF9–B, теги U+E0000–E007F); `\r` запрещён всегда;
  - `keys`: 1…16 клавиш закрытого `AllowedKey` (Enter, Tab, Backspace, Delete, стрелки, Home/End, PageUp/PageDown); нет модификаторов, Escape, F-клавиш, Win; только форма `"Left"` (самописный `Deserialize`);
  - `Enter`/`Tab` и `\n`/`\t` в тексте — только при `SlotPermissions::enter_tab` (SR-49, по умолчанию выключено);
  - `ProtoText`/`Action` без `Debug`/`Display`; ошибки — только коды, без эха данных гостя (тест-маркер);
  - framing для pipe (S-3b): 4 байта LE длины, отказ по длине до выделения памяти.
- `crates/ks-sandbox` — хост Wasmtime + бинарь `ks-sandbox` (stdin/stdout, коды выхода):
  - **никакого WASI**: `Linker` содержит только `ks.emit`, `ks.request_len`, `ks.request_read`; любой другой импорт (включая `wasi_snapshot_preview1.*`, `wasi:*`, `env.*`, импорт памяти/глобала) → `ImportDenied` до инстанцирования;
  - epoch-interruption, сторожевой поток, дедлайн 2 с (включая `start`-функцию); нет сторожа — гостевой код не запускается (fail closed);
  - `ResourceLimiter`: память ≤ 64 МиБ (рост сверх — trap, не −1), 1 инстанс, 1 память, 1 таблица, ≤ 20 000 элементов; `max_wasm_stack` 512 КиБ;
  - выключены предложения Wasm (SR-41): threads, shared-everything-threads, tail-call, multi-memory, exceptions (+legacy), memory64, component-model, function-references, GC, memory-control, custom-page-sizes, stack-switching, wide-arithmetic, relaxed-SIMD, custom-descriptors;
  - хост-функции без `unsafe`: `checked_add`, проверка длины ≤ 64 КиБ **до** копирования и выделения, только безопасный API памяти Wasmtime;
  - новый `Store` на каждый запуск (SR-51); кадр, выданный перед trap/таймаутом, отбрасывается; повторный `emit` → нарушение;
  - `Outcome` без `Debug`; в журнал — только `OutcomeCode`. Бинарь ничего не пишет в stderr и пишет в stdout только уже провалидированный кадр.
- `scripts/check.sh`: clippy и тесты песочницы, кросс-clippy под `x86_64-pc-windows-msvc`.
- CI: job `sandbox`, матрица `ubuntu-latest` / `windows-latest` / `macos-15`, `permissions: contents: read`, actions закреплены по SHA.

## Зависимости (правило 3; CISO Q-3)
Прямые:

| Crate | Версия | Опубликован | Лицензия | Фичи | Зачем |
|-------|--------|-------------|----------|------|-------|
| `wasmtime` | =48.0.8 | 2026-10-29 | Apache-2.0 WITH LLVM-exception | `default-features = false`; `runtime`, `std`, `cranelift` | Исполнение Wasm. Моложе 2 недель: допущен по процедуре CISO Q-3, `docs/SECURITY_REVIEW.md` §4.2 («первое применение — `wasmtime =48.0.8`», закрывает RUSTSEC-2026-0422/0423/0424), SEC-01, SEC-21; решение владельца D-22. Не включены: `cache`, `component-model(-async)`, `gc*`, `threads`, `pooling-allocator`, `async`, `wat`, `parallel-compilation`, `profiling`, `coredump`, `debug*`, `addr2line`, `backtrace`, `demangle`, `winch`, `anyhow` |
| `serde` | =1.0.229 | 2026-07-18 | MIT OR Apache-2.0 | `std`, `derive` | Схема протокола; уже в графе через Tauri |
| `serde_json` | =1.0.154 | 2026-10-11 | MIT OR Apache-2.0 | `std` | JSON протокола; уже в графе через Tauri |
| `wat` (dev) | =1.254.2 | 2026-10-02 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | `default-features = false` | Только тесты: WAT→wasm для враждебных модулей. Та же версия, что пинит сам wasmtime 48.0.8; парсер текста не попадает в бинарь песочницы |

Новые транзитивные (тянет `wasmtime` 48.0.8; все ≥ 2 недель, кроме семейства wasmtime/cranelift/pulley 48.0.8/0.135.8 от 2026-10-29 — Q-3 п. 5):
`cranelift-*` 0.135.8, `pulley-interpreter`/`pulley-macros` 48.0.8, `wasmtime-internal-*` 48.0.8, `wasmtime-environ` 48.0.8 (Apache-2.0 WITH LLVM-exception; 2026-10-29); `wasmparser`/`wasm-encoder`/`wasmprinter` 0.254.2 (2026-10-02); `regalloc2` 0.15.2 (2026-07-20); `object` 0.39.1 (2026-04-21); `gimli` 0.33.0 (2026-01-24); `rustix` 1.1.6 (2026-10-27); `linux-raw-sys` 0.12.1; `memfd` 0.6.6 (2026-09-01); `mach2` 0.6.0 (macOS); `postcard` 1.1.3; `cobs` 0.3.0; `target-lexicon` 0.13.5; `hashbrown` 0.16.1; `itertools` 0.14.0; `either` 1.19.0 (2026-10-06); `arbitrary` 1.5.0 (2026-10-06); `libm` 0.2.16; `allocator-api2` 0.2.21; `leb128fmt` 0.1.0; `termcolor` 1.4.1; `errno` 0.3.14; `async-trait` 0.1.92 (proc-macro). Только для тестов: `wast` 254.0.2 (2026-10-02), `unicode-width` 0.2.2 (2025-10-06; закреплена вместо 0.2.3 от 2026-10-28, чтобы не быть на границе 2 недель). Лицензии — MIT/Apache-2.0/BSD-2/Unlicense-варианты; `cargo deny --all-features check` — `advisories ok, bans ok, licenses ok, sources ok` (Linux, 2026-11-11). Даты — crates.io API.

## Враждебные модули → H-xx (`crates/ks-sandbox/tests/hostile.rs`, `process.rs`)

| H / SR | Тест | Ожидание |
|--------|------|----------|
| H-20, SR-52 | `h20_infinite_loop_terminated_by_deadline`, `…_in_start_function_…`, `h20_emit_then_loop_discards_frame`, `short_deadline_is_honoured` | `Timeout`, 2 с ≤ время ≤ 2,5 с; кадр до таймаута отброшен |
| H-22 | `h22_deep_recursion_traps_and_host_survives` | `StackOverflow`; тот же хост затем успешно выполняет модуль |
| H-23, SR-52 | `h23_memory_grow_loop_denied`, `h23_single_huge_grow_denied`, `h23_huge_initial_memory_denied_at_instantiation`, `h23_table_bomb_denied`, `h23_growth_up_to_limit_allowed` | `ResourceLimit`; ровно 64 МиБ — разрешено |
| H-05/H-06/H-07, SR-43 | `wasi_env_and_unknown_imports_rejected` (fd_write, environ_get, path_open, proc_exit, sock_accept, `wasi:sockets`, `wasi:http`, `env.system`, `env.memory`, `ks.exec`, `ks.emit` как global/memory, `KS.emit`) | `ImportDenied` до инстанцирования |
| SR-41 | `sr41_disabled_proposals_rejected` (threads, memory64, multi-memory, tail-call, exceptions, function-references, GC, relaxed-SIMD) | `InvalidModule`; MVP + bulk-memory + SIMD компилируются |
| SR-44 | `sr44_out_of_bounds_pointers_are_violations`, `request_read_with_small_or_oob_buffer_is_violation`, `emit_without_exported_memory_is_violation` | `Violation(OutOfBounds/BufferTooSmall/NoMemory)` |
| H-25, SR-46 | `h25_oversized_emit_rejected_before_copy` (64 КиБ+1, 2 ГиБ, −1), `h25_text_over_5000_chars_rejected`, `sr46_duplicate_emit_is_violation`, `malformed_frames_rejected` (не JSON, неизвестный тип/поле, чужой id, версия, глубина 10, не-UTF-8, пустой кадр) | `Violation(FrameTooLarge)` / `FrameRejected(...)` |
| H-26 | `h26_non_whitelisted_keys_rejected` (Ctrl, Escape, Win, `{"Enter":null}`, 17 клавиш, Enter без разрешения) | отказ; Enter с разрешением — `Ok` |
| H-27 | `h27_control_bidi_zero_width_rejected` (U+202E, U+2066, U+200B, U+FEFF, U+0000, `\r\n`, ESC, `\n` без разрешения) | `FrameRejected(ForbiddenChar/LineBreakNotPermitted)` |
| H-28, SR-51 | `h28_no_state_shared_between_runs` | 3 запуска подряд — у каждого свежие глобалы и память |
| — | `valid_insert_is_parsed`, `guest_can_read_request_then_emit` | валидный `insert` разобран, текст совпадает |
| — | `process.rs`: дочерний `ks-sandbox` (stdin-кадры) | код 0 + один кадр; отказ → код 15 и **пустые** stdout/stderr; цикл → код 10; WASI-модуль → 20; плохой framing (u32::MAX, обрезанное тело, пусто, запрос 256 КиБ+1) → 21 |
| SR-46 | `ks-sandbox-proto`: 16 unit-тестов + детерминированные случайные кадры (SplitMix64, фиксированное зерно): 100 000 по умолчанию, 1 000 000 в CI | нет паник; каждый принятый кадр удовлетворяет всем лимитам |

## Результаты
CI run [42981885998](https://github.com/vsagirov97-pixel/KEYSWITCH-MASTER-PROJECT-INSTRUCTIONS/actions/runs/42981885998) на коммите `b538cc7` — **все 7 jobs success** (включая `Windows build`: `cargo clippy/test --workspace` на windows-latest, и `Supply chain`: `cargo deny`).

| ОС (job) | proto 16 тестов | 10⁶ случайных кадров | hostile 31 | process 4 | H-20 цикл, debug / release | H-23 рост памяти, release | H-22 рекурсия, release | валидный insert, release (compile / run) |
|----------|-----------------|----------------------|------------|-----------|-----------------------------|---------------------------|------------------------|------------------------------------------|
| ubuntu-latest ([job](https://github.com/vsagirov97-pixel/KEYSWITCH-MASTER-PROJECT-INSTRUCTIONS/actions/runs/42981885998/job/130393961610)) | ok, 0,73 с | ok, 7,20 с | ok, 2,98 с | ok, 2,02 с | 2,0004 / 2,0003 с | 1,29 мс | 0,74 мс | 0,85 мс / 0,13 мс |
| windows-latest ([job](https://github.com/vsagirov97-pixel/KEYSWITCH-MASTER-PROJECT-INSTRUCTIONS/actions/runs/42981885998/job/130393961499)) | ok, 1,15 с | ok, 11,49 с | ok, 4,41 с | ok, 2,05 с | 2,0016 / 2,0024 с | 1,07 мс | 0,67 мс | 1,73 мс / 0,23 мс |
| macos-15 ([job](https://github.com/vsagirov97-pixel/KEYSWITCH-MASTER-PROJECT-INSTRUCTIONS/actions/runs/42981885998/job/130393961345)) | ok, 0,85 с | ok, 9,81 с | ok, 4,30 с | ok, 2,12 с | 2,0120 / **2,1357 с** | 0,50 мс | 0,35 мс | 0,70 мс / 0,11 мс |
| Linux, этот хост (4 ядра) | ok | ok, 6,66 с | ok | ok | 2,0009 / 2,0004 с | 0,67 мс | 0,78 мс | 0,86 мс / 0,18 мс |

Числа — из шага `timing_numbers_for_report` (печатаются только длительности и коды результата). Превышение дедлайна: ≤ 2 мс на Linux/Windows, до 136 мс на macOS-раннере (в пределах допуска 0,5 с; причина — планирование потока на нагруженном раннере, не исследовалась). Инстанцирование ≤ 0,2 мс везде.

Локально (Linux, 2026-11-11): `scripts/check.sh` — 11 PASS, 1 SKIP (`sandbox macOS cross-check`: нет macOS SDK), 0 FAIL; `cargo clippy -p ks-sandbox-proto -p ks-sandbox --all-targets --target x86_64-pc-windows-msvc -D warnings` — чисто; `--target aarch64-apple-darwin` — чисто только с заглушкой `stdlib.h` для `helpers.c` (см. «Не доказано»).

## Доказано / не доказано
**Доказано (на уровне Wasmtime, тестами, в CI на трёх ОС — см. выше):** бесконечный цикл (в т.ч. в `start`) прерывается в пределах дедлайн + 0,5 с; бомбы памяти/таблиц отклоняются детерминированно; переполнение стека — trap, хост жив; модуль с любым WASI/`env`/неизвестным импортом не загружается; выключенные предложения Wasm не компилируются; кадр > 64 КиБ отклоняется до копирования; некорректные кадры, чужой `request_id`, запрещённые символы и клавиши отклоняются; между запусками нет состояния; бинарь не выводит ничего, кроме валидированного кадра.

**Не доказано / не сделано:**
- изоляция процесса ОС: LPAC + Job Object + mitigation policies (SR-54…SR-56, H-30…H-40) — **S-3b**; App Sandbox помощник на macOS (SEC-55) — **S-3c/S-3m**. Сейчас `ks-sandbox` работает с правами пользователя;
- `TerminateJobObject` / убийство процесса основным процессом по дедлайну (вторая линия SR-52) — S-3b;
- реальные гости QuickJS-ng / CPython WASI, stdlib из памяти, Wizer, SHA-256 модулей (SR-42, SR-57, H-01…H-12, H-21, H-24) — S-3b;
- Pulley vs Cranelift и ACG (SR-45) — S-3c;
- fuzz `cargo fuzz` хост-функций ≥ 10 мин (SR-44) — нет; есть только граничные unit-тесты указателей;
- кросс-clippy под `aarch64-apple-darwin` локально проходит только с заглушкой C-заголовка (build-скрипт wasmtime компилирует `helpers.c`, macOS SDK на Linux нет); полноценная проверка — нативно в job `sandbox (macos-15)`;
- `ks-engine` ещё не существует: повторная независимая валидация действий в ядре (SR-47) и политика окна (SR-48) — после S-3.

## План S-3b / S-3c
- **S-3b (Windows, `windows-latest`):** запуск `ks-sandbox.exe` из хоста в LPAC (opt-out ALL APPLICATION PACKAGES, 0 capabilities), `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` только с концами pipe, `CHILD_PROCESS_POLICY`, mitigation policies; Job Object (KILL_ON_JOB_CLOSE, ACTIVE_PROCESS=1, память ≈256 МБ, UI-лимиты); самопроверка токена/Job при старте (SR-56) и фича `test-hooks` для H-30…H-40; framing по pipe; `TerminateJobObject` по дедлайну; сборка QuickJS-ng и CPython WASI из закреплённых SHA, `include_bytes!` + SHA-256, read-only VFS в памяти; H-01…H-12, H-21, H-24.
- **S-3c:** матрица Cranelift vs Pulley × ACG (время, размер, совместимость с `PROCESS_DYNAMIC_CODE_POLICY`), решение SR-45; macOS: помощник с App Sandbox без entitlements, hardened runtime, `allow-jit` только при Cranelift, `setrlimit`, H-xx на `macos-15` (SEC-55, S-3m).
- Решение владельца по остаточному риску SEC-01/SEC-55 — только после данных S-3b/S-3c.
