# Security Review M-2a — AGENT 21 (macOS Platform Security) → CISO

Дата: 2026-11-11 · Объект: `origin/spike/mac-ax-probe` (6d4a14b; CI run 42981230174 на 5dd3eb1 — зелёный на macos-15 arm64 и macos-15-intel, проверено PD) · Записано PD по отчёту AGENT 21 (файлы не правились).

**Вердикт: PASS WITH CONDITIONS.** M-2a принят как результат спайка; код пробника не является шаблоном продукта без условий ниже. Критических находок нет.

**Проверено:** весь diff с main; логи run 42981230174 (`gh run view --log`) — 23 строки PROBE на раннер, текста полей нет; D-27 соблюдён (нет tap/монитора/запроса прав, guard в CI); `TISEnable/DisableInputSource` удалены, `TISSelectInputSource` — только между включёнными источниками; CI `contents: read`, SHA-pinning (соответствие SHA тегам не сверялось); у `unsafe` — `SAFETY`.

**M-P01 — исправление легитимно, тест не ослаблен.** Свойство (роль/подроль/класс, отказ на secure до чтения значения) проверяется; отрицательный факт (system-wide путь вернул -25204 во всех 6 случаях при `frontmost=true`) записан. **Вывод для продукта — system-wide фокусу не доверять.** Порядок: (1) PID и время запуска через `NSWorkspace.frontmostApplication`; (2) `AXUIElementCreateApplication(pid)` → `AXFocusedUIElement`; (3) проверить PID элемента = PID frontmost, `AXFocused=true`, `AXWindow` = `AXFocusedWindow`; (4) system-wide ответил и указывает на другое приложение → отказ; (5) повторять перед каждой записью/пакетом (SEC-56).

| ID | Сер. | Место | Суть | Рекомендация / проверка |
|----|------|-------|------|--------------------------|
| F-1 | Medium | `tests/probe.rs:353-364` | без system-wide ответа берётся фокус приложения по PID без проверки frontmost → запись в фоновое окно | порядок выше; M-2b: фикстура в фоне → отказ |
| F-2 | Medium | `src/pasteboard.rs:116-127` | снимок молча пропускает устаревшие псевдонимы и `data()=None` → пустой снимок Ok → restore сотрёт буфер | пропускать псевдоним только при наличии UTI; пустой снимок при непустом `types()` → отказ M2; сценарий «только `NSStringPboardType`» |
| F-3 | Medium | `src/pasteboard.rs:131-136` | запись не атомарна, маркеры после текста, нет `prepareForNewContents(.currentHostOnly)` (SEC-52) | один `NSPasteboardItem` со всеми типами + `writeObjects` с `currentHostOnly`; CI — маркеры, Handoff — вручную |
| F-4 | Medium | M-P00; `docs/spikes/M-2a-mac-ax-probe.md:207-209` | на раннере Input Monitoring уже выдан, SIP выключен → «Input Monitoring не нужен» не доказано | перевести в «не доказано»; доказать вручную на Mac (подписанная сборка, SIP вкл.) |
| F-5 | Medium | `src/ax.rs:198-206` | отказ на пароле только по подроли; `IsSecureEventInputEnabled()` лишь записывается (SEC-51) | отказ по любому признаку в `ks-platform-mac`; M-P12 |
| F-6 | Medium | M-P18 | тот же аккорд регистрируется в другом процессе (0), Opt-only и без модификатора — тоже: конфликты не обнаружить | честный текст в UI + проверочное нажатие; запретить Opt-only и без модификатора глобально (только временно в NSPanel) |
| F-7 | Low | `tests/probe.rs:647-649`, M-P18 | пробник очищает общий буфер без снимка и шлёт HID-сочетания во frontmost → **на Mac владельца уничтожит его буфер** | запуск только при `CI=true` или явном флаге; ручной набор — безопасный режим |
| F-8 | Low | `tests/probe.rs:372`, `:546`, CI guard | при ошибке `AXNumberOfCharacters` лимит не применяется; число шагов Undo не в критерии; guard без `CGRequestPostEventAccess`, `IOHIDManager` | при ошибке — отказ; FAIL при Undo ≠ 1 (или FACT); дополнить guard |
| F-9 | Info | `crates/ks-probe-mac/Cargo.toml:66` | снят `unsafe_code=forbid` | допустимо для спайка; в продукте `unsafe` только в `ks-platform-mac` (SEC-L2) |

**Не проверено:** подпись, нотаризация, hardened runtime, entitlements, App Sandbox; `.ips`/unified logging (SEC-57); Handoff/Universal Clipboard; приватность буфера в macOS 26; `cargo deny` на macOS-графе в этом прогоне; соответствие SHA тегам.

**Требования к M-2b:** закрыть F-1, F-2, F-3, F-5 в коде — будущей основе `ks-platform-mac`; сценарии: фикстура в фоне, буфер только с устаревшим типом, Secure Event Input при обычном поле (M-P12); переформулировать F-4; защита F-7; порядок поиска фокуса — в карту и ADR; для GATE 8 — чек-лист ручной приёмки владельцем на реальном Mac (TCC без Input Monitoring, две раскладки) и матрица `macos-26`.
