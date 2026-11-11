# M-2a — macOS integration probe (AX, CGEvent, NSPasteboard, TIS, RegisterEventHotKey)

**Вердикт: базовые механизмы подтверждены на GitHub-hosted раннерах macOS 15 (arm64 и Intel) на AppKit-фикстуре.** Это первая часть M-2 (`MACOS_PLATFORM_MAP.md` §8); M-2 целиком **не закрыт** (см. «M-2b»).

Исполнитель: AGENT 19 · Ветка: `spike/mac-ax-probe` · Дата: 2026-11-11.
Где проверено: **только GitHub Actions** (`macos-15`, `macos-15-intel`). Ручных тестов владельца на реальном Mac **не было**. Хост облачной сессии — Linux: там только компиляция (`cargo clippy --target aarch64-apple-darwin`).

## Что сделано
- `crates/ks-probe-mac` — пробник (не продуктовый crate): обёртки AX (`ax.rs`), CGEvent Unicode-ввод (`events.rs`), NSPasteboard snapshot/restore (`pasteboard.rs`), TIS и `RegisterEventHotKey` (`carbon.rs`), отчёт JSON без текста (`report.rs`, `text.rs` — `ProbeText` без `Debug`/`Display`). Весь код за `cfg(target_os = "macos")`; на Linux/Windows crate пуст.
- `src/bin/ks-mac-fixture.rs` — AppKit-фикстура: `NSTextField`, `NSSecureTextField`, `NSTextView`, меню Edit (Undo). Только синтетические строки (`synthetic.rs`).
- `tests/probe.rs` — свой harness на главном потоке; одна строка `PROBE {json}` на сценарий; exit ≠ 0 только при FAIL; SKIP всегда с причиной.
- CI job `macos-probe` (матрица `macos-15`, `macos-15-intel`, `permissions: contents: read`): guard D-27 (grep запрещённых API: `CGEventTapCreate`, `CGRequestListenEventAccess`, `addGlobalMonitorForEvents`, `AXIsProcessTrustedWithOptions`, `TISEnable/DisableInputSource`), clippy, `cargo test -p ks-core`, пробник.
- D-27 соблюдён: event tap, Input Monitoring, запрос разрешений, глобальные мониторы — не используются. `CGPreflightListenEventAccess` вызывается только как диагностика (не запрос).

## Почему падал прошлый прогон (run 42830218515) и что исправлено
1. `macos-15` (arm64) — не стартовал: «The job was not acquired by Runner of type hosted» (нехватка arm64-раннеров GitHub; не дефект кода).
2. `macos-15-intel` — 5 FAIL:
   - **M-P01 ×3:** system-wide `AXFocusedUIElement` не указал на фикстуру за 10 с, хотя роль/подроль через элемент приложения были верны. Критерий был слишком строгим: путь фокуса — факт для M-2b, а не условие прохождения. Исправлено: PASS по роли/подроли/классу; путь и диагностика (`sys_focus_last`, `sys_focused_app`) записываются; `AXFrontmost` запрашивается повторно каждые 500 мс; ожидание 6 с.
   - **M-P08 ×2:** снимок буфера из `public.utf8-plain-text` + `public.html` отклонялся `UnsupportedType` — `-[NSPasteboard types]` отдаёт ещё и устаревшие псевдонимы (pre-UTI) [гип., подтверждено косвенно: после исключения трёх псевдонимов `NSStringPboardType`, `Apple HTML pasteboard type`, `NeXT Rich Text Format v1.0 pasteboard type` снимок и побайтное сравнение проходят на обоих раннерах]. `UnsupportedType` теперь несёт имя типа.
3. M-P15 приведён к требованию «переключать только если второй источник уже включён»: пробник больше **не включает/не выключает** источники ввода (FFI `TISEnable/DisableInputSource` удалён, guard в CI).

