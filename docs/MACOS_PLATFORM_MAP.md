# MACOS_PLATFORM_MAP — карта платформы macOS для KeySwitch

Автор: **AGENT 19 macOS Platform Lead** · Задание **M-1** · Редакция 1 · 2026-11-10 · ветка `docs/governance-v3`
Основание: `docs/master-instructions-v7.md` §2, §9 (macOS — обязательная платформа), `docs/SYSTEM_ARCHITECTURE.md` §8, `docs/research/arch-architecture-v2.md` §3.2 и §8.4, `docs/SECURITY_REVIEW.md` §4, §6, решения D-05, D-12, D-13, D-14, D-25, D-26.
Связанный документ: `docs/COMPATIBILITY_MATRIX_MACOS.md`.
Статус: **исследование, кода нет.** Ни одна строка не проверена на реальном Mac. Ревью: System Architect (трейты, §4), CISO (§1.10–1.14, §3), CLO (§1.14, §5).

## 0. Пометки достоверности и что проверено

| Пометка | Значение |
|---|---|
| **[док]** | Документированный Apple/вендором API. URL указан. **developer.apple.com из этой сессии не читается** (WebFetch вернул только заголовок страницы), поэтому все [док]-ссылки Apple — **UNVERIFIED по тексту**: существование API подтверждено компиляцией (см. ниже) или вторичными источниками, но формулировки документации не перечитаны |
| **[сообщ.]** | Сообщения разработчиков (форумы Apple, issue-трекеры, блоги); не документировано Apple |
| **[гип.]** | Гипотеза M-1; проверяется в M-2 (CI) или ручным тестом владельца |
| **[пров.-комп.]** | Проверено компиляцией в этой сессии (см. ниже) |

**Проверено мной в этой сессии (как, где):**
1. **Компиляция вызовов API** — `cargo +1.97.0 check --target aarch64-apple-darwin` на пробном crate **вне репозитория** (scratchpad), без линковки: `AXIsProcessTrusted`, `AXUIElement::new_system_wide`, `set_messaging_timeout`, `copy_attribute_value`, `is_attribute_settable`, `NSPasteboard::generalPasteboard`/`changeCount`, `NSEvent::modifierFlags_class`, `CGEventSource::new`, `CGEvent::new_keyboard_event`, `keyboard_set_unicode_string`, `post_to_pid`, `CGPreflightPostEventAccess`, `CGPreflightListenEventAccess`, `SMAppService::mainAppService`. Результат: `Finished` без ошибок. **Это не исполнение**: поведение не проверено.
2. **Версии/даты/лицензии crate** — API crates.io (`curl` с User-Agent), §6.
3. **Метки раннеров GitHub** — README `actions/runner-images` (WebFetch), §7.1.
4. **Выдача Accessibility на hosted-раннерах** — исходник образа `images/macos/scripts/build/configure-tccdb-macos.sh` в `actions/runner-images` (ветка `main`, прочитан `curl` 2026-11-10), §7.2.
5. `deny.toml`, `Cargo.lock` проекта — grep, §6.

**Не проверено (почему):** исполнение любого macOS-кода (хост — Linux, macOS-раннеров в CI проекта ещё нет); поведение приложений; тексты страниц developer.apple.com (недоступны); документация Tauri v2 (`v2.tauri.app` не резолвится в этой сессии — данные по updater из вторичных источников).

---

## 1. Карта по функциям

Общий принцип переносится без изменений: **до шага Write изменений нет; при сомнении — отказ**; текст пользователя только `UserText`; никаких event tap / keylogger-механизмов (аналог D-14); никаких инъекций, Apple Events и приватных API без ADR.

### 1.1 Фокусный элемент и чтение текста (`FieldAccess::inspect/read`)
- **API [док]:** `AXUIElementCreateSystemWide` → `kAXFocusedUIElementAttribute`; надёжнее — `AXUIElementCreateApplication(pid фронтального приложения)` → `kAXFocusedUIElementAttribute` (через `NSWorkspace.frontmostApplication`). Атрибуты: `kAXRoleAttribute`, `kAXSubroleAttribute`, `kAXValueAttribute`, `kAXNumberOfCharactersAttribute`, `kAXSelectedTextAttribute`, `kAXSelectedTextRangeAttribute`, `kAXSelectedTextRangesAttribute` (мульти-выделение), параметризованные `kAXStringForRangeParameterizedAttribute`, `kAXRangeForLineParameterizedAttribute`, `kAXInsertionPointLineNumberAttribute`. Таймаут — `AXUIElementSetMessagingTimeout` (аналог UIA ConnectionTimeout; предлагаю 0,5 с).
  `https://developer.apple.com/documentation/applicationservices/axuielement_h` — UNVERIFIED (недоступно).
