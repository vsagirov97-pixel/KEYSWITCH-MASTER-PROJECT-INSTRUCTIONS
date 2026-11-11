# Security Review S-2a — AGENT 15 (Windows Platform Security) → CISO

Дата: 2026-11-11 · Объект: `origin/spike/win-integration-probe` (head 45c982f; CI run 42980865753 на 9fdf510, разница — только отчёт спайка) · Записано Project Director по отчёту AGENT 15 (ревьюер файлы не менял).

**Вердикт: PASS WITH CONDITIONS** — S-2a принят как результат спайка. Условия: WS-01…WS-04 — в требования продуктового `ks-platform-win` и в S-2b; WS-08 — до первого продуктового Windows-кода.

Проверено: весь diff (21 файл), job `windows-probe`, Cargo.toml; run 42980865753 = success, 14 строк KS-PROBE (12 PASS, 2 OBSERVED) из лога job; нет хуков, инъекций, повышения прав, `keybd_event`; `AttachThreadInput` только в `fixture.rs:469-474`; продуктовые crates не зависят от ks-winprobe; у всех `unsafe` — `// SAFETY:`; CI `contents: read`, SHA-pinning; в KS-PROBE только метаданные.

| ID | Сер. | Место | Сценарий | Рекомендация / проверка |
|----|------|-------|----------|--------------------------|
| WS-01 | Medium | `tests/d_clipboard.rs:27`, `src/clipboard.rs:161` | `our_seq` читается после `CloseClipboard` — чужая запись в промежутке будет принята за свою, restore затрёт чужие данные | seq читать до `CloseClipboard`; тест с вклинивающимся писателем (S-2b) |
| WS-02 | Medium* | `src/clipboard.rs:166` | restore только `CF_UNICODETEXT`, `private=false` — пароль менеджера паролей вернётся без меток D-13 → история Win+V/облако | восстанавливать по белому списку с исходными метками (C-1′, XR-02, XR-04); S-2b P-07/P-08 |
| WS-03 | Medium* | `src/uia.rs:197` | `range_text` публична и обходит проверку пароля; проверяется только IsPassword, не `ES_PASSWORD` | одна точка чтения с проверкой IsPassword + `GWL_STYLE` `ES_PASSWORD`; unit + межпроцессный тест |
| WS-04 | Medium* | `src/fixture.rs:193` | `SendMessageW` без таймаута — зависшая цель блокирует поток | только `SendMessageTimeoutW(SMTO_ABORTIFHUNG)`; тест P-16 |
| WS-05 | Low | `src/input.rs:108` | перед пакетом проверяется только foreground; удерживаемый Ctrl превращает символы в сочетания | полный отпечаток + модификаторы + generation gate (C-6′); тест в S-2b |
| WS-06 | Low | `src/input.rs:60` | `is_control` пропускает U+2028/2029 и bidi (U+202A–202E, U+2066–2069) | отвергать категории Cf, Zl, Zp; unit-тест |
| WS-07 | Low | `src/input.rs:137` `send_vk_chord` | универсальная отправка VK (Enter, Ctrl+V) | не переносить в продукт; CI-запрет вне ks-winprobe |
| WS-08 | Low | CI / workspace | ks-winprobe без workspace-lints, запрет API не охраняется | CI-запрет `SetWindowsHookEx|AttachThreadInput|CreateRemoteThread|WriteProcessMemory` вне ks-winprobe; запрет зависимости продукта на ks-winprobe; `persist-credentials: false` |
| WS-09 | Info | `src/input.rs:102` | неточный SAFETY-комментарий | уточнить |
\* Medium при переносе кода в продукт.

**OBSERVED:**
- **XR-03 подтверждён (High для W1 в RichEdit).** D-30 принят с условиями: класс через `GetClassNameW` и `RealGetWindowClassW`, точное совпадение с `Edit` без учёта регистра; не писать в `ES_PASSWORD`/`ES_READONLY`; перед W1 повторная сверка отпечатка; KeySwitch никогда не кладёт `CF_RTF`/HTML в буфер.
- **f2: внедрённый ввод вызывает WM_HOTKEY.** Любой процесс на том же рабочем столе может запустить операцию KeySwitch; без хуков (D-14) отличить нельзя. Остаточный риск; нажатие не считается подтверждением пользователя: шлюз, ограничение частоты, все проверки цели; в S-2b проверить WM_HOTKEY от процесса Low-IL.

**Требования к S-2b:** межпроцессная фикстура (exe, WebView2, WPF PasswordBox); P-07/P-08 с белым списком и сохранением меток; seq под `OpenClipboard`; XR-01, XR-02; P-10/P-11 и полный отпечаток перед каждым пакетом; модификаторы; P-12, P-16, P-18; отказ для защищённого рабочего стола и для самого себя; WM_HOTKEY от Low-IL; `EM_SETTEXTEX` — только отдельным ADR; WER и краш-тест; ссылка на job для каждого факта.

**Не проверено (вне S-2a):** цели с повышенными правами, защищённый рабочий стол, KeySwitch как цель; WER/краш-тест; operation gate; межпроцессный UIPI; реальное исключение из Win+V; XR-01/05/06/10; Windows 10 и клиентские редакции.