## Результаты (run 42981230174, commit 5dd3eb1)
- arm64: macOS 15.8.2 (24H232) — <https://github.com/vsagirov97-pixel/KEYSWITCH-MASTER-PROJECT-INSTRUCTIONS/actions/runs/42981230174/job/130391882343>
- Intel: macOS 15.7.9 (24G830) — <https://github.com/vsagirov97-pixel/KEYSWITCH-MASTER-PROJECT-INSTRUCTIONS/actions/runs/42981230174/job/130391882149>
- Итог на каждом: `pass=19 fail=0 skip=1 fact=3 total=23`; `cargo test -p ks-core` — 30 тестов (4 + 26), 0 провалов, на обоих.

| ID | Сценарий | macos-15 (arm64) | macos-15-intel | Числа / факты |
|---|---|---|---|---|
| M-P00 | Шаг 0: окружение | FACT | FACT | `AXIsProcessTrusted=true`, `CGPreflightPostEventAccess=true`, SIP disabled, Secure Event Input выкл. — на обоих |
| M-P01 | Роль/подроль фокуса: field / view / secure | PASS ×3 | PASS ×3 | `AXTextField` / `AXTextArea` / `AXTextField`+`AXSecureTextField`; secure → отказ до чтения значения; `IsSecureEventInputEnabled()=true` при фокусе в пароле; system-wide `AXFocusedUIElement` и `AXFocusedApplication` → `-25204` (`kAXErrorCannotComplete`) во всех трёх, фокус получен через элемент приложения |
| M-P02 | Чтение `AXValue` (+лимит) | PASS ×3 | PASS ×3 | чтение 64–141 мкс (arm), 139–216 мкс (Intel); 6000 символов → отказ `TooLong` |
| M-P03 | Установка `AXSelectedTextRange` + перечитывание, `AXSelectedText` | PASS ×2 | PASS ×2 | settable, AX 0, диапазон и выделенный текст совпали |
| M-P05 | Запись M1 `AXSelectedText` + проверка + Cmd+Z | PASS ×2 | PASS ×2 | замена подтверждена за 3–57 мс; **Undo = 1 шаг** в `NSTextField` и `NSTextView` |
| M-P06 | M3 `CGEventKeyboardSetUnicodeString`, 153 UTF-16 (RU/EN + суррогатная пара) | PASS ×4, FACT ×1 | PASS ×4, FACT ×1 | см. ниже |
| M-P08 | NSPasteboard: snapshot → запись с `TransientType`+`ConcealedType` → restore-only-if-unchanged | PASS ×3 | PASS ×3 | `changeCount` +1 на запись; побайтное равенство после восстановления; чужая запись после нашей → восстановление отменено, чужие данные сохранены; приватный тип → снимок отклонён |
| M-P15 | TIS: текущий источник, переключение | SKIP | SKIP | текущий `com.apple.keylayout.US` (arm) / `com.apple.keylayout.ABC` (Intel); включён 1 источник → **SKIP: «only one enabled selectable keyboard source; switching not tested (probe does not enable sources)»**; раскладка Russian установлена, но не включена |
| M-P18 | `RegisterEventHotKey` + дубликат | PASS ×2, FACT ×1 | PASS ×2, FACT ×1 | Ctrl+Opt+Cmd+K → 0; повтор в том же процессе → `-9878` (`eventHotKeyExistsErr`); **тот же аккорд в другом процессе → 0**; Opt-only, Opt+Shift, Ctrl-only, `1` без модификаторов → регистрация 0; синтетический аккорд → обработчик сработал |

**M3, скорость (UTF-16 единиц/с, до подтверждения через AXValue):**

| Вариант | arm64 | Intel |
|---|---|---|
| field, PostToPid, 1 ед./событие (150 key-down) | 340 | 235 |
| field, PostToPid, 20 ед./событие (8 key-down) | 1719 | 1463 |
| field, HID, 20 ед./событие | 1312 | 2044 |
| view, PostToPid, 20 ед./событие | 1026 | 866 |
| field, PostToPid, 40 ед./событие (FACT) | 1623 | 1600 |