- **Разрешение:** Accessibility (`kTCCServiceAccessibility`).
- **Достижимость:** документировано для Cocoa (`NSTextField`, `NSTextView`), WebKit; для Chromium/Electron — после включения AX-дерева (§1.1.1).
- **Чтение области (D-03):** выделение → `kAXSelectedTextAttribute`; «всё поле» — `kAXNumberOfCharacters` ≤ 5 001, затем `kAXValue`/`StringForRange`; «фрагмент до каретки» — `kAXInsertionPointLineNumber` → `kAXRangeForLine` → `StringForRange(начало строки…каретка)`. Встроенные объекты: U+FFFC в строке или `AXAttachment` в атрибутированной строке → отказ `EmbeddedObjects`.
- **Подводные камни:**
  - **Chromium/Electron** (Chrome, VS Code, Discord, Slack): AX-дерево строится только при обнаружении вспомогательной технологии. Electron документирует атрибут `AXManualAccessibility` для включения сторонними утилитами [док: `https://www.electronjs.org/docs/latest/tutorial/accessibility`]; Chrome реагирует на `AXEnhancedUserInterface` [сообщ.], который у части приложений ломает анимации/позиционирование окон [сообщ., разработчики оконных менеджеров]. Первое включение — сотни мс до готовности дерева [сообщ., cua PR #1756: ожидание ~500 мс]. **Предложение:** `AXManualAccessibility` — только для Electron-приложений из профиля; `AXEnhancedUserInterface` — **не использовать по умолчанию** (меняет состояние чужого приложения; решение CISO/Architect); первая операция в «холодном» приложении может закончиться `ScopeUndetermined` + повтор (аналог `chromium_retry`).
  - `kAXValue` у `contenteditable` / `AXWebArea` может отдавать не то, что видит пользователь (плейсхолдеры, скрытые узлы) [гип.] → в браузерах до M-2/ручной приёмки только выделение/фрагмент (как D-19(a), V7 §5).
  - Атрибуты бывают «устаревшими» после смены фокуса; каждый вызов — с таймаутом, `kAXErrorCannotComplete`/`kAXErrorInvalidUIElement` → `ElementGone`/отказ.
- **Отказ:** нет Accessibility (`kAXErrorAPIDisabled`/`AXIsProcessTrusted()==false`) → `PermissionMissing` (новый код, §4); роль не из белого списка (`AXTextField`, `AXTextArea`, `AXComboBox`, `AXSearchField`, `AXWebArea`/`AXGroup` с редактируемым текстом только по профилю); `AXSecureTextField` → `Password`; длина > 5 000 → `TooLong`; фокус в собственном окне KeySwitch (pid == свой) → `SelfTarget`; экран заблокирован/fast user switching → отказ.

### 1.2 Выделение (`select_verified`)
- **API [док]:** `AXUIElementSetAttributeValue(kAXSelectedTextRangeAttribute, AXValue(CFRange))` → перечитать `kAXSelectedTextAttribute` → сравнить дайджест (I-4). Проверка `AXUIElementIsAttributeSettable` до записи.
- **Cmd+A** — только как резерв по D-04 (элемент текстовый, не пароль, фокус внутри), синтез через `CGEvent` (§1.3).
- **Камни:** Chromium принимает установку `AXSelectedTextRange` для `<input>/<textarea>`, для `contenteditable` — [гип.]; диапазоны в UTF-16 code units (как `NSString`) — конвертировать аккуратно (суррогатные пары, эмодзи) — источник порчи «вне области» (S1).

### 1.3 Замена текста (`native_replace_selection` / `InputSynth`)
Три способа — аналоги W1/W3/W2, порядок тот же: **M1 → M3 (≤ порога, без `\n`/`\t`) → M2 (только по профилю) → отказ**.
| Способ | API | Плюсы | Камни |
|---|---|---|---|
| **M1 AX-запись выделения** (аналог W1) | `AXUIElementSetAttributeValue(kAXSelectedTextAttribute, новый текст)` после проверки settable [док] | нет синтеза ввода, не трогает буфер | **Undo не гарантирован** (D-12): попадает ли запись в `NSUndoManager` — [гип.] для `NSTextView`, неизвестно для Chromium/Office; React-контролируемые поля могут не получить событие `input` [гип.]. Допускается только для классов, где M-2 покажет Cmd+Z → исходный текст |
| **M3 синтез Unicode** (аналог W3) | `CGEventCreateKeyboardEvent` + `CGEventKeyboardSetUnicodeString` + `CGEventPostToPid(pid цели)` или `CGEventPost(kCGHIDEventTap)` [док] | работает почти везде | практический предел ~20 UTF-16 на событие, часть приложений берёт 1 символ на событие [сообщ.: форум Qt, typechar] → по одному символу/суррогатной паре на событие; модификаторы хоткея должны быть отпущены (`NSEvent.modifierFlags`, без разрешений); `\n` запрещён (Enter = отправка); проверка фокуса между пакетами; при включённом Secure Event Input события в поле пароля не идут — но мы туда и не пишем |
| **M2 вставка** (аналог W2) | `NSPasteboard` + `Cmd+V` через `CGEvent` | быстро, многострочно | см. §1.4 — **на macOS нет надёжного сигнала потребления с PID**, поэтому по Q-1/D-26 буфер почти никогда не восстанавливается автоматически |
- **Запрещено:** установка `kAXValueAttribute` всего поля как способ замены (аналог запрета `ValuePattern.SetValue`, D-12: теряется Undo, ломает контролируемые поля). Ввод через Apple Events/AppleScript (`System Events keystroke`) — требует Automation, запрещено без ADR.
- **Post-verify:** перечитать область, сравнить дайджест ожидаемого результата; `Unknown` → `WriteUnverified`.

### 1.4 Буфер обмена (`ClipboardAccess`, контракт §4.1 SYSTEM_ARCHITECTURE)
- **API [док]:** `NSPasteboard.generalPasteboard`, `changeCount` (аналог sequence number), `pasteboardItems`/`types` (перечисление без чтения), `NSPasteboardItem` + `NSPasteboardItemDataProvider` (ленивое предоставление данных — аналог delayed rendering), `prepareForNewContents(with: .currentHostOnly)` — **запрет Universal Clipboard** (аналог `CanUploadToCloudClipboard=0`) [док, UNVERIFIED текст].
- **Метки D-13 (аналог):** конвенция nspasteboard.org — `org.nspasteboard.TransientType`, `org.nspasteboard.ConcealedType`, `org.nspasteboard.AutoGeneratedType` [сообщ.: конвенция сообщества, соблюдение менеджерами буфера добровольное] — то же положение, что Q-8 (остаточный Medium).
- **Privacy pasteboard (macOS 26+):** Apple ввела предупреждение при программном чтении общего буфера без действия «вставить» пользователя и свойство `NSPasteboard.accessBehavior` + методы `detect…` для проверки типов без чтения [док: `developer.apple.com/documentation/appkit/nspasteboard/accessbehavior-swift.enum`, UNVERIFIED текст; сообщ.: MacRumors 2025-05-12, mjtsai 2025-05-12]. Поставляется ли включённым по умолчанию в 26/27 — **противоречивые данные** [сообщ.]. **Следствие:** snapshot (чтение прежнего содержимого) у KeySwitch может показать системное предупреждение. **Предложение:** `probe` использует только `types`/`detect`; если `accessBehavior` ≠ `alwaysAllow` → M2 запрещён (M3 или отказ); никогда не читать буфер «на всякий случай».
- **Сигнал потребления:** ленивый провайдер вызывается, когда **кто-то** запросил данные, но **без PID запросившего** [гип.: в API нет параметра процесса]. Менеджеры буфера запрашивают данные сразу после смены `changeCount` → вызов **до** Cmd+V — аналог раннего `WM_RENDERFORMAT` → прервать M2 (`ClipboardListenerInterference`). После Cmd+V сигнал = `FromUnknown` → по Q-1 **не восстанавливать** (D-26: исправленный текст остаётся, пользователю — «Вернуть буфер», снимок в обнуляемой памяти ≤ 120 с).
- **Восстановление:** только если `changeCount` равен нашему и есть подтверждённый для профиля сигнал (M-2/ручные тесты). Форматы снимка — белый список (plain text, RTF, HTML, `public.utf8-plain-text`); файлы/изображения/приватные типы → M2 запрещён.
- **Отказ:** `accessBehavior` запрещает/спрашивает; чужие приватные типы; объём > 16 МБ; ранний запрос данных.

### 1.5 Глобальные горячие клавиши (`Hotkeys`)
- **API [док]:** Carbon `RegisterEventHotKey` / `UnregisterEventHotKey` (HIToolbox), обработчик через `InstallEventHandler` на event target приложения; **разрешения не требуются** (не event tap, не Input Monitoring). Tauri-экосистема использует тот же механизм: crate `global-hotkey` [сообщ.: исходники crate].
- **Камни:**
  - **macOS 15.0:** регистрация сочетаний только с Option или Option+Shift возвращала `-9868` (сознательно, против keylogger); в 15.2 частично возвращено, в 2025 есть сообщения о нестабильности [сообщ.: Apple Developer Forums thread 763878, eternalstorms.at]. **Предложение:** сочетание по умолчанию — с Cmd или Ctrl; ошибка регистрации → `HotkeyError::RejectedByOs` с подсказкой в UI.
  - Работают ли зарегистрированные хоткеи при включённом Secure Event Input — [гип.] (данных нет), проверка в M-2/ручная.
  - Хоткей должен содержать не-модификаторную клавишу (D-14) — то же правило.
  - Событие приходит в главный поток (Carbon event loop = run loop `tao`) — влияет на модель потоков (§4.2).
- **Запрещено:** `CGEventTapCreate`, `NSEvent.addGlobalMonitorForEvents` (требует Input Monitoring/Accessibility и видит все нажатия — keylogger-класс), IOHID.

### 1.6 Определение и переключение источника ввода (`KeyboardLayouts`)
- **API [док]:** Text Input Sources (HIToolbox): `TISCopyCurrentKeyboardInputSource`, `TISGetInputSourceProperty(kTISPropertyInputSourceID / kTISPropertyInputSourceLanguages / kTISPropertyInputSourceType)`, `TISCreateInputSourceList` (фильтр: включённые раскладки), `TISSelectInputSource`; уведомление `kTISNotifySelectedKeyboardInputSourceChanged`. Разрешения не нужны.
- **Rust-привязки:** в семействе objc2 **нет** биндингов TIS (`objc2-carbon` 0.3.2 без фич; `objc2-hi-toolbox` не существует — проверено crates.io) → **собственный `extern "C"` FFI** (~10 функций) в `ks-platform-mac` с `// SAFETY:`.
- **Камни:**
  - Источник ввода по умолчанию **глобальный**, а не поточный (отличие от Windows HKL); при включённой настройке «Автоматически переключаться на источник ввода документа» — по документу [док-настройка, поведение — гип.]. `current_for(session)` на macOS фактически «текущий глобальный».
  - TIS-функции следует вызывать на главном потоке: сообщения о падениях/assert вне главного потока на macOS 14+ [сообщ.] → вызовы через главный поток.
  - `TISSelectInputSource` из фонового приложения иногда не применяется или не обновляет индикатор (особенно IME/CJK) [сообщ.: утилиты переключения раскладок] → после переключения перечитать текущий источник; несовпадение → `layout_switched = Unknown`, без повторов.
  - IME (тип `kTISTypeKeyboardInputMethodWithoutModes`/`…ModeEnabled`) → `ime_active = Yes` → отказ по существующей политике.
  - Соответствие раскладок данных ks-core (скан-коды, D-15): macOS использует виртуальные коды `kVK_*` (позиционные, ANSI/ISO/JIS) — нужна таблица соответствия `kVK_*` ↔ скан-коды и проверка раскладок «Russian», «Russian – PC», «ABC», «U.S.» [гип.: «Russian» и «Russian – PC» отличаются расположением знаков препинания] → задача для `/add-layout` (данные, не код ядра).

### 1.7 Приложение в строке меню (`Tray`)
- **API [док]:** `NSStatusItem` (через встроенную фичу Tauri `tray-icon`, crate `tray-icon` уже в `Cargo.lock` 0.25.1), `LSUIElement = true` / `ActivationPolicy::Accessory` (без значка в Dock).
- **Камни:** на MacBook с вырезом значок скрывается при переполнении строки меню [сообщ.]; в macOS 26 пользователь может скрывать элементы строки меню в Настройках [сообщ.] → нужна вторая точка входа (повторный запуск приложения открывает главное окно; пункт в настройках). Разрешений не требуется.

### 1.8 Автозапуск (`Autostart`)
- **API [док]:** `SMAppService.mainApp.register()/unregister()/status` (macOS 13+) — `https://developer.apple.com/documentation/servicemanagement/smappservice` (UNVERIFIED текст; сущ. подтверждено компиляцией `objc2-service-management`).
- **Камни:** состояние `requiresApproval` (пользователь выключил в «Настройки → Основные → Объекты входа»); система показывает уведомление «Добавлен фоновый объект» [сообщ.]; LaunchAgent-plist в `~/Library/LaunchAgents` — устаревший путь, **не использовать**. Плагин `tauri-plugin-autostart` не нужен (собственная реализация, как на Windows).

### 1.9 Разрешения и онбординг TCC (новый трейт `Permissions`, §4)
- **Нужно:** только **Accessibility** (`kTCCServiceAccessibility`) — чтение/запись AX и `CGEventPost` (Post Event покрывается Accessibility [сообщ.]; проверка `CGPreflightPostEventAccess` [пров.-комп.]).
- **Избегаем:** **Input Monitoring** (`kTCCServiceListenEvent`) — не нужен при `RegisterEventHotKey` и `NSEvent.modifierFlags`; `CGPreflightListenEventAccess` вызывается только для диагностики. **Automation** (Apple Events) — не нужен; для будущих адаптеров Word/Excel — только по ADR. **Screen Recording, Full Disk Access** — не нужны. Если в M-2 окажется, что что-то требует Input Monitoring, — это эскалация к CISO, не тихое расширение.
- **Поток:** онбординг объясняет зачем → `AXIsProcessTrustedWithOptions({kAXTrustedCheckOptionPrompt: true})` (системный диалог) → кнопка «Открыть настройки» (`x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility` [сообщ.]) → опрос `AXIsProcessTrusted()` раз в 1–2 с, пока окно онбординга открыто (колбэка нет [гип.]). Отзыв во время работы → AX-ошибки → отказ `PermissionMissing` + значок в строке меню.
- **Камни:** разрешение привязано к подписи (designated requirement: Team ID + bundle id). **Неподписанные/ad-hoc сборки теряют разрешение при каждой пересборке** (меняется cdhash; переключатель «включён», но не работает) [сообщ.] → для разработчиков `tccutil reset Accessibility <bundle-id>`; для пользователей — стабильная подпись Developer ID (§1.15). Смена Team ID/bundle id в обновлении = повторный запрос у всех пользователей (LEVEL 3: смена корня доверия).

### 1.10 Поля паролей и Secure Event Input
- **API [док]:** роль/подроль `AXSecureTextField` / `kAXSecureTextFieldSubrole`; `IsSecureEventInputEnabled()` (Carbon, TN2150) — включается любым процессом при фокусе в поле пароля, Terminal «Secure Keyboard Entry», менеджерами паролей.
- **Политика (аналог D-05):** `AXSecureTextField` → `Password`; `IsSecureEventInputEnabled() == true` → отказ `SecureInput` **до чтения текста** (новый код или отображение в `SecureDesktop`, §4). Исключения по bundle id по умолчанию: менеджеры паролей (1Password, Bitwarden, «Пароли», «Связка ключей»), терминалы (Terminal, iTerm2, Ghostty и т.п.).
- **Камни:** некоторые приложения «забывают» выключить secure input → KeySwitch будет отказывать везде [сообщ.]. Диагностика: PID виновника доступен через IORegistry-ключ `kCGSSessionSecureInputPID` [сообщ., не документировано] — допустимо только чтение через IOKit без запуска `ioreg`; показать имя приложения в диагностике (без текста).

### 1.11 Окно выбора языка для смешанного текста (`Chooser`)
- **API [док]:** `NSPanel` со `styleMask: .nonactivatingPanel`, `canBecomeKey = false`, `level = .floating`/`.statusBar`, показ `orderFrontRegardless` — не активирует KeySwitch и не забирает фокус у поля. Клавиши `1`/`2`/`Esc` — временные `RegisterEventHotKey` только на время показа с RAII-снятием и таймаутом 10 с — **прямой перенос решения CISO X-4**.
- **Камни:** регистрация хоткеев без модификаторов (`1`, `2`, `Esc`) — [гип.], допустимость под ограничениями macOS 15 проверить в M-2; если не удалось — операция отменяется (никаких мониторов событий как запасного пути). Мышь по панели без активации — [гип.]. Окно не показывает текст пользователя.

### 1.12 Изоляция процесса песочницы макросов (`SandboxLauncher`)
Варианты (Wasmtime-хост `ks-sandbox` — тот же код, что на Windows):
| Вариант | Механизм | Оценка |
|---|---|---|
| **A (рекомендую)** | отдельный исполняемый helper в бандле (`Contents/MacOS/ks-sandbox`) со своей подписью и entitlements **App Sandbox** (`com.apple.security.app-sandbox = true`, без `network.client/server`, без файлов, без `inherit`), запуск `posix_spawn` из основного процесса, обмен кадрами по унаследованным pipe | [док] App Sandbox; запуск sandboxed helper из несандбоксированного родителя и требование встроенного `Info.plist` (`-sectcreate __TEXT __info_plist`) для контейнера — [сообщ.] |
| B | XPC-сервис (`Contents/XPCServices`) с App Sandbox | [док]; но жизненным циклом управляет launchd → «новый процесс на каждый запуск» (Q-9) реализовать сложнее, kill — косвенный |
| C | seatbelt `sandbox_init` с собственным профилем | API помечен устаревшим (так делает Chromium через приватный SPI) → **не использовать** без ADR |
- **Чего нет на macOS по сравнению с Windows:** Job Object (лимит памяти процесса, запрет дочерних, UI-ограничения), win32k lockdown, ACG. **Компенсации:** лимит памяти — `wasmtime::StoreLimits` (64 МБ гостя) + сторож RSS через `proc_pid_rusage` + `SIGKILL`; `RLIMIT_AS` на macOS не применяется надёжно [сообщ.]; запрет дочерних процессов — правила App Sandbox [гип.]; таймаут — epoch + `SIGKILL` из родителя (аналог `TerminateJobObject`); helper не линкуется с AppKit [гип.: проверить `otool -L`].
- **JIT и hardened runtime:** Cranelift на Apple Silicon под hardened runtime требует `MAP_JIT` и entitlement `com.apple.security.cs.allow-jit` [док: Apple «Porting JIT compilers to Apple silicon», UNVERIFIED текст; сообщ.: wasmtime issue #12076 / #11989]. **Pulley (интерпретатор) не требует JIT-entitlement** → на macOS сильный аргумент за Pulley (как ACG на Windows). Решение — по замерам S-3/M-3, CISO.
- **Остаточный риск SEC-01 на macOS** (выход из Wasm + обход App Sandbox) — оценка CISO после M-3; ожидаю не ниже, чем на Windows без ACG [гип.].

### 1.13 Отчёты о сбоях
- **Поведение системы [сообщ.]:** `ReportCrash` пишет `.ips` в `~/Library/Logs/DiagnosticReports` локально (потоки, регистры, загруженные образы; без кучи); отправка Apple — только при согласии пользователя «Отправлять аналитику Mac»; для приложений вне App Store разработчик эти отчёты не получает.
- **Политика:** panic hook без payload (как на Windows); `panic = "abort"` в релизе; `UserText` живёт минимально (zeroize). Аналога исключения из WER (`WerAddExcludedApplication`) на macOS **нет** [гип.] → фрагменты текста теоретически могут оказаться в регистрах в `.ips` → **вопрос CISO** (предлагаю принять как Low: отчёт локальный, отправка — по согласию пользователя, в Apple, не нам).
- Собственных отчётов о сбоях с отправкой по сети — нет (правило 2 / D-25: сеть только канал обновлений).

### 1.14 Обновления (собственный `ks-updater`, ADR-0011; без `tauri-plugin-updater`)
Ред. 2026-11-11 по замечанию CISO UA-C14 / SEC-68 (`SECURITY_REVIEW.md` §7.4, §7.7). Предыдущая редакция (через Tauri updater) отменена.
- **Решение (ADR-0011, принят CISO с условиями; одобрение CLO и корень доверия K-1 — LEVEL 3 владельца — по ADR):** обновления выполняет собственный механизм TUF-lite: `ks-update-core` (без I/O, `#![forbid(unsafe_code)]`, правила доверия: роли root/release/policy/timestamp, сроки, анти-downgrade, `min_safe_version`, отзыв) + бинарь `ks-updater`. **`tauri-plugin-updater` не используется ни в одном crate** (§7.4 п. 1; так же запрещены `velopack`, `self_update`, `tough`, обвязки Sparkle). Конфликт с `SECURITY_REVIEW.md` §6 п. 3 снят правилом §7.4.
- **Формат артефактов сохраняется:** `bundle.createUpdaterArtifacts` → `KeySwitch.app.tar.gz` + `.sig` (minisign, `tauri signer` — инструмент сборки, не runtime) [сообщ.: вторичные руководства]; подпись проверяет `ks-update-core` (`minisign-verify`), записи `darwin-aarch64` / `darwin-x86_64` в нашем подписанном манифесте. DMG для обновлений не используется.
- **macOS-реализация `ks-updater` (план, кода нет):** отдельный helper-процесс с **App Sandbox** и единственным entitlement `com.apple.security.network.client`; сеть — только HTTP-стек ОС (`NSURLSession`), TLS/HTTP-crate запрещены (§7.4 п. 2); WebView прав на обновления не имеет (событие состояния + 3 команды без параметров). Helper только скачивает и проверяет метаданные/пакет; **замену бандла выполняет основной процесс** после проверок UA-C13. **Без запроса пароля администратора.**
- **UA-C13 (SEC-68, ответственный — AGENT 19):** проверка нового бандла `SecStaticCodeCheckValidity` (strict, nested code, все архитектуры) против **designated requirement текущего** приложения (Team ID + bundle id) [док: <https://developer.apple.com/documentation/security/secstaticcodecheckvalidity(_:_:_:)>, UNVERIFIED — страница не открывалась из облачной сессии]; App Translocation или нет права записи в каталог бандла (например, `/Applications` установлено администратором) → **только уведомление**; распаковка `.tar.gz` — только после проверки подписи артефакта, с лимитами размера и числа файлов, без абсолютных путей, `..`, symlink за пределы бандла, setuid/setgid. Обёртка `security-framework` (если понадобится) — только по правилу 3 CLAUDE.md.
- **UA-C11:** автоустановка на macOS — только при наличии Developer ID и нотаризации (K-6/K-7, L3-1); до этого режим «только уведомлять». Обновлённый `.app` подписан **тем же Developer ID** и нотаризован (тикет пристёплен до упаковки), иначе Gatekeeper/TCC сбросят Accessibility у пользователей [сообщ.].
- **Два независимых контроля:** minisign-подпись артефакта (ключи K-2…K-4 по `SIGNING_KEY_MANAGEMENT.md`) + подпись кода Apple (UA-C13). Защита от downgrade — в `ks-update-core` (версия в подписанном манифесте) + проверка версии в бандле.
- **До PASS GATE 10:** код `ks-updater` — только в ветках `spike/u-1-*` с тестовыми ключами и тестовым хостом (§7.4 п. 5); проверка на macOS — в спайке M-4 + ревью кода (SEC-68 Open).

### 1.15 Подпись кода, нотаризация, hardened runtime
- **Требования [док, UNVERIFIED текст; сообщ.]:** сертификат **Developer ID Application** (только в Apple Developer Program, §5); hardened runtime обязателен для нотаризации; `xcrun notarytool submit --wait` (ключ App Store Connect API в секретах CI) → `xcrun stapler staple`; `spctl -a -vvv` для проверки. Tauri поддерживает подпись/нотаризацию через переменные окружения сборки [сообщ.].
- **Почему без этого нельзя:** с macOS 15 Sequoia обход Gatekeeper через Control-click для ненотаризованных приложений **удалён**; остаётся «Настройки → Конфиденциальность и безопасность → Всё равно открыть» (кнопка появляется примерно на час после неудачного запуска) [сообщ.: Macworld, eclecticlight 2024-10-01]. Плюс TCC-разрешение у неподписанных сборок нестабильно (§1.9). **Публичная бета для macOS без Developer ID непрактична.**
- **Entitlements:** основной процесс — без App Sandbox (AX к чужим приложениям в App Sandbox невозможен [сообщ.] → **распространение вне Mac App Store**), без `allow-jit` (JIT WebKit выполняется в его собственных процессах [гип.]), без `disable-library-validation`; helper песочницы — §1.12.
- **CI:** подпись — только в отдельном job по защищённому тегу/окружению с обязательным ревьюером; PR из форков секретов не получают.

---

## 2. Сводная таблица функций

| Функция | API | Разрешение | Достижимость | Главный риск |
|---|---|---|---|---|
| Фокус и чтение | AXUIElement | Accessibility | док (Cocoa/WebKit); сообщ. (Electron) | AX-дерево Chromium выключено; таймауты |
| Выделение | `kAXSelectedTextRange` set | Accessibility | док; гип. для contenteditable | UTF-16 диапазоны |
| Замена M1 | `kAXSelectedText` set | Accessibility | док (API); гип. (Undo) | D-12 Undo не гарантирован |
| Замена M3 | CGEvent Unicode | Accessibility | док; сообщ. (20 символов) | смена фокуса, модификаторы |
| Замена M2 | NSPasteboard + Cmd+V | — (privacy pasteboard 26+) | док; гип. (сигнал) | нет PID потребителя → нет авто-восстановления |
| Хоткеи | RegisterEventHotKey | нет | док; сообщ. (огр. macOS 15) | Option-only сочетания |
| Источник ввода | TIS | нет | док; сообщ. (камни) | глобальный, главный поток, нет биндингов |
| Строка меню | NSStatusItem (tray-icon) | нет | док | скрытие при переполнении |
| Автозапуск | SMAppService | нет (подтверждение пользователя) | док (13+) | `requiresApproval` |
| Онбординг TCC | AXIsProcessTrustedWithOptions | — | док | привязка к подписи |
| Пароли / secure input | `AXSecureTextField`, `IsSecureEventInputEnabled` | — | док | «залипший» secure input |
| Окно выбора | NSPanel nonactivating + временные хоткеи | нет | док; гип. (хоткеи без модиф.) | — |
| Песочница | helper + App Sandbox | — | док; сообщ. | нет Job Object/ACG; JIT |
| Сбои | ReportCrash (локально) | — | сообщ. | нет исключения из отчётов |
| Обновления | свой `ks-updater` (ADR-0011): `.app.tar.gz` + minisign, `NSURLSession`, helper в App Sandbox | сеть (D-25), только `ks-updater` | план (кода нет) | UA-C13: проверка designated requirement, распаковка tar; без Developer ID — только уведомление |
| Подпись | Developer ID + notarytool + hardened runtime | — | док | платно (LEVEL 3) |

---

## 3. Условия отказа (macOS-специфичные, дополняют общий конвейер)
1. `AXIsProcessTrusted() == false` или AX-ошибка `APIDisabled` → `PermissionMissing`.
2. `IsSecureEventInputEnabled() == true` → `SecureInput` (до любого чтения).
3. Роль `AXSecureTextField` или подроль secure → `Password`.
4. Фронтальное приложение = KeySwitch → `SelfTarget`; приложение из списка исключений (bundle id) → `Excluded`.
5. Экран заблокирован, окно входа, переключение пользователей, экранная заставка → отказ.
6. Активен IME (тип источника — input method) → `ImeActive`.
7. Нет каретки/диапазона, мульти-выделение (`kAXSelectedTextRanges` > 1) → отказ.
8. Electron/Chromium без готового AX-дерева → `ScopeUndetermined` (повтор — только по профилю).
9. M1 неприменим (не settable или класс без подтверждённого Undo), M3 запрещён (длина > порога, `\n`/`\t`), M2 запрещён (privacy pasteboard, приватные типы, ранний запрос данных) → `NoSafeWriteMethod`.
10. Смена фронтального приложения/элемента/текста между чтением и записью → `TargetChanged`.

---

## 4. Отображение на трейты `ks-platform` и предложения для Architect (AGENT 03)

### 4.1 Соответствие
| Трейт (AV2 §3.2) | macOS-реализация | Изменения |
|---|---|---|
| `Environment` | `NSWorkspace.frontmostApplication` (pid, bundle id), `proc_pidinfo` (время старта), `IsSecureEventInputEnabled`, состояние сессии (`CGSessionCopyCurrentDictionary`) | П-1, П-2, П-3 |
| `FieldAccess` | AXUIElement в арене главного/платформенного потока; `SessionId` → CF-объекты внутри адаптера | П-4, П-5 |
| `InputSynth` | `CGEvent` (Unicode, Cmd+V, Cmd+A, разрешённые клавиши D-07), `NSEvent.modifierFlags` | П-6 |
| `ClipboardAccess` / `PasteTx` | `NSPasteboard` + ленивый провайдер, `.currentHostOnly`, метки nspasteboard.org | П-7, П-8 |
| `KeyboardLayouts` | TIS (FFI), только главный поток | П-9 |
| `Chooser` | `NSPanel` nonactivating + временные хоткеи | без изменений |
| `Hotkeys` | `RegisterEventHotKey` | П-10 |
| `Autostart` | `SMAppService.mainApp` | П-11 |
| `SandboxLauncher` | helper + App Sandbox, `posix_spawn`, pipe, `SIGKILL` | П-12 |
| — | новый `Permissions` | П-13 |

### 4.2 Предложения изменений (для решения Architect, без изменения ks-core/ks-engine-логики)
- **П-1** `Environment::input_desktop_is_default() -> bool` → обобщить до `fn secure_input_state(&self) -> PResult<SecureInputState>` (`Normal | SecureDesktop | SecureEventInput | Locked`). Windows: secure desktop; macOS: secure input / блокировка.
- **П-2** `ForegroundInfo.integrity: IntegrityRelation` — на macOS понятия нет → всегда `Same`/`Unknown`; оставить, документировать. `window_class: String` → непрозрачный `WindowTag` (на macOS — роль/подроль окна), чтобы профили не зависели от Win32-классов.
- **П-3** `AppIdentity` — добавить `bundle_id: Option<String>` (основной ключ исключений/профилей на macOS; имя exe ненадёжно).
- **П-4** `FieldKind` — заменить Windows-перечень на платформенно-нейтральный (`SingleLineText | MultiLineText | WebText | Document | Other`) + платформенная деталь внутри `FieldCaps`; добавить `FieldCaps::native_replace_undo: Tri` (M1 допустим только при `Yes` по профилю — D-12).
- **П-5** `AbortReason` — добавить `PermissionMissing`, `SecureInput`, `AccessibilityTreeUnavailable` (или явно отобразить их в существующие коды; решение Architect + PA для UI-текстов).
- **П-6** `InputSynth::send_paste_chord`/`ctrl_a` → `paste_chord`/`select_all_chord` (Cmd на macOS); `wait_modifiers_released` — через `NSEvent.modifierFlags` (без Input Monitoring).
- **П-7** `ClipboardProbe` — добавить `access: ClipboardAccessState { Allowed, WouldPrompt, Denied }`; `probe` обязан не читать содержимое (только типы).
- **П-8** `PasteTx::wait_consumed -> bool` → `-> ConsumeSignal { FromTarget, FromUnknown, Early, None }`. На macOS максимум `FromUnknown` → по Q-1 восстановление не выполняется; Windows возвращает `FromTarget` при совпадении PID.
- **П-9** `KeyboardLayouts::current_for(SessionId)` — документировать, что на macOS результат глобальный; добавить `fn scope(&self) -> LayoutScope { PerThread, Global }`, чтобы движок не делал выводов о «раскладке окна».
- **П-10** `HotkeyError` — добавить `RejectedByOs` (`-9868`), отличать от `Conflict`.
- **П-11** `Autostart::enabled -> bool` → `state() -> AutostartState { On, Off, RequiresApproval }`.
- **П-12** `ProcessLimits` — разделить на «обязательные» и «если поддерживается» (`memory_hard_limit`, `no_child_processes`, `ui_restrictions`); `SandboxLauncher::spawn` возвращает фактически применённый профиль для самопроверки и журнала (CISO).
- **П-13** Новый трейт `Permissions { fn status(&self) -> PermissionStatus; fn request(&self, p: Permission); fn open_settings(&self, p: Permission); }` (`Permission::Accessibility`). Windows-реализация — всегда `Granted`. Используется онбордингом (UI-команда без текста) и шагом Gate.
- **Модель потоков (AV2 §4):** на macOS функции T1 (хоткеи, окно выбора, владелец буфера) и TIS/AppKit обязаны работать на **главном потоке** (run loop `tao`), т.е. T0 и T1 совмещаются; AX-вызовы — в T2 (AX допускает вызовы из фонового потока [сообщ.]) с `AXUIElementSetMessagingTimeout`. Нужна абстракция «выполнить на платформенном UI-потоке с дедлайном» в `ks-platform` вместо прямого знания о T1.

---

## 5. Команда и LEVEL 3

### 5.1 Нужен ли отдельный macOS Integration Developer
- **Сейчас (M-1…M-2):** нет. M-2 — пробник на одном-двух crate, выполняю я (AGENT 19) с ревью AGENT 05 (паттерны S-2) и CISO.
- **Нужен с момента старта реализации `ks-platform-mac`** (после закрытия M-2 и GATE 4 для macOS, параллельно GATE 7 Windows): объём сопоставим с `ks-platform-win` (AX, CGEvent, NSPasteboard, TIS-FFI, NSPanel, SMAppService, helper песочницы, подпись/нотаризация в CI). Lead не должен принимать собственную работу (V7 §7 — независимая приёмка) → **рекомендую добавить AGENT «macOS Integration Developer» (Rust + objc2 + AppKit) к началу M-3**; приёмку ведут QA (08) и владелец на реальном Mac.
- **Ручные тесты на реальном Mac** (D-10-аналог для GATE 8/16) — без Mac у владельца или тестировщика GATE 8 не закрыть: hosted-раннеры не заменяют приложения из матрицы (Word, Telegram, WhatsApp и т.д.).

### 5.2 Решения владельца (LEVEL 3)
| № | Вопрос | Рекомендация | Когда |
|---|---|---|---|
| L3-1 | **Apple Developer Program** — **99 USD в год** (одинаково для частного лица и организации; [сообщ.: несколько независимых источников 2026; страница Apple не проверена — UNVERIFIED]). Без неё нет Developer ID и нотаризации → нет практичного распространения на macOS 15+. Регистрация организации требует D-U-N-S и юрлица (связано с D-23) [сообщ.] | Купить до первой внешней macOS-беты (не нужно для M-2/M-3: CI и локальные тесты идут на ad-hoc подписи) | до GATE 16 (план: к началу бета-тестов) |
| L3-2 | **Корень доверия macOS:** Team ID и bundle id (`com.<org>.keyswitch`) фиксируются навсегда — их смена сбрасывает Accessibility у всех пользователей и ломает цепочку обновлений (V7 §11) | Выбрать вместе с решением о названии (D-22(5)) и юрлице (D-23) | до первой подписанной сборки |
| L3-3 | **Аппаратное обеспечение для ручных тестов:** Mac на Apple Silicon (минимум) + доступ к Intel Mac при поддержке Intel | Если у владельца нет Mac — согласовать покупку/аренду (облачные Mac) отдельно (Finance Manager — расчёт) | до закрытия M-2 |
| L3-4 | Поддержка Intel (см. `COMPATIBILITY_MATRIX_MACOS.md` §2) | Universal binary в v1, Intel — поддержка до конца обновлений безопасности macOS 26 | к GATE 4 macOS |

---

## 6. Rust crate (проверено crates.io 2026-11-10)

| Crate | Версия (дата) | Лицензия | Назначение | В `Cargo.lock` | Оценка |
|---|---|---|---|---|---|
| `objc2` | 0.6.5 (2026-10-05) | MIT | рантайм Objective-C | да (через Tauri) | ок (≥ 2 недель) |
| `objc2-foundation` | 0.3.2 (2025-10-04) | MIT | NSString и др. | да | ок |
| `objc2-app-kit` | 0.3.2 (2025-10-04) | Zlib OR Apache-2.0 OR MIT | NSPasteboard, NSEvent, NSPanel, NSWorkspace | да | ок |
| `objc2-core-foundation` | 0.3.2 (2025-10-04) | Zlib OR Apache-2.0 OR MIT | CFString/CFType | да | ок |
| `objc2-core-graphics` | 0.3.2 (2025-10-04) | Zlib OR Apache-2.0 OR MIT | CGEvent, CGPreflight* | да | ок |
| `objc2-application-services` | 0.3.2 (2025-10-04) | Zlib OR Apache-2.0 OR MIT | **AXUIElement**, AXValue, AX-константы, `AXIsProcessTrusted` | **нет — новая** | рекомендую; фичи `AXUIElement`, `AXError`, `AXValue`, `AXAttributeConstants`, `AXRoleConstants`, `HIServices` |
| `objc2-service-management` | 0.3.2 (2025-10-04) | Zlib OR Apache-2.0 OR MIT | SMAppService | **нет — новая** | рекомендую |
| `objc2-carbon` | 0.3.2 (2025-10-04) | Zlib OR Apache-2.0 OR MIT | — | нет | **не годится**: фич нет, TIS/RegisterEventHotKey отсутствуют → FFI вручную |
| `core-foundation` | 0.10.1 (2025-05-26) | MIT OR Apache-2.0 | старое семейство servo | да | не брать новых зависимостей от него (единое семейство objc2) |
| `core-graphics` | 0.25.0 (2025-05-27) | MIT OR Apache-2.0 | старое семейство | — | не нужно |
| `accessibility` / `accessibility-sys` | 0.2.0 (2025-03-22) | MIT / Apache-2.0 | обёртки AX (старое семейство) | нет | не рекомендую: малая аудитория, дублирует `objc2-application-services` |
| `macos-accessibility-client` | 0.0.2 (2026-01-18) | Apache-2.0 | только `AXIsProcessTrusted` | нет | не нужно (одна функция) |
| `global-hotkey` | 0.10.0 (**2026-11-09**) / 0.9.0 (2026-10-16) | Apache-2.0 OR MIT | RegisterEventHotKey | нет | как **референс**; собственная реализация (как на Windows); 0.10.0 моложе 2 недель |
| `tauri-plugin-macos-permissions` | 2.3.0 (2025-05-06) | MIT | проверка TCC из WebView | нет | **не брать**: расширяет поверхность IPC, онбординг — собственными командами |
| `tray-icon` | 0.26.1 (2026-10-07); в lock 0.25.1 | MIT OR Apache-2.0 | NSStatusItem | да | через фичу Tauri |
| `security-framework` | 3.7.0 (2026-02-20) | MIT OR Apache-2.0 | Keychain/подписи | нет | только если понадобится проверка подписи бандла при обновлении |

Примечания: (1) `deny.toml` `targets` содержит только Windows и Linux → **добавить `aarch64-apple-darwin`, `x86_64-apple-darwin`** (AGENT 09), иначе `cargo deny` не проверяет macOS-граф. (2) Лицензии `Zlib OR Apache-2.0 OR MIT` и `MIT` — в allow-list `deny.toml`; подтверждение CLO — в `LICENSE_REGISTER.md`. (3) `unsafe` — только в `ks-platform-mac` (SEC-L2), каждый блок с `// SAFETY:`.

---

## 7. План CI для macOS

### 7.1 Раннеры GitHub Actions (README `actions/runner-images`, прочитан 2026-11-10)
| Метка | ОС | Архитектура | Примечание |
|---|---|---|---|
| `macos-latest`, `macos-26` | macOS 26 | arm64 (Apple Silicon) | `macos-latest` → 26 |
| `macos-15` | macOS 15 | arm64 | |
| `macos-26-intel` | macOS 26 | x64 (Intel) | |
| `macos-15-intel` | macOS 15 | x64 | |
| `macos-14` | macOS 14 | — | **выведен из GitHub Actions 2026-11-02** |
| `xcode-27` | macOS 27 (preview) | arm64 | preview, не для обязательных проверок |
| `*-large`, `*-xlarge` | — | — | платные, **не используем** |
Стандартные раннеры для публичных репозиториев бесплатны [сообщ.: документация GitHub о биллинге; D-09]. Сроков вывода Intel-образов 15/26 на странице нет; учитывая, что macOS 27 — только Apple Silicon, ожидаю их вывод в 2027–2028 [гип.].

### 7.2 Можно ли выдать Accessibility на hosted-раннере
- **Факт [исходник образа, прочитан мной]:** `configure-tccdb-macos.sh` при сборке образа вносит `kTCCServiceAccessibility` (и `kTCCServicePostEvent`) в **системный** TCC.db для `/bin/bash`, `/opt/hca/hosted-compute-agent`, провижионера раннера, `osascript`, Terminal, и в пользовательский TCC.db — для `/bin/bash`, `osascript`, Terminal. Комментарий в том же файле: TCC учитывает **ответственный процесс**, «на hosted-раннере это агент, какой бы инструмент ни запускал job».
- **Вывод [гип., проверка — шаг 0 M-2]:** наш пробник, запущенный шагом job, вероятно, наследует Accessibility агента → AX и `CGEventPost` работают **без** модификации TCC.db. Добавлять свои записи в системный TCC.db во время job не планирую (зависит от SIP; не проверено `csrutil status`). Существует action `xa11y/setup-a11y` (MIT) — механизм не описан, **не используем** без ревью.
- **Ограничения:** раннер — VM с автологином, но без реального пользователя; разрешение «наследуется», т.е. **онбординг TCC (диалог, отзыв, привязка к подписи) на CI не проверяется** — только вручную.

### 7.3 Что проверяется headless / на CI / только вручную
| Уровень | Что | Где |
|---|---|---|
| Linux (каждый PR) | `cargo +1.97.0 clippy -p ks-platform-mac --target aarch64-apple-darwin` (компиляция без линковки — **проверено мной пробой**); предложение для `scripts/check.sh` | облачная сессия, `core-linux` |
| macOS CI, без UI | `cargo test --workspace` (arm64 и x86_64), `tauri build --bundles app` (ad-hoc подпись), `codesign --verify`, `otool -L` для helper песочницы (нет AppKit), unit-тесты таблиц `kVK_*`, тесты TIS-чтения | `macos-15`, `macos-15-intel` |
| macOS CI, с UI (информационно) | собственные фикстуры AppKit/WKWebView/Electron: AX-чтение, выделение, M1/M3/M2, буфер, пароли, secure input (фикстура включает `EnableSecureEventInput`), окно выбора | `macos-15`, `macos-26` |
| Только вручную (владелец/QA на реальном Mac) | онбординг TCC и отзыв; подписанная/нотаризованная сборка, Gatekeeper; обновление `.app.tar.gz` с сохранением разрешения; приложения из матрицы (Word, Excel, Telegram, Discord, WhatsApp, Chrome, Firefox, Safari, Notes); переключение раскладки «Russian»/«ABC» с индикатором; строка меню с вырезом; SMAppService `requiresApproval`; privacy pasteboard на 26/27 | реальный Mac |

### 7.4 Предлагаемые job (для AGENT 09; actions по SHA, правило 4)
- `macos` (`runs-on: macos-15`, `timeout-minutes: 30`): `npm ci --ignore-scripts`, `npm run build`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `npx tauri build --bundles app --target aarch64-apple-darwin` (без секретов, ad-hoc), артефакт `keyswitch-macos-arm64-unsigned`.
- `macos-intel` (`runs-on: macos-15-intel`): `cargo test --workspace` на x86_64.
- `macos-integration` (после M-2 — информационный, затем обязательный для `feat/platform-mac`), матрица `macos-15`, `macos-26`.
- `macos-release` — только по защищённому тегу/окружению: universal build, подпись Developer ID, notarytool, stapler, `.app.tar.gz` + minisign. **После L3-1.**

---

## 8. Дизайн спайка M-2 «macOS integration probe» (аналог S-2)

**Ветка:** `spike/mac-ax-probe` · **Исполнитель:** AGENT 19 (до появления macOS Integration Developer) · **Ревью:** AGENT 05 (единообразие с S-2), CISO (буфер, secure input, хоткеи) · **Таймбокс:** 5 рабочих дней · **Зависит от:** job `macos` (§7.4) в CI; трейты из BL-ARCH-01 (можно на временных локальных трейтах, как S-2).

### 8.1 Цель
Подтвердить/опровергнуть гипотезы §1 на CI-фикстурах: AX-чтение/выделение/запись, Undo для M1, предел M3, сигналы буфера, распознавание паролей и secure input, окно выбора без активации, временные хоткеи без модификаторов, TIS. Получить таблицу «проверено / не проверено» для каждой [гип.].

### 8.2 Состав
```
crates/ks-platform-mac/          # минимум для пробника (AX, CGEvent, NSPasteboard, TIS-FFI, RegisterEventHotKey)
tools/ks-probe-mac/              # консольный бинарь; JSON-отчёт без пользовательского текста (только дайджесты, коды, длины классов)
tests/mac-fixtures/
  appkit-fixture/  (Rust + objc2-app-kit) — NSTextField, NSSecureTextField, NSTextView (plain, multiline),
                   NSTextView rich (с вложением-картинкой), NSTextField read-only, NSSearchField;
                   кнопка-режим «включить EnableSecureEventInput»; режим «зависнуть в обработчике 2 с»
  web-fixture/index.html — тот же, что в S-2 (input, textarea, password, contenteditable, role=textbox, <img>, контролируемый input)
  wkwebview-fixture/ (Rust + objc2-web-kit, тот же index.html)
  electron-fixture/  (тот же electron, что в S-2, dev-only)
```
Строки — только синтетические (`Ghbdtn, rfr ltkf?`). Фикстуры не входят в продукт.

### 8.3 Сценарии (каждый — отдельный тест с JSON-результатом)
| ID | Сценарий | Фикстуры | Критерий «пройдено» |
|---|---|---|---|
| M-P00 | Окружение: `AXIsProcessTrusted`, `CGPreflightPostEventAccess`, `CGPreflightListenEventAccess`, `csrutil status`, версия ОС/архитектура | — | значения записаны; если AX не доверен — все AX-сценарии «не проверено», не «провалено» |
| M-P01 | Фокус: роль/подроль, pid, отпечаток; `AXSecureTextField` распознан | все | совпадение с ожидаемой матрицей; пароль распознан везде |
| M-P02 | Чтение всего поля, лимит 5 000 (6 000 → `TooLong`) | все текстовые | текст (дайджест) совпадает |
| M-P03 | Установка `kAXSelectedTextRange` + перечитывание (I-4) | все | выделение совпадает; для contenteditable — фиксируем факт |
| M-P04 | Фрагмент до каретки через `InsertionPointLineNumber`/`RangeForLine` | NSTextView multiline, textarea | фрагмент == последняя строка до каретки |
| M-P05 | **M1** `kAXSelectedText` set + Cmd+Z | NSTextField, NSTextView, web, Electron | замена верна; **число Cmd+Z до исходного** записано (D-12) |
| M-P06 | **M3** Unicode RU/EN, суррогатная пара, 1 и 20 символов на событие, `CGEventPostToPid` vs HID tap | все | текст верен; предел подтверждён/опровергнут |
| M-P07 | **M2** ленивый провайдер: момент запроса данных до/после Cmd+V; `.currentHostOnly`; метки nspasteboard.org | AppKit, web | факт сигнала записан; восстановление только по правилу Q-1 |
| M-P08 | Snapshot/restore: текст+RTF+HTML+изображение; приватный тип → отказ M2; `accessBehavior` записан | — | побайтное равенство; отказ срабатывает |
| M-P09 | Ранний «слушатель» (вспомогательный процесс читает буфер при смене `changeCount`) | — | M2 прерван, `ClipboardListenerInterference` |
| M-P10 | Смена фокуса между чтением и записью | AppKit | `TargetChanged`, поле не изменено |
| M-P11 | Изменение текста между чтением и записью | AppKit, web | `TargetChanged` |
| M-P12 | Secure Event Input включён фикстурой | AppKit | отказ `SecureInput` до чтения; хоткей — факт срабатывания записан |
| M-P13 | Встроенный объект (`<img>`, вложение NSTextView) | web, AppKit rich | `EmbeddedObjects` |
| M-P14 | Electron: AX-дерево до/после `AXManualAccessibility`, время готовности | Electron | факт и время записаны |
| M-P15 | TIS: перечень, текущий источник, `TISSelectInputSource` (ABC ↔ Russian, если раскладку удалось включить на раннере) | — | источник == целевой или `Unknown` записан |
| M-P16 | Зависшая фикстура | AppKit | отказ ≤ дедлайна, пробник не завис |
| M-P17 | Окно выбора: NSPanel без активации + временные хоткеи `1/2/Esc` | AppKit | фронтальное приложение не сменилось; регистрация удалась/не удалась — записано |
| M-P18 | `RegisterEventHotKey`: Cmd/Ctrl-сочетание, Option-only (ожидаем `-9868` или успех — факт), без модификаторов | — | результаты записаны |

### 8.4 Критерии успеха (закрытие M-2)
1. M-P00…M-P13 и M-P16 выполнены на `macos-15` **и** `macos-26` (arm64): зелёные или с объяснённым фактом; M-P01, M-P02, M-P06 — также на `macos-15-intel`.
2. **Нет ни одного сценария, где пробник изменил поле вопреки отказу**; пароль/secure input распознан во всех фикстурах.
3. Решение по каждому способу записи: M1 — для каких классов Undo = 1 шаг; M3 — подтверждённый размер пакета; M2 — какой сигнал реально доступен.
4. Подтверждено, что Input Monitoring не нужен ни для одного шага (иначе — эскалация CISO).
5. Отчёт `docs/spikes/M-2-mac-ax-probe.md`: матрица фикстура × сценарий × результат, статусы всех [гип.] этого документа, уточнённые предложения П-1…П-13, **чек-лист ручной приёмки владельца** (приложения `COMPATIBILITY_MATRIX_MACOS.md`, онбординг TCC, подписанная сборка, privacy pasteboard). Отчёт без пользовательского текста.
6. Job `macos-integration` информационный до закрытия M-2.

### 8.5 Следующие шаги после M-2
M-3 — песочница на macOS (helper + App Sandbox, Pulley vs Cranelift + allow-jit, H-01…H-40 на `macos-15`) — совместно с AGENT 13/14; M-4 — подписанная/нотаризованная сборка и обновление `.app.tar.gz` (после L3-1, совместно с AGENT 09 и CISO).
