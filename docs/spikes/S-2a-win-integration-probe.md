# Спайк S-2a — Windows integration probe (Win32-фикстура)

Ветка: `spike/win-integration-probe` · Исполнитель: AGENT 05 (Windows Integration) · Руководитель: AGENT 20 · Дата: 2026-11-10, подтверждение 2026-11-11.
Статус: **код и CI готовы, результаты получены на `windows-latest`; спайк не закрыт** (S-2 целиком требует S-2b, см. ниже). Ничего не слито в `main`.

## 1. Что сделано
- Crate `crates/ks-winprobe` (не продуктовый код): Win32-фикстура в процессе теста — окно с `EDIT` (однострочный, многострочный `ES_MULTILINE|ES_WANTRETURN`, `ES_PASSWORD`) и `RICHEDIT50W`; обёртки UIA (MTA, `CUIAutomation8`, таймауты 500/1000 мс), `EM_REPLACESEL`, `SendInput(KEYEVENTF_UNICODE)`, буфер обмена, раскладки, `RegisterHotKey`.
- Интеграционные тесты `crates/ks-winprobe/tests/{a_uia,b_w1,c_sendinput,d_clipboard,e_layout,f_hotkey}.rs`. Включаются только при `KS_WINPROBE=1`; без него каждый тест печатает SKIP (поэтому `cargo test --workspace` в job `windows` их не исполняет).
- Каждый сценарий печатает строку `KS-PROBE {json}` только с метаданными (длины, счётчики, id, тайминги, флаги). Текст полей/буфера не печатается; все строки синтетические.
- `unsafe` — только в `ks-winprobe/src`, каждый блок с `// SAFETY:` (lint `undocumented_unsafe_blocks = deny`). Хуков клавиатуры, инъекций в чужие процессы, сети нет. `AttachThreadInput` используется **только фикстурой** для получения foreground на CI и не должен попасть в продукт.
- CI: job `windows-probe` (`windows-latest`, `permissions: contents: read`, actions по SHA как в остальных job): `cargo test -p ks-winprobe --no-fail-fast -- --nocapture --test-threads=1`; строки KS-PROBE публикуются в Step Summary и как notice-аннотации. `--no-fail-fast` добавлен к заданной команде, чтобы провал одного файла не скрывал остальные.

## 2. Результаты на windows-latest
Прогон: run 42829765734, коммит `0f47a57`, job https://github.com/vsagirov97-pixel/keyswitch-master-project-instructions/actions/runs/42829765734/job/129876445583 — success.
Повторный прогон после слияния `origin/main` (подтверждение воспроизводимости): run 42980865753, коммит `9fdf510`, job https://github.com/vsagirov97-pixel/keyswitch-master-project-instructions/actions/runs/42980865753/job/130390708655 — success; все статусы совпали (12 PASS, 2 OBSERVED, 0 SKIP, 0 FAIL). Отличия только в таймингах: c2 — 163 мс / **≈1225 символов/с** (send_call 3 мс), c1 суррогатная пара — 82 мс, a1 focus 3–26 мс. Числа таймингов — единичные замеры на общем раннере, не бенчмарк.

Окружение раннера: сессия 2 (интерактивная), `OpenInputDesktop` успешен, фикстура получила foreground (`direct`; в e1 — через `AttachThreadInput`).

