# Конкурентный ландшафт KeySwitch — первичное исследование

Автор: AGENT 10 Market & Competitive Intelligence Analyst · Дата: 2026-11-10 · Статус: **первый отчёт, для Orchestrator → Product Analyst** (B-14)
Связанные отчёты: `2026-11-10-user-needs.md`, `2026-11-10-desktop-ui-references.md`, `ideas-backlog.md`.

## 0. Методика и ограничения (читать первым)

**Маркировка утверждений:**
- **[V]** — *verified*: аналитик сам прочитал первичный источник (README, CHANGELOG, файл версии в репозитории) 2026-11-10.
- **[R]** — *reported*: сведения из поисковой выдачи (сводка поисковика по странице), каталогов (AlternativeTo, Wingetly, SaaSHub), СМИ, форумов. Страницу целиком аналитик не открывал.
- **[A]** — *assumption*: вывод/оценка аналитика, не факт.

**Доступность источников (2026-11-10).** Из облачной сессии `WebFetch` не открыл ни одного сайта: `yandex.ru`, `habr.com`, `alternativeto.net`, `vc.ru` — ошибка DNS (хосты вне разрешённой сети). `github.com` и `api.github.com` — 403, `gitea.com` — 403. Доступны только `raw.githubusercontent.com` (через Bash) и `WebSearch`. Поэтому:
- для закрытых продуктов (Punto, Caramba, EveryLang, Orfo, LangOver и др.) все данные — **[R]**, с официальных сайтов напрямую ничего не прочитано;
- для open-source проектов на GitHub (Kright, RuSwitcher, SimpleSwitcher, LangSwitcher, Keyboard Switch, Espanso, PowerToys, AutoHotkey) README/CHANGELOG прочитаны напрямую — **[V]**;
- даты релизов закрытых продуктов почти везде не установлены; там, где написано «не установлено», — это честный пробел, а не «нет релизов».

Чтобы повысить уровень доказательности, нужно разрешить в сети сессии хосты `yandex.ru`, `caramba-switcher.com`, `habr.com`, `alternativeto.net`, `github.com`, `apps.apple.com` и повторить проверку (см. §6).

Все URL ниже — дата обращения **2026-11-10**, если не указано иное.

---

## 1. Ключевые выводы (TL;DR)

1. **Рынок фрагментирован, лидер стагнирует.** Punto Switcher (Яндекс) — самый известный продукт, но, по сообщениям, находится «в режиме фоновой поддержки», версия для Mac заморожена с 2017 г.; репутационный багаж — функция «Дневник» (журнал всех нажатий), телеметрия «по сообщениям», навязывание ПО Яндекса в установщике **[R]**.
2. **Основной коммерческий наследник — Caramba Switcher** (автор Punto): freemium, Pro по подписке ≈ $4.99/год (Windows) и $6.99/год (Mac App Store), «одна настройка», ставка на автопереключение «на нейросетях» **[R]**.
3. **Open-source волна 2025–2026 гг.** — Kright (MIT, Win+Mac, подписанные сборки, Microsoft Store), RuSwitcher (MIT, Mac 3.5.0, Windows-бета 0.9.0), SimpleSwitcher (C++23, иммутабельные GitHub-релизы), LangSwitcher, Keyboard Switch **[V]**. Они закрывают «бесплатно + приватно» и быстро наращивают функции.
4. **Все изученные конкуренты, кроме части «только по выделению», используют глобальный перехват клавиатуры**: `WH_KEYBOARD_LL` на Windows, `CGEventTap` + Input Monitoring на macOS **[V]** (Kright, RuSwitcher-Win, SimpleSwitcher) / **[R]** (Punto, Caramba). Именно это делает возможным «последнее слово без выделения» и автопереключение — и именно это вызывает подозрения «кейлоггер», конфликты с играми/RDP/антивирусами.
5. **Ниша KeySwitch по принципам почти пуста:** продукта, который *не перехватывает клавиатуру вовсе* (только `RegisterHotKey`), *доказуемо* не имеет сети и *честно отказывается* при риске порчи текста, на рынке не найдено **[A, по выборке из ~20 продуктов]**. Цена этого — нет автопереключения и нет «последнего слова без выделения» на основе буфера нажатий; нужно компенсировать через UIA (D-03).
6. **Macro Studio с песочницей JS/Python у переключателей раскладки нет ни у кого** из найденных **[A]**. Ближайшие аналоги — текстовые экспандеры (Espanso, PhraseExpress, Beeftext) и AutoHotkey, у которых скрипты исполняются **с полными правами пользователя** **[V]** (Espanso: «Shell commands support»; SimpleSwitcher: запуск процессов с правами SS).
7. **Конфликт имени.** На Product Hunt уже есть продукт **«KeySwitch»** — бесплатная утилита Windows (по комментарию автора — и macOS 1.0.9) для исправления неверной раскладки по горячей клавише **[R]**; на crates.io есть crate `keyswitch` (macOS, MIT, 0.1.2, 2025-10-12) **[R]**. → **Решение владельца о названии/товарном знаке нужно до публичного релиза** (см. MI-20 в backlog).