Вариант 40 ед./событие тоже доставил текст полностью — предел «20 символов» [сообщ.] на этих фикстурах **не воспроизвёлся**; в продукт всё равно закладываем ≤ 20 (Apple предел не документирует).

Предыдущий прогон (run 42830218515, Intel, код до исправления; включал Russian, затем выключал): `TISSelectInputSource` ABC → Russian → ABC — статусы 0, переключение и возврат подтверждены — <https://github.com/vsagirov97-pixel/KEYSWITCH-MASTER-PROJECT-INSTRUCTIONS/actions/runs/42830218515/job/129877979343>. Это поведение из пробника удалено; факт сохраняется как [сообщ.: одиночный прогон CI].

## Доказано (только CI, AppKit-фикстура, macOS 15)
- На hosted-раннере пробник, запущенный шагом job, получает `AXIsProcessTrusted=true` и право `CGEventPost`; сами мы TCC.db не меняли (как именно это выдано в образе раннера — не проверено). Гипотеза §7.2 карты подтверждена для macOS 15.
- Распознавание пароля по `AXSecureTextField` без чтения значения; Secure Event Input включается при фокусе в `NSSecureTextField`.
- AX-чтение, выделение с перечитыванием, M1-запись с проверкой и Undo в 1 шаг — для `NSTextField`/`NSTextView`.
- M3: Unicode-ввод с суррогатной парой, PostToPid и HID; пакет 20 — ~0,9–2 тыс. ед./с.
- NSPasteboard: правило «восстанавливать только при неизменном `changeCount`» работает; приватные типы отклоняются; метки nspasteboard.org записываются.
- `RegisterEventHotKey` работает без Input Monitoring и срабатывает; конфликты с **другими** процессами API не обнаруживает.
- Input Monitoring ни для одного шага не потребовался (критерий 4 §8.4).

## Не доказано (почему)
- **System-wide фокус:** в M-P01 стабильно `-25204`, в M-P05/M-P06 на тех же раннерах иногда разрешается — причина неизвестна [гип.: AX-сервер ещё не связал системный фокус с только что запущенным процессом без активного окна-ключа]. Продукт не должен полагаться только на system-wide; HID-доставку — только после подтверждения `AXFrontmost`.
- Срабатывание Opt-only и хоткеев без модификаторов (проверена только регистрация); поведение хоткеев при Secure Event Input.
- Переключение TIS текущим кодом (SKIP по правилу); источник ввода на реальном Mac с двумя раскладками — ручной тест владельца.
- Web/WKWebView/Electron, Chromium, Office, браузеры, contenteditable — фикстур нет.
- `macos-26`, реальные Mac, подписанная сборка, онбординг TCC, privacy pasteboard (macOS 26).
- Сценарии M-P04, M-P07, M-P09–M-P14, M-P16, M-P17 в M-2a не входили.

## M-2b (предлагаемый объём)
1. Фикстуры web (`index.html` из S-2), WKWebView, Electron; M-P14 (`AXManualAccessibility`).
2. M-P04 (фрагмент до каретки), M-P07 (ленивый провайдер, сигнал M2), M-P09 (слушатель буфера), M-P10/M-P11 (`TargetChanged`), M-P12 (Secure Event Input включён фикстурой + хоткей), M-P13 (вложения), M-P16 (зависшая фикстура, дедлайн), M-P17 (NSPanel без активации + временные `1/2/Esc`).
3. Диагностика system-wide фокуса (повтор после активации, `NSWorkspace.frontmostApplication`) и решение для П-1: какой путь фокуса использует `ks-platform-mac`.
4. Срабатывание Opt-only/без модификаторов (синтетический аккорд), M-P18 под Secure Event Input.
5. Матрица `macos-26` (критерий 1 §8.4); arm64-раннеры могут не выделяться — job оставить информационным.
6. Чек-лист ручной приёмки владельца на реальном Mac (две раскладки EN/RU, TCC-онбординг, реальные приложения).