| Тест | Результат | Факты |
|------|-----------|-------|
| a1 UIA focused element | PASS | `GetFocusedElement` совпал с HWND за 4–35 мс. ControlType: однострочный EDIT и пароль = Edit (50004); **многострочный EDIT и RICHEDIT50W = Document (50030)**. IsPassword: true только у `ES_PASSWORD`. RuntimeId длины 2, стабилен при повторном чтении, уникален. `GetText` вызван 0 раз. |
| a2 пароль не читается | PASS | `ES_PASSWORD` → IsPassword=true; **TextPattern у пароля отсутствует**; защищённое чтение вернуло RefusedPassword, счётчик `GetText` = 0 (контроль на обычном поле = 1). |
| a3 DocumentRange | PASS | Текст совпал: single/multi — побайтно; RichEdit — после нормализации переводов строк (`\r\n` → `\r`, 18 единиц). `GetText(5001)` на 6000 единиц вернул ровно 5001. |
| a4 выделение | PASS | `GetSelection` отражает `EM_SETSEL`; `TextRange.Select()` → `EM_GETSEL` = 0..6, `Compare` = true, текст совпал. Выделение всего DocumentRange в RichEdit = 0..18, а `EM_SETSEL(0,-1)` = 0..19 — **DocumentRange не включает финальную метку абзаца RichEdit**. Фикстура была foreground. |
| b1 W1 EM_REPLACESEL + Undo | PASS | single и multi: замена верна, каретка 6..6, `EM_CANUNDO`=1, один `EM_UNDO` вернул исходник; второй `EM_UNDO` — redo (одноуровневый Undo Edit). С `fCanUndo=FALSE` после очистки буфера Undo недоступен. |
| b2 RichEdit RTF (XR-03) | OBSERVED | **`EM_REPLACESEL("{\rtf1 \b x}")` в RICHEDIT50W интерпретирует RTF** (в поле «x»); Undo откатывает. `WM_SETTEXT` с той же строкой — вставлен буквально. Обычный текст через `EM_REPLACESEL` + Undo в RichEdit — корректно. |
| c1 SendInput Unicode | PASS | «ASCII + кириллица» (12 единиц, 24 события, 1 пакет) — дошло за 11 мс; суррогатная пара (U+1F600) — дошла целой за 128 мс. |
| c2 200 символов | PASS | 7 пакетов по ≤ 32 единицы, 400 событий, вызовы `SendInput` 3 мс, весь текст в поле за 173 мс → **≈1150 символов/с**; проверка foreground перед каждым пакетом (7 раз). Один `EM_UNDO` удалил весь набранный текст (Edit). |
| c3 отказ `\n`/`\t`/`\r` | PASS | `\n`, `\t`, `\r\n`, `\r` отвергнуты до отправки (проверок пакетов 0); в корректном тексте нет VK_RETURN/CR/LF; сфокусированное многострочное поле не изменилось. |
| d1 буфер, форматы D-13 | PASS | После нашей записи 7 форматов (с синтезированными), присутствуют `CF_UNICODETEXT` и все три: `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory`, `CanUploadToCloudClipboard`. Перечисление форматов не меняет номер последовательности; наша запись меняет (+8 за одну операцию Empty + 4 × SetClipboardData; разбивка по вызовам не измерялась). |
| d2 restore только без изменений | PASS | Без вмешательства — restore выполнен, снимок на месте. Второй писатель (окно другого потока этого процесса) и **другой процесс** (`powershell Set-Clipboard`) → restore пропущен, их данные сохранены. |
| e1 раскладка | PASS | Загружена была только 0409; 0409 и 0419 загружены `LoadKeyboardLayoutW`; `WM_INPUTLANGCHANGEREQUEST` в сфокусированный EDIT → раскладка потока фикстуры 0419 за 10 мс и обратно 0409 за 10 мс; добавленная 0419 выгружена. |
| f1 RegisterHotKey | PASS | Ctrl+Alt+Shift+F13 на отдельном потоке с циклом сообщений; повторная регистрация → ошибка 1409 (`ERROR_HOTKEY_ALREADY_REGISTERED`); после снятия — регистрируется снова. |
| f2 доставка WM_HOTKEY | OBSERVED | Внедрённый аккорд (8 событий) → `WM_HOTKEY` с id 3 получен. |

Предыдущие итерации: run 42828479342 — a1/a4 FAIL из-за ожиданий (Edit вместо Document для многострочного EDIT; финальная метка RichEdit) — ожидания исправлены по факту; run 42828963511 — d1/d2 SKIP, т.к. «номер последовательности = 0» ошибочно считался отсутствием доступа (в свежей сессии он равен 0 до первой записи) — исправлено.