---

## 2. Прямые конкуренты: переключатели/исправители раскладки

### 2.1 Сводная таблица

| Продукт | Платформы | Статус / последний релиз | Модель | Механизм | Приватность / репутация |
|---|---|---|---|---|---|
| **Punto Switcher** (Яндекс) | Win; Mac (заморожен) | Win 4.5.0, дата не установлена; страница comss.ru обновлена 2024-04-27 [R]; «фоновая поддержка» [R] | Бесплатно; бандлинг ПО Яндекса [R] | Автопереключение по словарям + хоткей; глобальный хук (DLL `pshook.dll` [R]) | «Дневник» = журнал нажатий [R]; телеметрия «по сообщениям» [R]; история обвинений 2010 г. и опровержение [R] |
| **Caramba Switcher** | Win, Mac | Активен; Pro для Windows «появился недавно» [R] | Classic бесплатно; Pro $4.99/год (Win, 3 ПК), $6.99/год (Mac App Store), 499 ₽/год (Allsoft) [R] | Автопереключение («нейросети»), Shift×2 — последнее слово [R] | Политику приватности найти не удалось [R]; Mac-отзывы критикуют подписку [R] |
| **Kright** | Win (C#/.NET 8), Mac (Swift) | Win 1.1.1 — 2026-08-14; Mac 1.1.0 — 2026-07-18 [V] | Бесплатно, MIT; подписанные сборки, MS Store [V] | Хоткей Ctrl+Alt+K; `WH_KEYBOARD_LL` «пассивно смотрит нажатия»; запись через UIA, в консолях — SendInput+буфер; опц. автоисправление [V] | «No network, nothing stored» [V]; автообновление встроено [V] |
| **RuSwitcher** | Mac 3.5.0; Win 0.9.0 бета | Активен, даты не установлены [V-версии] | Бесплатно, MIT; донаты [V] | Mac: `CGEventTap` listen-only; Win: `WH_KEYBOARD_LL`, `SendInput` Unicode, `ISpellChecker` [V] | «No telemetry» [V]; но опц. загрузка словаря ~250 КБ и проверка обновлений раз в день [V] |
| **SimpleSwitcher** | Win (+ отдельная Linux-версия) | 6.x, иммутабельные релизы на GitHub Actions [V], дата не установлена | Бесплатно, open source (GPL-3.0 [R]); донаты [V] | Глобальный хук; CapsLock/F24, «double»/«up»-триггеры; запуск процессов и эмуляция клавиш [V] | «Очищает/восстанавливает буфер, чистит память по таймеру» [V] |
| **Mahou** | Win | Релизы 2017+ на Gitea, актуальная версия не установлена [R] | Бесплатно, GPL-2.0 [R] | Хук, конвертация выделенного, сниппеты [R] | — |
| **EveryLang** | Win | Активность не установлена; в реестре отечественного ПО РФ [R] | Freemium + бессрочная лицензия Pro [R] | Переключение + перевод через Google/Microsoft/Yandex, орфография, сниппеты, **журнал набора** [R] | Перевод = отправка текста в облако [R]; журнал набора [R] |
| **Orfo Switcher / VirtAssist** | Win | 3.050 — 2011-09-09 [R] → заброшен [A] | Donateware / pay once [R] — противоречие | Словарь «слово есть в одном языке» [R] | — |
| **Arum Switcher** | Win | Обзоры 2008–2011 [R] → вероятно заброшен [A] | Бесплатная 3.2 [R] | Хоткей, исправление опечаток, регистр [R] | — |
| **Keyboard Ninja** | Win | 2.1, совет запускать в режиме совместимости с Windows 98 [R] → заброшен [A] | Freeware [R] | Автопереключение 7 языков [R] | — |
| **Switch It (switchit.ru)** | Win | «работа над программой, видимо, прекратилась» [R] | — | — | — |
| **dotSwitcher** | Win | Discontinued [R] | Open source [R] | — | — |
| **MySwitcher** | Win 7–11 x64 | Версия не установлена [R] | Бесплатно (личн. и рабочее), проприетарно [R] | Авто/вручную UK/EN/RU, отдельный словарь украинского, ручной режим для терминалов/IDE/игр [R] | — |
| **LangOver** | Win | Обзор 2000-х [R]; цена не найдена | — | Хоткей, последний текст/выделение; иврит↔англ. [R] | — |
| **reLayout, Kavanati** | Win, Mac | Активность не установлена | Kavanati: Pro за персональный словарь [R] | Автоисправление иврит↔англ. «as you type» [R] | — |
| **LangSwitcher** (reg2005) | Mac 13+ | Активен [V] | Бесплатно, MIT [V] | Выделение + хоткей Shift×2; «Greedy Line»/«Last Word» через авто-выделение; **журнал конвертаций выкл. по умолчанию** [V] | «No network access» [V] |
| **Keyboard Switch** (TolikPylypchuk) | Win, Mac (+Linux [R]) | Активность не установлена | Бесплатно, MIT [R] | Только выделение → копировать → конвертировать → вставить (через буфер) [V] | Работает через буфер [V] |
| **Lang Switcher, Type Switch, Boomkey, MLSwitcher, FlicKey, Keyboard Language Fixer** | Mac | Разные | Lang Switcher: $7.99 / $11.99 в год [R]; Boomkey ≈ $14.99 [R] | Автопереключение / выделение + Shift×2 (Boomkey) [R] | Keyboard Language Fixer: «данные не собираются» (декларация App Store) [R] |
| **Punto для Mac** | Mac | 2.1.5 [R]; разработка заморожена с 2017 г. по Habr [R] — противоречие с comss | Бесплатно | — | Пользователи на Sequoia просят альтернативу [R] |
| **xneur / gxneur** | Linux (X11) | Debian 0.20.0 (миграция в testing 2024-05), FreeBSD-порт 2024-02-24 [R] | GPL [A] | X11-перехват [A] | — |
| **Retype** (расширение Raycast) | Mac | — | — | Использует раскладки macOS [R] | — |
| **Браузерные расширения** (KeyFix, FlipKey, Hebrew Layout Fixer…) | Chrome/Firefox | Много | В основном бесплатно [R] | Только в браузере; часть с «high-risk permissions» [R] | Риск: доступ к содержимому страниц [R] |

### 2.2 Карточки ключевых конкурентов

#### Punto Switcher (Яндекс) — «знаменитый, но уставший»
- **Платформы/статус.** Windows 4.5.0 (что нового: «правила пользователя стали приоритетными и независимыми от словарей», «исправлена замена буфера обмена») — дата не установлена [R: https://yandex.ru/soft/punto/win/release через поиск]. comss.ru указывает «обновлено 2024-04-27» (дата страницы, не обязательно релиза) [R: https://www.comss.ru/page.php?id=8856]. На Habr Q&A цитируется ответ «Punto Switcher работает в режиме фоновой поддержки, мы не занимаемся его активным развитием» — автор ответа не подтверждён [R: https://qna.habr.com/user/Dwellss/comments?page=6]. Mac-версия: по Habr — заморожена с 2017 г. [R: https://habr.com/ru/articles/1050394/], по comss — 2.1.5 [R].
- **Модель.** Бесплатно; страница загрузки предлагает «два продукта в одной установке: Яндекс Браузер с Алисой AI и расширение „Поиск Яндекса“» [R: https://yandex.ru/soft/punto/definition через поиск]; исторически — Яндекс.Бар [R: https://habr.com/ru/articles/152509/].
- **Как работает.** Автопереключение по словарям/правилам + ручные хоткеи (Break и др.), конвертация буфера (Win+C по комментариям) [R]. Требует глобального хука (антивирусная эвристика помечала `pshook.dll` как «подозрение на Keylogger» — единичный отчёт) [R: https://habr.com/ru/articles/1050394/ через поиск].
- **Приватность.** Функция «Дневник» записывает весь набор в файл, может быть запаролена [R: https://alice.yandex.ru/…, https://www.anti-malware.ru/threats/keyloggers]. Anti-malware.ru: злоумышленники ставят Punto как «легальный кейлоггер», т.к. антивирусы его не детектируют [R]. В 2010 г. — волна обвинений «шпионит» (v3.1.1) и ответ Яндекса: BITS передаёт тип ОС, версию и ID программы, «не передаёт персональных данных»; независимый тест увидел только запрос обновлений [R: https://yandex.ru/blog/punto/6286, https://yushchuk.livejournal.com/254576.html, https://www.seonews.ru/events/punto-switcher-shpionit-za-vami-/]. Habr-2026: «по многочисленным сообщениям» отправляет данные о конфигурации — технически не подтверждено [R].
- **Жалобы.** Ложные срабатывания на коротких словах и сокращениях → люди выключают автопереключение и переходят на ручное исправление [R: https://habr.com/en/articles/979382/comments]; «периодически терял буфер обмена» [R: https://habr.com/ru/articles/1050394/]; перестаёт работать после обновления Parallels Tools на Windows 11 [R: https://forum.parallels.com/threads/parallels-tools.358783/]; 96 настроек — перегруз (по сравнению с Caramba) [R: https://lifehacker.ru/caramba-switcher/].
- **Геополитика [R/A].** Яндекс-сервисы попали под санкции Украины (май 2017) [R: https://www.gazeta.ru/tech/2017/05/17_a_10678583.shtml]; прямого упоминания Punto в указе не найдено. Для украинских, балтийских и корпоративных пользователей «продукт Яндекса» — вероятный барьер [A].
- **Вердикт.** Сильный бренд и словари, слабые доверие, развитие и macOS. Главная «цель переключения» для KeySwitch [A].

#### Caramba Switcher — «простота и подписка»
- **Статус/цена.** Classic бесплатна; Pro для Windows: «3 days free, then $4.99 per year», лицензия на 3 компьютера; Allsoft: 499 ₽/год; Mac App Store: годовая подписка $6.99 с «Семейным доступом» [R: https://caramba-switcher.com/en/, https://allsoft.ru/software/vendors/ip-moskalyev-sergey-eduardovich/caramba-switcher-/4547544/, https://apps.apple.com/us/app/caramba-switcher/id1565826179?mt=12]. Wingetly публикует параметры тихой установки (Intune) — косвенный интерес к корпоративному развёртыванию [R: https://wingetly.io/apps/sergey-moskalev/caramba-switcher-pro].
- **Как работает.** «Ровно одна настройка — включить/выключить автопереключение», «на нейросетях»; Shift×2 — последнее слово; Alt+Shift — выделенное [R: https://lifehacker.ru/caramba-switcher/, https://caramba-switcher.com/en/]. Mac: только стандартные русская и английская раскладки; функции — автопереключение, исправление опечаток, пауза для отдельных приложений (код, игры) [R: App Store]. Языки на Windows — противоречиво (RU/EN/DE по старым обзорам; + UK/EL по SaaSHub) [R].
- **Приватность.** Политика конфиденциальности на caramba-switcher.com поиском не найдена [R]. Механизм автопереключения технически требует перехвата набора [A].
- **Жалобы.** Mac-отзывы против подписки при бесплатной Windows-версии (часть 2021 г.) [R]; «мало настроек» [R].
- **Вердикт.** Лучший UX-пример «ноль настроек», но закрытый, с перехватом клавиатуры и подпиской за базовую функцию. Конкурирует с KeySwitch за ту же аудиторию RU/EN [A].

#### Kright — «самый близкий по философии open-source конкурент»
- **Факты [V: https://raw.githubusercontent.com/walteryaron/kright/HEAD/README.md, …/CHANGELOG.md, …/Windows/README.md].** MIT; Win 1.1.1 (2026-08-14, подпись Authenticode через Azure Trusted Signing, публикация в Microsoft Store), Mac 1.1.0 (2026-07-18); подписанное автообновление. Хоткей Ctrl+Alt+K; конвертирует **последнее слово** с использованием реальных раскладок (`ToUnicodeEx`/`UCKeyTranslate`) — любая пара языков; запись через UI Automation, в cmd/PowerShell/Windows Terminal — SendInput + сохранение/восстановление буфера («не гарантирует восстановление»); автоисправление по Space/Tab (опц.); автопереход на латиницу в полях e-mail/URL/пароль; правила раскладки по приложению и по контакту (Teams/WhatsApp на Mac). Технически: `WH_KEYBOARD_LL` («passively watches the keys you type — never recording them»), на Mac — «Key Log» для прозрачности.
- **Важная находка для нас.** Kright пишет, что WhatsApp для Windows «renders its whole UI inside a WebView2 control that exposes nothing to UI Automation», подтверждено тестами на устройстве [V]. Это прямо касается риска X-13 в `synthesis.md` и матрицы `pa-app-matrix.md`.
- **Вердикт.** Сильный, свежий, бесплатный, подписанный, кроссплатформенный; основной фокус — иврит. Уязвим в части «хук = доверие» и отсутствия песочницы/макросов [A].

#### RuSwitcher — «open-source замена Punto на Mac, идёт на Windows»
- **Факты [V: https://raw.githubusercontent.com/rashn/RuSwitcher/HEAD/README.md, …/version.json, …/windows/README.md, …/windows/version.json].** MIT (имя и иконка — не под MIT, есть TRADEMARKS.md). Mac 3.5.0: триггер — одиночный/двойной модификатор (Option и др.), повторное нажатие — отмена; автоконверсия (бета, выкл.) по системному словарю с консервативными правилами (пропуск слов < 3 букв, URL, CamelCase, терминалов/IDE/менеджеров паролей; списки «никогда/всегда»); расширенный словарь ~250 КБ загружается опционально; 16 языков интерфейса; режим удалённого рабочего стола; флаг раскладки у курсора; звук раскладки. В 3.5.0 исправлены «потеря слова в браузерах и веб-чатах, Enter в Outlook, буфер обмена при конвертации выделенного, рост памяти». Windows 0.9.0 — первая бета: `WH_KEYBOARD_LL`, SendInput Unicode, `ISpellChecker`, проверка обновлений раз в день, подпись через SignPath Foundation.
- **Вердикт.** Наиболее зрелая модель «осторожного автопереключения» (precision over recall, learn-from-undo). Как и Kright — на хуке [A].

#### SimpleSwitcher — «power-user и прозрачная сборка»
- **Факты [V: https://raw.githubusercontent.com/Aegel5/SimpleSwitcher/HEAD/README.md].** C++23, бесплатно; с 6.10.2 — «Immutable» релизы, собираемые на GitHub Actions («облачная сборка гарантирует чистоту бинарных файлов»); исправление последнего текста и выделения; почти любые клавиши как триггер (CapsLock, F24-ремап, double/up); запуск процессов и вставка текста по хоткею (с правами самого SS); исключения программ через JSON; «Напоминалка»; известные проблемы с RDP (локальный хук перехватывает клавиши, уходящие на удалённую машину).
- **Вердикт.** Сигнал рынка: продвинутые пользователи ценят **проверяемую сборку** и хотят «команды по хоткею» — это ниша Macro Studio, но SS исполняет команды без песочницы [A].

#### EveryLang — «комбайн с облачным переводом»
- [R: https://habr.com/ru/articles/250233/, https://soware.ru/products/everylang]: переключение (ручное/авто), перевод через Google/Microsoft/Yandex, орфография, сниппеты, журнал набора, индикатор раскладки; freemium + бессрочная Pro; в реестре отечественного ПО РФ. Цена не найдена.
- **Вердикт.** Противоположность KeySwitch по приватности (облачный перевод, журнал). Интересен как показатель спроса на «сниппеты + переключатель» в одном продукте [A].

### 2.3 Наблюдения по механизмам (анализ)

| Подход | Кто | Плюсы | Минусы | KeySwitch |
|---|---|---|---|---|
| Глобальный хук + буфер последних нажатий | Punto, Caramba, Kright, RuSwitcher, SimpleSwitcher, Mahou | «Последнее слово» без выделения; автопереключение | Ощущение «кейлоггера», AV-эвристики, конфликты с RDP/играми/Sticky Keys, отказ от Input Monitoring на Mac | **Запрещено** (D-14, CLAUDE.md п.8) |
| Только выделение + буфер обмена | Keyboard Switch, Boomkey, LangSwitcher (частично) | Без хука; просто | Портит буфер; требует выделения | Только по профилю (W2), с метками исключения (D-13) |
| Авто-выделение через Accessibility/UIA | LangSwitcher «Greedy Line», Kright (запись UIA) | Без хука для чтения | Зависит от приложения; WebView2-чаты не отдают текст | Основной путь (D-03, FR-SCOPE) |
| Автопереключение «as you type» | Punto, Caramba, RuSwitcher (бета), Kright (опц.), MySwitcher | «Магия», не нужно жать клавишу | Ложные срабатывания — главная жалоба | Вне рамок v1 (NG-1) |

**Вывод [A].** Отсутствие хука — не только ограничение, но и единственное устойчивое отличие, которое конкуренты на хуке не могут скопировать без отказа от своих ключевых функций.

---

## 3. Смежные инструменты (важные для Macro Studio и хоткеев)

| Инструмент | Что важно для KeySwitch | Источник |
|---|---|---|
| **AutoHotkey** (v2; v1 не поддерживается) | Стандарт де-факто для «всё по хоткею» на Windows; GPL-2.0; официально признаёт проблему **ложных срабатываний антивирусов** и держит отдельную страницу о ней. Скрипты = полный доступ к системе. | [V] https://raw.githubusercontent.com/AutoHotkey/AutoHotkey/HEAD/README.md, license.txt |
| **Espanso** | Кроссплатформенный экспандер на Rust, GPL-3.0, «Privacy-first (100% local, no tracking)», но поддерживает **custom scripts и shell commands**, пакеты из hub, regex-триггеры, формы. Модель угроз: пакет из hub = исполняемый код. | [V] https://raw.githubusercontent.com/espanso/espanso/HEAD/README.md |
| **PhraseExpress** | Коммерческий лидер Windows-экспандеров (v16 к 20-летию); бесплатно для личного использования с напоминаниями о лицензии; Standard ≈ $99.95 / Pro $149.95 / Enterprise $249.95 (сторонний источник, цены не подтверждены). | [R] https://phraseexpress.com/shop/freeware/, https://us.fitgap.com/products/phrase-express, https://betanews.com/article/phraseexpress-16-unveils-new-look-and-trigger-system-to-celebrate-its-20th-anniversary |
| **Beeftext** | Open source (MIT [R]), 16.0 — 2022-12-31, по стороннему блогу в режиме поддержки с 2023 г. | [R] https://beeftext.org/, https://adaptiva.com/patch-library/1000000778_patch_for_beeftext_x64 |
| **PowerToys** | 30+ утилит; готовится v0.103 [V]. **0.94 (2025-09-02): поиск по настройкам (Ctrl+F) и обнаружение конфликтов хоткеев** с плиткой на дашборде [R: devblogs.microsoft.com]. Advanced Paste: с 0.96 — локальные модели (Foundry Local, Ollama), в превью 0.101 (2026-08-10) — Phi Silica на NPU [R]. Keyboard Manager — ремап клавиш. | [V] https://raw.githubusercontent.com/microsoft/PowerToys/HEAD/README.md; [R] https://devblogs.microsoft.com/commandline/powertoys-0-94-is-here-settings-search-shortcut-conflict-detection-and-more/, https://pureinfotech.com/powertoys-phi-silica-ai-advanced-paste/ |
| **Raycast для Windows** | v2.10 — 2026-10-28; сниппеты, хоткеи команд, **Game Mode (отключение хоткеев в играх)** в v0.41, автоочистка «устаревших» хоткеев, Script Commands (v0.45, 2026-02-13), AI Inline Replace. | [R] https://www.raycast.com/changelog/windows, https://alternativeto.net/news/2026/6/raycast-launches-ai-chat-and-inline-replace-features-for-windows-and-mac/ |
| **Ditto / Win+V** | Менеджеры буфера сохраняют всё, что проходит через буфер, — причина D-13. | [A] (знание аналитика; не перепроверено в этой сессии) |
| **macOS: Input Source Pro, LockIME, Kawa** | Переключение источника ввода по приложению и **по сайту** (Input Source Pro, GPL-3.0, 2.11.0). Для будущей macOS-версии KeySwitch — ориентир «раскладка по контексту». | [R] https://github.com/runjuu/InputSourcePro, https://inputsource.pro/changelog |

**Выводы для Macro Studio [A].**
1. Пользователи уже «склеивают» переключатель + экспандер + AHK. Ни один из них не даёт **безопасного** исполнения пользовательского кода: всё работает с правами пользователя. Песочница Wasmtime + AppContainer (D-08) — уникальное свойство, его надо *показывать* (см. MI-05, MI-06).
2. Обратная сторона: один слот и белый список клавиш (D-07) несопоставимы по мощности с AHK/Espanso. Позиционировать Macro Studio не как «замену AHK», а как «безопасные текстовые трансформации по хоткею» (регистр, обёртки, шаблоны, очистка форматирования, транслит-по-желанию и т. п.).
3. Обнаружение конфликтов хоткеев и поиск по настройкам — уже «гигиена» Windows-утилит после PowerToys 0.94; отсутствие будет заметно [A].

---

## 4. Матрица функций: KeySwitch v1 против рынка

Обозначения: ✅ есть · ◐ частично/опционально · ❌ нет · ? не установлено. Данные конкурентов — по §2 (метки V/R там же). KeySwitch v1 — по `docs/research/pa-prd.md` §3.1 (план, не реализовано).

| Функция | KeySwitch v1 (план) | Punto | Caramba | Kright | RuSwitcher | SimpleSwitcher | EveryLang |
|---|---|---|---|---|---|---|---|
| RU↔EN по хоткею | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Без выделения («последнее слово/строка») | ◐ через UIA (поле/строка, D-03) | ✅ хук | ✅ хук | ✅ хук | ✅ хук | ✅ хук | ✅ хук |
| Автопереключение при наборе | ❌ (NG-1) | ✅ | ✅ | ◐ | ◐ бета | ? | ✅ |
| Без глобального перехвата клавиатуры | ✅ | ❌ | ❌ [A] | ❌ | ❌ | ❌ | ❌ [A] |
| Нет сетевых вызовов вообще | ✅ (нет даже обновлений, NG-7) | ❌ [R] | ? | ◐ (автообновление) | ◐ (проверка обновлений, словарь) | ◐ [A] | ❌ (облачный перевод) |
| Диалог для смешанного текста | ✅ | ? | ? | ❌ [A] | ◐ «smart per-word keep» | ? | ? |
| Отказ при риске (пароль, повышенные права, встроенные объекты) | ✅ (D-05, FR-SCOPE-6) | ? | ◐ пауза по приложениям | ◐ пароли не читаются | ◐ исключения приложений | ◐ исключения JSON | ? |
| Защита буфера (метки исключения из истории/облака) | ✅ (D-13) | ? | ? | ❌ (восстановление без гарантий) | ◐ (буфер редко) | ◐ очистка | ? |
| Ctrl+Z возвращает исходный текст | ✅ (D-12, возможно 2 шага) | ? | ? | ? | ◐ повторное нажатие = отмена | ? | ? |
| Повторное нажатие = отмена | ❌ (NG-12) | ? | ? | ? | ✅ | ? | ? |
| Пользовательские макросы | ✅ 1 слот, JS/Python в песочнице | ❌ | ❌ | ❌ | ❌ | ◐ команды без песочницы | ◐ сниппеты |
| Проверка конфликтов хоткеев | ✅ (ТЗ §10) | ? | ❌ | ? | ? | ❌ | ? |
| Диагностика без текста пользователя | ✅ | ❌ (Дневник) | ? | ◐ Key Log (Mac) | ? | ? | ❌ (журнал набора) |
| Языки помимо RU/EN | ❌ v1 (задел D-15) | ✅ | ◐ | ✅ любые установленные | ✅ любые установленные | ✅ | ✅ |
| macOS | ❌ v1 | ◐ заморожен | ✅ | ✅ | ✅ | ❌ | ❌ |
| Подписанная сборка | ? (D-11, решение до релиза) | ✅ [A] | ✅ [A] | ✅ | ✅ (SignPath) | ◐ иммутабельные CI-релизы | ? |
| Open source | ✅ (публичный репо, D-09) | ❌ | ❌ | ✅ | ✅ | ✅ | ❌ |
| Интерфейс RU/EN, тёмная тема | ✅ | ✅ RU [A] | ? | EN [A] | ✅ 16 языков | ✅ RU | ✅ RU [A] |
| Цена | бесплатно (1 слот) | бесплатно | free / $4.99–6.99 в год | бесплатно | бесплатно | бесплатно | freemium |

**Анализ разрывов [A].**
- **Мы проигрываем** в: «последнее слово без выделения в любом приложении», автопереключении, числе языков, macOS. Это следствие принципов — компенсировать качеством отказа и UIA-путём, а не копированием.
- **Мы выигрываем** (если реализуем): отсутствие хука, отсутствие сети, безопасность буфера, песочница макросов, диагностика без текста, честная матрица поддерживаемых приложений.
- **Нейтрально/«гигиена»:** подпись кода (у ближайших OSS-конкурентов уже есть), поиск по настройкам, конфликты хоткеев.
- **Риск рынка:** аргумент «бесплатно + open source + приватно» уже заняли Kright и RuSwitcher. Отличие KeySwitch должно формулироваться через *проверяемость* и *отсутствие хука*, а не через «бесплатно».

---

## 5. Предварительные гипотезы позиционирования и модели монетизации

> **ПРЕДВАРИТЕЛЬНО.** Это гипотезы для обсуждения, а не рекомендация. Полный анализ монетизации — отдельная задача AGENT 10 перед коммерческим запуском (GATE 9). Монетизация в v1 — вне рамок (NG-6).

### 5.1 Модели, встречающиеся на рынке
| Модель | Примеры | Наблюдения |
|---|---|---|
| Бесплатно + бандлинг/экосистема | Punto Switcher (Яндекс Браузер/расширения в установщике) [R] | Подрывает доверие; для KeySwitch неприемлемо (нет сети, нет стороннего ПО) [A] |
| Freemium + подписка | Caramba Pro $4.99/год (Win), $6.99/год (Mac); Lang Switcher $11.99/год [R] | Пользователи Mac жалуются на подписку за базовую функцию [R] |
| Freemium + разовая покупка | EveryLang Pro (бессрочно) [R]; InputSwitcher Pro [R]; Boomkey ≈ $14.99 [R] | Типично для утилит; принимается лучше подписки [A] |
| Бесплатно для личного, платно для бизнеса | PhraseExpress (бизнес — $99.95+ разово, цифры [R]); MySwitcher — бесплатно и для работы [R] | Классическая модель экспандеров |
| Бесплатно OSS + донаты | Kright, RuSwitcher, SimpleSwitcher, LangSwitcher [V] | Задаёт ценовой «пол» = 0 для базовой конвертации |
| «1 бесплатный слот макроса» (ТЗ §11) | Аналогов у переключателей не найдено [A] | Отделяет базовую функцию (бесплатно) от продвинутой (макросы) |

### 5.2 Гипотезы позиционирования (для проверки)
- **H-1 (основная).** *«Исправитель раскладки, который не слушает вашу клавиатуру»*: единственный (из найденных) инструмент без глобального перехвата клавиатуры и без сетевых вызовов, с проверяемыми доказательствами (open source, воспроизводимая сборка, отчёт о приватности на устройстве). Аудитория: люди с недоверием к Punto, корпоративные/регулируемые среды, разработчики. Метрика проверки: доля интервью/отзывов, где «нет кейлоггера» — главный мотив выбора [A].
- **H-2.** *«Лучше отказаться, чем испортить»*: позиционирование через надёжность — не трогает пароли, не ломает буфер, не отправляет Enter в чат, Ctrl+Z работает. Опирается на главные жалобы (ложные срабатывания, потеря буфера — см. `2026-11-10-user-needs.md`) [A].
- **H-3.** *«Безопасные макросы по хоткею»*: Macro Studio как «AutoHotkey без страха» для текстовых трансформаций; песочница — продаваемое свойство. Риск: мощности одного слота с белым списком может не хватить, чтобы это стало причиной выбора [A].
- **H-4 (B2B, позже).** Корпоративная версия: MSI/групповые политики, фиксированный список исключений, отчёт для ИБ «без хуков, без сети, без инъекций». Сигнал спроса: Caramba публикует параметры тихой установки для Intune [R]; Punto воспринимается как кейлоггер [R]. Доказательств реального спроса со стороны ИБ **не найдено** — гипотеза слабая.

### 5.3 Предварительные соображения по монетизации [A]
1. Базовая конвертация должна оставаться бесплатной: OSS-конкуренты задали цену 0.
2. Платное — то, чего нет у OSS: дополнительные слоты Macro Studio, будущие языки/раскладки как «пакеты», корпоративные функции (политики, MSI, поддержка), macOS-версия.
3. Подписка за базовую функцию вызывает негатив (Caramba Mac) [R] → для «Pro» предпочтительнее разовая покупка или «разово + платные мажорные обновления»; подписку — только для B2B.
4. При публичном репозитории (D-09) лицензия кода определяет, можно ли вообще продавать «закрытые» функции; это решение нужно до проектирования монетизации (MI-19).

---

## 6. Что не удалось проверить (и как проверить)

| Пробел | Почему | Как закрыть |
|---|---|---|
| Даты релизов Punto 4.5.0, Caramba, EveryLang, Mahou, MySwitcher | Официальные сайты недоступны из сессии | Разрешить `yandex.ru`, `caramba-switcher.com`, `everylang.net`, `gitea.com` |
| Текущая политика приватности Caramba, EveryLang, Punto | То же | То же + анализ сетевого трафика на Windows (вне облака) |
| Механизм Caramba (хук или иное) | Закрытый код | Анализ импортов бинарника на Windows-машине владельца (только с его согласия) |
| Реальная частота жалоб (отзывы App Store, Reddit, 4PDA, Microsoft Store) | Поиск возвращает мало прямых отзывов; сайты не открываются | Разрешить `apps.apple.com`, `reddit.com`, `4pda.to`, `apps.microsoft.com` |
| Товарный знак «KeySwitch» | Нет доступа к реестрам | Проверка в WIPO Global Brand Database / Роспатент / EUIPO — задача владельца/юриста |
| Issues GitHub конкурентов (что ломается в каких приложениях) | `github.com` 403 | Разрешить `github.com`/`api.github.com` |

## 7. Источники (обращение 2026-11-10)

Первичные, прочитаны напрямую [V]:
- https://raw.githubusercontent.com/walteryaron/kright/HEAD/README.md ; …/CHANGELOG.md ; …/Windows/README.md
- https://raw.githubusercontent.com/rashn/RuSwitcher/HEAD/README.md ; …/version.json ; …/windows/README.md ; …/windows/version.json
- https://raw.githubusercontent.com/Aegel5/SimpleSwitcher/HEAD/README.md
- https://raw.githubusercontent.com/reg2005/langSwitcher/HEAD/README.md
- https://raw.githubusercontent.com/TolikPylypchuk/KeyboardSwitch/HEAD/README.md
- https://raw.githubusercontent.com/espanso/espanso/HEAD/README.md ; …/LICENSE
- https://raw.githubusercontent.com/microsoft/PowerToys/HEAD/README.md
- https://raw.githubusercontent.com/AutoHotkey/AutoHotkey/HEAD/README.md ; …/license.txt

Вторичные / через поисковую выдачу [R]:
- Punto: https://yandex.ru/soft/punto/win/release · https://www.comss.ru/page.php?id=8856 · https://habr.com/ru/articles/1050394/ · https://habr.com/en/articles/979382/comments · https://habr.com/ru/articles/152509/ · https://qna.habr.com/user/Dwellss/comments?page=6 · https://yandex.ru/blog/punto/6286 · https://yushchuk.livejournal.com/254576.html · https://www.seonews.ru/events/punto-switcher-shpionit-za-vami-/ · https://www.anti-malware.ru/threats/keyloggers · https://forum.parallels.com/threads/parallels-tools.358783/ · https://qna.habr.com/q/1303436 · https://vc.ru/40834-chto-sluchilos-s-punto-switcher · https://www.gazeta.ru/tech/2017/05/17_a_10678583.shtml
- Caramba: https://caramba-switcher.com/en/ · https://caramba-switcher.com/en/update/ · https://allsoft.ru/software/vendors/ip-moskalyev-sergey-eduardovich/caramba-switcher-/4547544/ · https://apps.apple.com/us/app/caramba-switcher/id1565826179?mt=12 · https://wingetly.io/apps/sergey-moskalev/caramba-switcher-pro · https://lifehacker.ru/caramba-switcher/ · https://zoom.cnews.ru/news/item/410291 · https://www.saashub.com/compare-caramba-switcher-vs-everylang
- Прочие: https://alternativeto.net/software/punto-switcher · https://alternativeto.net/software/myswitcher/about/ · https://alternativeto.net/software/simpleswitcher · https://gitea.com/BladeMight/Mahou · https://habr.com/ru/articles/250233/ · https://soware.ru/products/everylang · https://cyclowiki.org/wiki/Orfo_Switcher · https://habr.com/en/articles/110359 · https://www.ixbt.com/soft/arum-switcher.shtml · https://www.npackd.org/p/com.keyboardninja.KeyboardNinja/2.1 · https://rarst.net/software/langover · https://alternativeto.net/software/kavanati/about/ · https://apps.apple.com/us/app/lang-switcher/id1597566195 · https://setapp.com/de/apps/boomkey · https://apps.apple.com/app/id6760297342 · https://freshports.org/deskutils/gxneur · https://tracker.debian.org/xneur · https://www.raycast.com/pavel_shut/retype-fix-wrong-keyboard-layout-text · https://lifehacker.ru/kirgudu/
- Конфликт имени: https://www.producthunt.com/products/keyswitch · https://hunted.space/product/keyswitch · https://lib.rs/crates/keyswitch
- Смежные: https://devblogs.microsoft.com/commandline/powertoys-0-94-is-here-settings-search-shortcut-conflict-detection-and-more/ · https://pureinfotech.com/powertoys-phi-silica-ai-advanced-paste/ · https://www.windowscentral.com/software-apps/powertoys-breaks-free-from-openai-limits-advanced-paste-now-works-with-multiple-ai-models · https://www.raycast.com/changelog/windows · https://phraseexpress.com/shop/freeware/ · https://us.fitgap.com/products/phrase-express · https://beeftext.org/ · https://github.com/runjuu/InputSourcePro · https://inputsource.pro/changelog