## 3. Выводы для архитектуры
1. Классификация поля не может опираться на ControlType=Edit: многострочный Win32 EDIT и RichEdit видны как Document. Отпечаток должен включать класс окна (`Edit`, `RICHEDIT50W`).
2. **XR-03 подтверждён: W1 (`EM_REPLACESEL`) для RichEdit небезопасен** — строка, начинающаяся с `{\rtf`, интерпретируется как RTF. Предложение: W1 только для класса `Edit` (как в WINDOWS_PLATFORM_MAP); для RichEdit — W3 или отказ, либо (после отдельного решения) `EM_SETTEXTEX` с `ST_UNICODE`/plain-text режимом — не проверено.
3. Для пароля TextPattern отсутствует вовсе — дополнительный признак, но решение принимается по IsPassword/`ES_PASSWORD` (fail closed).
4. Сравнение «всё ли выделено» в RichEdit должно учитывать финальную метку абзаца (±1).
5. W3: ≈1150–1225 симв./с на CI (два прогона) — порог профиля 200 символов (≈0,2 с) реалистичен; Undo в Edit откатывает весь набор одним шагом (для других приложений не проверено).

## 4. Автоматически проверено (CI windows-latest, runs 42829765734 и 42980865753)
Сценарии a1–a4, b1, c1–c3, d1–d2, e1, f1 — PASS; b2, f2 — зафиксированы факты (OBSERVED). Только на собственной фикстуре в процессе теста, Windows Server-образ GitHub (`windows-latest`), учётная запись администратора раннера.
Локально (Linux): `cargo clippy -p ks-winprobe --target x86_64-pc-windows-msvc --all-targets -- -D warnings` — чисто; `scripts/check.sh` — все PASS.

## 5. Не проверено
- Реальные приложения (Notepad Win11, Chromium/WebView2/Electron, Firefox, Qt/Telegram, Word, Excel, WPF/UWP) — S-2b и ручная приёмка владельцем (GATE 7).
- Межпроцессный доступ: фикстура живёт **в процессе теста** (UIA-провайдер-прокси тот же, но UIPI, межпроцессные таймауты и зависшие цели не проверены). Цель с повышенными правами (P-12), зависшее окно (P-16) — не проверены.
- Реальные нажатия клавиш: доставка `WM_HOTKEY` проверена только для внедрённого ввода.
- Фактическое исключение из истории Win+V / облачного буфера — проверено только присутствие форматов; служба истории буфера на раннере не проверялась.
- Отложенная отрисовка буфера (W2, `WM_RENDERFORMAT`), snapshot/restore форматов кроме текста, «зависший владелец буфера» — S-2b.
- Смена фокуса между пакетами W3 (второй процесс забирает foreground) — проверяется только наличие проверки, сам перехват не моделировался.
- Windows 10 и клиентские редакции Windows 11; per-app раскладки; IME.
- Получение foreground на CI шло через `AttachThreadInput` (e1) — для продукта поведение «без кражи фокуса» не проверено.

## 6. Не поддерживается (решения)
- Низкоуровневые хуки клавиатуры, инъекции в чужие процессы, повышение прав — запрещены (CLAUDE.md п. 8); не используются и не тестируются.
- Ввод `\n`/`\t`/`\r` через W3 — отказ по дизайну.
- W1 для RichEdit — не поддерживать до отдельного решения (п. 3.2).

## 7. Предлагаемый объём S-2b
Межпроцессные фикстуры (отдельный exe с той же Win32-фикстурой; WebView2/Edge `input`/`textarea`/`contenteditable`/`type=password`; WPF TextBox/PasswordBox), P-07/P-08 (delayed rendering, snapshot белого списка, монитор буфера), P-10/P-11 (смена фокуса/текста между чтением и записью → `TargetChanged`), P-12 (elevated-цель), P-16 (зависшее окно), W3 при 1000/5000 символов и потеря foreground между пакетами, окно выбора `WS_EX_NOACTIVATE`, `EM_SETTEXTEX` для RichEdit (XR-03), P-18 (уровень целостности).
