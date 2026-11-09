# KeySwitch — дизайн-система (v0.1, исследование фазы 1)

Статус: **черновик для GATE 3** · Автор: UI/UX Designer (Agent 04) · Концепция: `design-concept.md` · Прототип: `design-prototype.html` (использует эти же токены).

Цель документа — дать frontend-dev точные значения, которые **1:1** переносятся в CSS custom properties и в `@theme` Tailwind v4. Итоговая версия после GATE 3 переезжает в `docs/06-design-system.md`.

## 1. Как токены ложатся на Tailwind v4

- Все токены — CSS-переменные с префиксом `--ks-`. Семантические цвета задаются на `:root` (тёмная тема — по умолчанию) и переопределяются на `[data-theme="light"]`. Тема «Системная» = атрибут `data-theme="system"` + `@media (prefers-color-scheme: light)`.
- В `@theme` Tailwind регистрируются **ссылки** на семантические переменные, чтобы утилиты (`bg-surface`, `text-fg-muted`, `border-strong`, `rounded-md`, `shadow-2`) автоматически следовали теме.
- Префикс в Tailwind-namespace: `--color-*`, `--font-*`, `--text-*`, `--spacing`, `--radius-*`, `--shadow-*`, `--ease-*` — стандартные namespace Tailwind v4.

```css
/* app/src/index.css — эскиз для frontend-dev */
@import "tailwindcss";

@theme {
  --color-*: initial;                     /* отключить встроенную палитру — только наши токены */
  --color-canvas:   var(--ks-bg-canvas);
  --color-surface:  var(--ks-bg-surface);
  --color-raised:   var(--ks-bg-raised);
  --color-overlay:  var(--ks-bg-overlay);
  --color-hover:    var(--ks-bg-hover);
  --color-selected: var(--ks-bg-selected);
  --color-fg:            var(--ks-fg-primary);
  --color-fg-secondary:  var(--ks-fg-secondary);
  --color-fg-muted:      var(--ks-fg-muted);
  --color-fg-disabled:   var(--ks-fg-disabled);
  --color-border-subtle:  var(--ks-border-subtle);
  --color-border:         var(--ks-border-default);
  --color-border-strong:  var(--ks-border-strong);
  --color-accent:         var(--ks-accent);
  --color-accent-hover:   var(--ks-accent-hover);
  --color-accent-pressed: var(--ks-accent-pressed);
  --color-on-accent:      var(--ks-on-accent);
  --color-accent-fg:      var(--ks-accent-fg);
  --color-focus:          var(--ks-focus-ring);
  --color-success: var(--ks-success-fg);  --color-success-bg: var(--ks-success-bg);
  --color-warning: var(--ks-warning-fg);  --color-warning-bg: var(--ks-warning-bg);
  --color-danger:  var(--ks-danger-fg);   --color-danger-bg:  var(--ks-danger-bg);
  --color-info:    var(--ks-info-fg);     --color-info-bg:    var(--ks-info-bg);
  --color-danger-solid: var(--ks-danger-solid); --color-on-danger: var(--ks-on-danger);

  --font-sans: "Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI", system-ui, sans-serif;
  --font-display: "Segoe UI Variable Display", "Segoe UI Variable", "Segoe UI", system-ui, sans-serif;
  --font-mono: "Cascadia Mono", "Cascadia Code", Consolas, "Courier New", monospace;
  --spacing: 4px;                         /* p-1 = 4px, p-2 = 8px … */
  --radius-sm: 4px; --radius-md: 6px; --radius-lg: 8px; --radius-xl: 12px;
  --shadow-1: var(--ks-shadow-1); --shadow-2: var(--ks-shadow-2); --shadow-3: var(--ks-shadow-3);
  --ease-standard: cubic-bezier(0.2, 0, 0, 1);
  --ease-exit: cubic-bezier(0.3, 0, 1, 1);
}

:root { color-scheme: dark; /* + все --ks-* тёмной темы, §2.2 */ }
:root[data-theme="light"] { color-scheme: light; /* + --ks-* светлой темы, §2.3 */ }
@media (prefers-color-scheme: light) {
  :root[data-theme="system"] { color-scheme: light; /* те же значения, что light */ }
}
@custom-variant dark (&:where([data-theme="dark"], [data-theme="dark"] *));
```

> Вариант без дублирования значений (так сделано в прототипе): JS выставляет `data-sys="light|dark"` по `matchMedia('(prefers-color-scheme: light)')` с подпиской на `change`, а светлые токены задаются один раз селектором `:root[data-theme="light"], :root[data-theme="system"][data-sys="light"]`. Результат тот же: «Системная» = `prefers-color-scheme`. **[ARCH]**: проверить, что WebView2 отдаёт актуальный `prefers-color-scheme` при смене темы Windows без перезапуска; иначе — событие темы от Tauri.

> Тёмная тема — значение по умолчанию на `:root` (и до загрузки настроек), чтобы не было вспышки белого при запуске (FOUC). Настройку темы фронтенд выставляет атрибутом `data-theme` на `<html>` до первого рендера.

## 2. Цвет

### 2.1. Нейтральная шкала (примитивы)

Холодно-серая шкала, общая основа для обеих тем. Семантические токены (§2.2–2.3) ссылаются на эти значения или на близкие производные; в коде используются **только семантические** токены.

| Токен-примитив | Hex | Где используется |
|---|---|---|
| `--ks-neutral-0` | `#FFFFFF` | light: surface/raised/overlay |
| `--ks-neutral-25` | `#F4F5F7` | light: canvas |
| `--ks-neutral-50` | `#F2F4F7` | light: текущая строка редактора |
| `--ks-neutral-100` | `#EBEDF0` | light: hover |
| `--ks-neutral-150` | `#E4E7EB` | light: border-subtle |
| `--ks-neutral-200` | `#CDD2D9` | light: border-default |
| `--ks-neutral-400` | `#A0A7B1` | light: fg-disabled |
| `--ks-neutral-500` | `#7D8796` | light: border-strong |
| `--ks-neutral-550` | `#6C7686` | dark: border-strong |
| `--ks-neutral-600` | `#5B6472` | light: fg-muted |
| `--ks-neutral-700` | `#454D5A` | light: fg-secondary |
| `--ks-neutral-750` | `#353C47` | dark: border-default |
| `--ks-neutral-800` | `#2A303A` | dark: hover |
| `--ks-neutral-850` | `#232831` | dark: overlay |
| `--ks-neutral-875` | `#1C2027` | dark: raised |
| `--ks-neutral-900` | `#15181D` | dark: surface |
| `--ks-neutral-950` | `#0E1013` | dark: canvas |
| `--ks-neutral-975` | `#16191E` | light: fg-primary |

### 2.2. Семантические токены — тёмная тема (`:root`, `[data-theme="dark"]`)

| CSS-переменная | Tailwind (`@theme`) | Hex | Назначение |
|---|---|---|---|
| `--ks-bg-canvas` | `bg-canvas` | `#0E1013` | фон окна, редактор кода |
| `--ks-bg-surface` | `bg-surface` | `#15181D` | сайдбар, панели, строка состояния |
| `--ks-bg-raised` | `bg-raised` | `#1C2027` | карточки, поля ввода |
| `--ks-bg-overlay` | `bg-overlay` | `#232831` | меню, поповеры, модалки, окно выбора языка |
| `--ks-bg-hover` | `bg-hover` | `#2A303A` | hover строк/пунктов/ghost-кнопок |
| `--ks-bg-selected` | `bg-selected` | `#1E2B44` | активный пункт навигации, выбранная строка |
| `--ks-border-subtle` | `border-border-subtle` | `#262B33` | разделители (декоративные) |
| `--ks-border-default` | `border-border` | `#353C47` | рамки карточек (декоративные) |
| `--ks-border-strong` | `border-border-strong` | `#6C7686` | рамки полей ввода, дорожка тумблера выкл. (≥ 3:1) |
| `--ks-fg-primary` | `text-fg` | `#E7EAEE` | основной текст |
| `--ks-fg-secondary` | `text-fg-secondary` | `#AEB6C2` | подписи, второстепенный текст |
| `--ks-fg-muted` | `text-fg-muted` | `#949DAA` | подсказки, плейсхолдеры, метаданные |
| `--ks-fg-disabled` | `text-fg-disabled` | `#5A626E` | отключённые элементы (исключение WCAG) |
| `--ks-accent` | `bg-accent` | `#2D6AD6` | primary-кнопка, тумблер вкл., выбранный сегмент |
| `--ks-accent-hover` | `bg-accent-hover` | `#3572DD` | hover primary |
| `--ks-accent-pressed` | `bg-accent-pressed` | `#255DC0` | active primary |
| `--ks-on-accent` | `text-on-accent` | `#FFFFFF` | текст на акценте |
| `--ks-accent-fg` | `text-accent-fg` | `#7FB0FF` | ссылки, акцентный текст, иконка активного пункта |
| `--ks-focus-ring` | `outline-focus` | `#7FB0FF` | кольцо фокуса |
| `--ks-success-fg` / `-bg` | `text-success` / `bg-success-bg` | `#4CC38A` / `#13261C` | «Зарегистрировано», «Выполнено» |
| `--ks-warning-fg` / `-bg` | `text-warning` / `bg-warning-bg` | `#E5A93D` / `#2B2213` | «Раскладка не найдена», пауза |
| `--ks-danger-fg` / `-bg` | `text-danger` / `bg-danger-bg` | `#FF7B72` / `#2E1716` | конфликт, ошибка макроса |
| `--ks-info-fg` / `-bg` | `text-info` / `bg-info-bg` | `#6CB6FF` / `#14233A` | баннер приватности, справка |
| `--ks-danger-solid` | `bg-danger-solid` | `#D23F3F` | кнопка «Очистить журнал» в модалке |
| `--ks-on-danger` | `text-on-danger` | `#FFFFFF` | текст на danger-solid |
| `--ks-bg-editor` | `bg-editor` | `#0E1013` | фон редактора |
| `--ks-bg-editor-line` | `bg-editor-line` | `#171B21` | текущая строка |
| `--ks-syn-keyword` | — | `#C59BFF` | ключевые слова |
| `--ks-syn-string` | — | `#8FD18F` | строки |
| `--ks-syn-number` | — | `#F2B36F` | числа, литералы |
| `--ks-syn-comment` | — | `#8C95A3` | комментарии |
| `--ks-syn-function` | — | `#7FB0FF` | имена функций, вызовы API |

### 2.3. Семантические токены — светлая тема (`[data-theme="light"]`)

| CSS-переменная | Hex | | CSS-переменная | Hex |
|---|---|---|---|---|
| `--ks-bg-canvas` | `#F4F5F7` | | `--ks-accent` | `#1F5FCC` |
| `--ks-bg-surface` | `#FFFFFF` | | `--ks-accent-hover` | `#1A53B5` |
| `--ks-bg-raised` | `#FFFFFF` | | `--ks-accent-pressed` | `#16479C` |
| `--ks-bg-overlay` | `#FFFFFF` | | `--ks-on-accent` | `#FFFFFF` |
| `--ks-bg-hover` | `#EBEDF0` | | `--ks-accent-fg` | `#1A56BD` |
| `--ks-bg-selected` | `#E3ECFD` | | `--ks-focus-ring` | `#1F5FCC` |
| `--ks-border-subtle` | `#E4E7EB` | | `--ks-success-fg` / `-bg` | `#18794A` / `#E6F4EC` |
| `--ks-border-default` | `#CDD2D9` | | `--ks-warning-fg` / `-bg` | `#8A5300` / `#FBF1DE` |
| `--ks-border-strong` | `#7D8796` | | `--ks-danger-fg` / `-bg` | `#BF2A2A` / `#FCE9E8` |
| `--ks-fg-primary` | `#16191E` | | `--ks-info-fg` / `-bg` | `#1A64A8` / `#E5F0FA` |
| `--ks-fg-secondary` | `#454D5A` | | `--ks-danger-solid` / `--ks-on-danger` | `#C42B2B` / `#FFFFFF` |
| `--ks-fg-muted` | `#5B6472` | | `--ks-bg-editor` / `-line` | `#FFFFFF` / `#F2F4F7` |
| `--ks-fg-disabled` | `#A0A7B1` | | `--ks-syn-keyword` / `-string` | `#7A3DC7` / `#2D7A2D` |
| | | | `--ks-syn-number` / `-comment` / `-function` | `#A3520A` / `#5F6876` / `#1A56BD` |

В светлой теме `surface` = `raised` = `overlay` = белый; иерархия создаётся рамкой `border-default` и тенью (`elevation`), а canvas чуть темнее.

### 2.4. Проверка контраста (WCAG 2.x)

Метод: относительная яркость по WCAG 2.x (sRGB-линеаризация, порог 0.04045), `(L1+0.05)/(L2+0.05)`. Требования: обычный текст ≥ 4.5:1 (1.4.3), нетекстовые элементы UI — границы полей, фокус-кольцо, тумблеры, иконки-статусы ≥ 3:1 (1.4.11). Отключённые элементы освобождены от требования (1.4.3, исключение «inactive UI component»), декоративные разделители — тоже; они приведены для информации («—»).

Скрипт запущен 2026-11-09 (Python 3, без зависимостей; исходник — в Приложении A). **Итоговый результат: 0 FAIL из 96 нормируемых пар.**

**История итераций (честно):** первый прогон палитры дал 3 FAIL, значения исправлены, второй прогон — 0 FAIL:

| Пара (тёмная тема) | Было | Контраст | Стало | Контраст |
|---|---|---|---|---|
| `on-accent` на `accent` | `#3B7BEB` | 4.02:1 **FAIL** | `#2D6AD6` | 5.07:1 PASS |
| `on-accent` на `accent-hover` | `#4F8BF2` | 3.33:1 **FAIL** | `#3572DD` | 4.56:1 PASS |
| `fg-muted` на `bg-hover` | `#8C95A3` | 4.39:1 **FAIL** | `#949DAA` | 4.84:1 PASS |

**Пограничные пары (проходят с малым запасом — не затемнять/не осветлять без перепроверки):** `on-accent`/`accent-hover` dark 4.56:1; `on-danger`/`danger-solid` dark 4.64:1; `syn-string`/`bg-editor-line` light 4.85:1; `border-strong`/`bg-overlay` dark 3.22:1; `accent`/`bg-raised` dark 3.22:1.

**Правила, вытекающие из расчёта:**
- `border-default`/`border-subtle` (1.5–1.6:1) — **только декоративные**. Границы интерактивных полей, чекбоксов, дорожки выключенного тумблера — `border-strong`.
- Акцентная заливка в тёмной теме (`#2D6AD6`) — допустима как нетекстовый индикатор (3.2–3.5:1) и как фон белого текста; **как цвет текста на тёмном фоне не использовать** — для текста есть `accent-fg`.
- Семантические `*-fg` проверены на своём `*-bg`, на `surface` и на `raised`; на `hover`/`selected` не проверялись — не использовать там без перепроверки.

Полный вывод скрипта:
#### Тёмная тема

| Передний план | Фон | Контраст | Требование | Итог |
|---|---|---|---|---|
| `fg-primary` #E7EAEE | `bg-canvas` #0E1013 | 15.79:1 | text ≥ 4.5 | PASS |
| `fg-primary` #E7EAEE | `bg-surface` #15181D | 14.74:1 | text ≥ 4.5 | PASS |
| `fg-primary` #E7EAEE | `bg-raised` #1C2027 | 13.54:1 | text ≥ 4.5 | PASS |
| `fg-primary` #E7EAEE | `bg-overlay` #232831 | 12.26:1 | text ≥ 4.5 | PASS |
| `fg-primary` #E7EAEE | `bg-hover` #2A303A | 11.00:1 | text ≥ 4.5 | PASS |
| `fg-primary` #E7EAEE | `bg-selected` #1E2B44 | 11.72:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #AEB6C2 | `bg-canvas` #0E1013 | 9.32:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #AEB6C2 | `bg-surface` #15181D | 8.70:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #AEB6C2 | `bg-raised` #1C2027 | 7.99:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #AEB6C2 | `bg-overlay` #232831 | 7.24:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #AEB6C2 | `bg-hover` #2A303A | 6.49:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #AEB6C2 | `bg-selected` #1E2B44 | 6.92:1 | text ≥ 4.5 | PASS |
| `fg-muted` #949DAA | `bg-canvas` #0E1013 | 6.95:1 | text ≥ 4.5 | PASS |
| `fg-muted` #949DAA | `bg-surface` #15181D | 6.49:1 | text ≥ 4.5 | PASS |
| `fg-muted` #949DAA | `bg-raised` #1C2027 | 5.96:1 | text ≥ 4.5 | PASS |
| `fg-muted` #949DAA | `bg-overlay` #232831 | 5.40:1 | text ≥ 4.5 | PASS |
| `fg-muted` #949DAA | `bg-hover` #2A303A | 4.84:1 | text ≥ 4.5 | PASS |
| `fg-muted` #949DAA | `bg-selected` #1E2B44 | 5.16:1 | text ≥ 4.5 | PASS |
| `fg-disabled` #5A626E | `bg-raised` #1C2027 | 2.65:1 | disabled (exempt, info)  | — |
| `on-accent` #FFFFFF | `accent` #2D6AD6 | 5.07:1 | text ≥ 4.5 | PASS |
| `on-accent` #FFFFFF | `accent-hover` #3572DD | 4.56:1 | text ≥ 4.5 | PASS |
| `on-accent` #FFFFFF | `accent-pressed` #255DC0 | 6.17:1 | text ≥ 4.5 | PASS |
| `on-danger` #FFFFFF | `danger-solid` #D23F3F | 4.64:1 | text ≥ 4.5 | PASS |
| `accent-fg` #7FB0FF | `bg-canvas` #0E1013 | 8.67:1 | text ≥ 4.5 | PASS |
| `accent-fg` #7FB0FF | `bg-surface` #15181D | 8.10:1 | text ≥ 4.5 | PASS |
| `accent-fg` #7FB0FF | `bg-raised` #1C2027 | 7.44:1 | text ≥ 4.5 | PASS |
| `accent-fg` #7FB0FF | `bg-selected` #1E2B44 | 6.44:1 | text ≥ 4.5 | PASS |
| `success-fg` #4CC38A | `success-bg` #13261C | 7.17:1 | text ≥ 4.5 | PASS |
| `success-fg` #4CC38A | `bg-surface` #15181D | 8.03:1 | text ≥ 4.5 | PASS |
| `success-fg` #4CC38A | `bg-raised` #1C2027 | 7.38:1 | text ≥ 4.5 | PASS |
| `warning-fg` #E5A93D | `warning-bg` #2B2213 | 7.51:1 | text ≥ 4.5 | PASS |
| `warning-fg` #E5A93D | `bg-surface` #15181D | 8.54:1 | text ≥ 4.5 | PASS |
| `warning-fg` #E5A93D | `bg-raised` #1C2027 | 7.84:1 | text ≥ 4.5 | PASS |
| `danger-fg` #FF7B72 | `danger-bg` #2E1716 | 6.66:1 | text ≥ 4.5 | PASS |
| `danger-fg` #FF7B72 | `bg-surface` #15181D | 7.06:1 | text ≥ 4.5 | PASS |
| `danger-fg` #FF7B72 | `bg-raised` #1C2027 | 6.48:1 | text ≥ 4.5 | PASS |
| `info-fg` #6CB6FF | `info-bg` #14233A | 7.34:1 | text ≥ 4.5 | PASS |
| `info-fg` #6CB6FF | `bg-surface` #15181D | 8.28:1 | text ≥ 4.5 | PASS |
| `info-fg` #6CB6FF | `bg-raised` #1C2027 | 7.60:1 | text ≥ 4.5 | PASS |
| `focus-ring` #7FB0FF | `bg-canvas` #0E1013 | 8.67:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `border-strong` #6C7686 | `bg-canvas` #0E1013 | 4.15:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `focus-ring` #7FB0FF | `bg-surface` #15181D | 8.10:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `border-strong` #6C7686 | `bg-surface` #15181D | 3.87:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `focus-ring` #7FB0FF | `bg-raised` #1C2027 | 7.44:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `border-strong` #6C7686 | `bg-raised` #1C2027 | 3.56:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `focus-ring` #7FB0FF | `bg-overlay` #232831 | 6.73:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `border-strong` #6C7686 | `bg-overlay` #232831 | 3.22:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `accent` #2D6AD6 | `bg-surface` #15181D | 3.51:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `accent` #2D6AD6 | `bg-raised` #1C2027 | 3.22:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `border-default` #353C47 | `bg-surface` #15181D | 1.60:1 | decorative (info)  | — |
| `syn-keyword` #C59BFF | `bg-editor` #0E1013 | 8.63:1 | text ≥ 4.5 | PASS |
| `syn-keyword` #C59BFF | `bg-editor-line` #171B21 | 7.82:1 | text ≥ 4.5 | PASS |
| `syn-string` #8FD18F | `bg-editor` #0E1013 | 10.60:1 | text ≥ 4.5 | PASS |
| `syn-string` #8FD18F | `bg-editor-line` #171B21 | 9.62:1 | text ≥ 4.5 | PASS |
| `syn-number` #F2B36F | `bg-editor` #0E1013 | 10.39:1 | text ≥ 4.5 | PASS |
| `syn-number` #F2B36F | `bg-editor-line` #171B21 | 9.43:1 | text ≥ 4.5 | PASS |
| `syn-comment` #8C95A3 | `bg-editor` #0E1013 | 6.30:1 | text ≥ 4.5 | PASS |
| `syn-comment` #8C95A3 | `bg-editor-line` #171B21 | 5.71:1 | text ≥ 4.5 | PASS |
| `syn-function` #7FB0FF | `bg-editor` #0E1013 | 8.67:1 | text ≥ 4.5 | PASS |
| `syn-function` #7FB0FF | `bg-editor-line` #171B21 | 7.86:1 | text ≥ 4.5 | PASS |
| `fg-primary` #E7EAEE | `bg-editor` #0E1013 | 15.79:1 | text ≥ 4.5 | PASS |
| `fg-primary` #E7EAEE | `bg-editor-line` #171B21 | 14.32:1 | text ≥ 4.5 | PASS |
| `danger-fg` #FF7B72 | `bg-editor` #0E1013 | 7.56:1 | text ≥ 4.5 | PASS |

#### Светлая тема

| Передний план | Фон | Контраст | Требование | Итог |
|---|---|---|---|---|
| `fg-primary` #16191E | `bg-canvas` #F4F5F7 | 16.15:1 | text ≥ 4.5 | PASS |
| `fg-primary` #16191E | `bg-surface` #FFFFFF | 17.62:1 | text ≥ 4.5 | PASS |
| `fg-primary` #16191E | `bg-raised` #FFFFFF | 17.62:1 | text ≥ 4.5 | PASS |
| `fg-primary` #16191E | `bg-overlay` #FFFFFF | 17.62:1 | text ≥ 4.5 | PASS |
| `fg-primary` #16191E | `bg-hover` #EBEDF0 | 15.02:1 | text ≥ 4.5 | PASS |
| `fg-primary` #16191E | `bg-selected` #E3ECFD | 14.84:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #454D5A | `bg-canvas` #F4F5F7 | 7.82:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #454D5A | `bg-surface` #FFFFFF | 8.53:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #454D5A | `bg-raised` #FFFFFF | 8.53:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #454D5A | `bg-overlay` #FFFFFF | 8.53:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #454D5A | `bg-hover` #EBEDF0 | 7.27:1 | text ≥ 4.5 | PASS |
| `fg-secondary` #454D5A | `bg-selected` #E3ECFD | 7.18:1 | text ≥ 4.5 | PASS |
| `fg-muted` #5B6472 | `bg-canvas` #F4F5F7 | 5.48:1 | text ≥ 4.5 | PASS |
| `fg-muted` #5B6472 | `bg-surface` #FFFFFF | 5.98:1 | text ≥ 4.5 | PASS |
| `fg-muted` #5B6472 | `bg-raised` #FFFFFF | 5.98:1 | text ≥ 4.5 | PASS |
| `fg-muted` #5B6472 | `bg-overlay` #FFFFFF | 5.98:1 | text ≥ 4.5 | PASS |
| `fg-muted` #5B6472 | `bg-hover` #EBEDF0 | 5.10:1 | text ≥ 4.5 | PASS |
| `fg-muted` #5B6472 | `bg-selected` #E3ECFD | 5.04:1 | text ≥ 4.5 | PASS |
| `fg-disabled` #A0A7B1 | `bg-raised` #FFFFFF | 2.43:1 | disabled (exempt, info)  | — |
| `on-accent` #FFFFFF | `accent` #1F5FCC | 5.89:1 | text ≥ 4.5 | PASS |
| `on-accent` #FFFFFF | `accent-hover` #1A53B5 | 7.12:1 | text ≥ 4.5 | PASS |
| `on-accent` #FFFFFF | `accent-pressed` #16479C | 8.69:1 | text ≥ 4.5 | PASS |
| `on-danger` #FFFFFF | `danger-solid` #C42B2B | 5.63:1 | text ≥ 4.5 | PASS |
| `accent-fg` #1A56BD | `bg-canvas` #F4F5F7 | 6.19:1 | text ≥ 4.5 | PASS |
| `accent-fg` #1A56BD | `bg-surface` #FFFFFF | 6.75:1 | text ≥ 4.5 | PASS |
| `accent-fg` #1A56BD | `bg-raised` #FFFFFF | 6.75:1 | text ≥ 4.5 | PASS |
| `accent-fg` #1A56BD | `bg-selected` #E3ECFD | 5.69:1 | text ≥ 4.5 | PASS |
| `success-fg` #18794A | `success-bg` #E6F4EC | 4.78:1 | text ≥ 4.5 | PASS |
| `success-fg` #18794A | `bg-surface` #FFFFFF | 5.42:1 | text ≥ 4.5 | PASS |
| `success-fg` #18794A | `bg-raised` #FFFFFF | 5.42:1 | text ≥ 4.5 | PASS |
| `warning-fg` #8A5300 | `warning-bg` #FBF1DE | 5.65:1 | text ≥ 4.5 | PASS |
| `warning-fg` #8A5300 | `bg-surface` #FFFFFF | 6.33:1 | text ≥ 4.5 | PASS |
| `warning-fg` #8A5300 | `bg-raised` #FFFFFF | 6.33:1 | text ≥ 4.5 | PASS |
| `danger-fg` #BF2A2A | `danger-bg` #FCE9E8 | 5.02:1 | text ≥ 4.5 | PASS |
| `danger-fg` #BF2A2A | `bg-surface` #FFFFFF | 5.87:1 | text ≥ 4.5 | PASS |
| `danger-fg` #BF2A2A | `bg-raised` #FFFFFF | 5.87:1 | text ≥ 4.5 | PASS |
| `info-fg` #1A64A8 | `info-bg` #E5F0FA | 5.30:1 | text ≥ 4.5 | PASS |
| `info-fg` #1A64A8 | `bg-surface` #FFFFFF | 6.12:1 | text ≥ 4.5 | PASS |
| `info-fg` #1A64A8 | `bg-raised` #FFFFFF | 6.12:1 | text ≥ 4.5 | PASS |
| `focus-ring` #1F5FCC | `bg-canvas` #F4F5F7 | 5.40:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `border-strong` #7D8796 | `bg-canvas` #F4F5F7 | 3.33:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `focus-ring` #1F5FCC | `bg-surface` #FFFFFF | 5.89:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `border-strong` #7D8796 | `bg-surface` #FFFFFF | 3.63:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `focus-ring` #1F5FCC | `bg-raised` #FFFFFF | 5.89:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `border-strong` #7D8796 | `bg-raised` #FFFFFF | 3.63:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `focus-ring` #1F5FCC | `bg-overlay` #FFFFFF | 5.89:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `border-strong` #7D8796 | `bg-overlay` #FFFFFF | 3.63:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `accent` #1F5FCC | `bg-surface` #FFFFFF | 5.89:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `accent` #1F5FCC | `bg-raised` #FFFFFF | 5.89:1 | non-text 1.4.11 ≥ 3.0 | PASS |
| `border-default` #CDD2D9 | `bg-surface` #FFFFFF | 1.52:1 | decorative (info)  | — |
| `syn-keyword` #7A3DC7 | `bg-editor` #FFFFFF | 6.33:1 | text ≥ 4.5 | PASS |
| `syn-keyword` #7A3DC7 | `bg-editor-line` #F2F4F7 | 5.74:1 | text ≥ 4.5 | PASS |
| `syn-string` #2D7A2D | `bg-editor` #FFFFFF | 5.34:1 | text ≥ 4.5 | PASS |
| `syn-string` #2D7A2D | `bg-editor-line` #F2F4F7 | 4.85:1 | text ≥ 4.5 | PASS |
| `syn-number` #A3520A | `bg-editor` #FFFFFF | 5.57:1 | text ≥ 4.5 | PASS |
| `syn-number` #A3520A | `bg-editor-line` #F2F4F7 | 5.06:1 | text ≥ 4.5 | PASS |
| `syn-comment` #5F6876 | `bg-editor` #FFFFFF | 5.63:1 | text ≥ 4.5 | PASS |
| `syn-comment` #5F6876 | `bg-editor-line` #F2F4F7 | 5.11:1 | text ≥ 4.5 | PASS |
| `syn-function` #1A56BD | `bg-editor` #FFFFFF | 6.75:1 | text ≥ 4.5 | PASS |
| `syn-function` #1A56BD | `bg-editor-line` #F2F4F7 | 6.13:1 | text ≥ 4.5 | PASS |
| `fg-primary` #16191E | `bg-editor` #FFFFFF | 17.62:1 | text ≥ 4.5 | PASS |
| `fg-primary` #16191E | `bg-editor-line` #F2F4F7 | 15.99:1 | text ≥ 4.5 | PASS |
| `danger-fg` #BF2A2A | `bg-editor` #FFFFFF | 5.87:1 | text ≥ 4.5 | PASS |

Итого FAIL: **0**.

## 3. Типографика

Шрифты — **только системные Windows, без бандлинга** (нет сетевых загрузок, нет лицензионных вопросов, нет роста установщика):

| Токен | Стек | Комментарий |
|---|---|---|
| `--font-sans` | `"Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI", system-ui, sans-serif` | Segoe UI Variable — Windows 11; на Windows 10 — Segoe UI. Кириллица есть в обоих. |
| `--font-display` | `"Segoe UI Variable Display", "Segoe UI Variable", "Segoe UI", system-ui, sans-serif` | для H1/крупных цифр (оптический размер Display) |
| `--font-mono` | `"Cascadia Mono", "Cascadia Code", Consolas, "Courier New", monospace` | Cascadia Mono — Windows 11 (и с Windows Terminal); Consolas — Windows 10 гарантированно. Кириллица есть. Лигатуры выключить: `font-variant-ligatures: none` (Cascadia Code). |

Будущее (macOS): стек расширяется `-apple-system, "SF Pro Text"` и `"SF Mono", Menlo` — изменение только токена.

| Токен | Размер / line-height | Насыщенность | Использование |
|---|---|---|---|
| `--text-xs` | 11px / 16px | 400 | клавиши-чипы, бейджи, строка состояния |
| `--text-sm` | 12px / 16px | 400 / 600 | подписи полей, метаданные, заголовки колонок (600, без капса) |
| `--text-base` | 13px / 20px | 400 | основной текст UI, таблицы, кнопки (500) |
| `--text-md` | 14px / 20px | 400 / 600 | описания разделов, текст карточек, пункты сайдбара (500) |
| `--text-lg` | 16px / 24px | 600 | заголовки карточек и групп настроек (H2) |
| `--text-xl` | 20px / 28px | 600 | заголовок раздела (H1), `--font-display` |
| `--text-2xl` | 28px / 36px | 600 | крупный статус на Обзоре, онбординг |
| `--text-code` | 13px / 20px | 400 | редактор кода, логи (`--font-mono`) |

Правила: насыщенности только 400 / 500 / 600 (Segoe UI 500 = Semibold-light на Win10 → fallback 600 допустим); без капса для заголовков (кириллица в капсе длиннее и хуже читается); `font-variant-numeric: tabular-nums` для времени, мс, счётчиков; минимальный размер текста — 11 px только для чипов/бейджей, всё читаемое — ≥ 12 px.

## 4. Отступы (spacing)

Базовая единица `--spacing: 4px` (Tailwind v4: `p-1` = 4px).

| Токен | px | Tailwind | Где |
|---|---|---|---|
| `--ks-space-0_5` | 2 | `0.5` | зазор между чипами клавиш, offset фокус-кольца |
| `--ks-space-1` | 4 | `1` | иконка ↔ текст в бейдже |
| `--ks-space-2` | 8 | `2` | иконка ↔ текст в кнопке, gap в группе кнопок |
| `--ks-space-3` | 12 | `3` | padding полей по горизонтали, строки таблицы |
| `--ks-space-4` | 16 | `4` | padding карточки, gap сетки карточек |
| `--ks-space-5` | 20 | `5` | padding модалки |
| `--ks-space-6` | 24 | `6` | поля рабочей области, между группами |
| `--ks-space-8` | 32 | `8` | между секциями страницы |
| `--ks-space-12` | 48 | `12` | empty states, онбординг |

Размеры контролов: `--ks-control-sm: 24px` (иконки-кнопки в таблицах, 24×24 — минимум WCAG 2.5.8), `--ks-control-md: 32px` (по умолчанию), `--ks-control-lg: 40px` (онбординг, окно выбора языка). Сайдбар `--ks-sidebar-w: 224px` / свернутый `56px`; строка состояния `24px`; max-width контента `1040px`.

## 5. Радиусы

| Токен | px | Где |
|---|---|---|
| `--radius-sm` | 4 | клавиши-чипы, бейджи, чекбоксы |
| `--radius-md` | 6 | кнопки, поля, сегменты, пункты навигации |
| `--radius-lg` | 8 | карточки, панели, меню |
| `--radius-xl` | 12 | модалки, окно выбора языка |
| `--radius-full` | 9999 | тумблер, точки статуса, пилюли |

## 6. Высота (elevation)

| Токен | Dark | Light | Где |
|---|---|---|---|
| `--ks-shadow-0` | none | none | всё в потоке (карточки отделены рамкой) |
| `--ks-shadow-1` | `0 1px 2px rgba(0,0,0,.40)` | `0 1px 2px rgba(16,24,40,.06)` | карточки в светлой теме, липкие заголовки |
| `--ks-shadow-2` | `0 4px 12px rgba(0,0,0,.45), 0 0 0 1px #353C47` | `0 4px 12px rgba(16,24,40,.10), 0 0 0 1px #E4E7EB` | меню, select, tooltip |
| `--ks-shadow-3` | `0 12px 32px rgba(0,0,0,.55), 0 0 0 1px #353C47` | `0 12px 32px rgba(16,24,40,.16), 0 0 0 1px #CDD2D9` | модалки, окно выбора языка |
| `--ks-scrim` | `rgba(0,0,0,.55)` | `rgba(16,24,40,.32)` | подложка модалки |

В тёмной теме тень почти невидима, поэтому `shadow-2/3` включают 1-px обводку. Z-index: `base 0 · sticky 10 · dropdown 100 · modal 1000 · toast 1100`.

## 7. Движение (motion)

| Токен | Значение | Где |
|---|---|---|
| `--ks-dur-instant` | 0ms | смена раздела, выбор в окне языка (закрытие без анимации) |
| `--ks-dur-fast` | 100ms | hover/press, цвет, фокус |
| `--ks-dur-base` | 160ms | тумблер, сегменты, раскрытие меню |
| `--ks-dur-slow` | 240ms | модалка, нижняя панель |
| `--ease-standard` | `cubic-bezier(0.2,0,0,1)` | вход/перемещение |
| `--ease-exit` | `cubic-bezier(0.3,0,1,1)` | выход |

Правила: анимируются только `opacity`, `transform`, `color`, `background-color`, `border-color`; никаких «пружин» и анимаций дольше 240 мс; окно выбора языка появляется ≤ 100 мс (fade+scale 0.98→1) — скорость важнее эффекта.

```css
@media (prefers-reduced-motion: reduce) {
  :root { --ks-dur-fast: 0ms; --ks-dur-base: 0ms; --ks-dur-slow: 0ms; }
  *, *::before, *::after { animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; transition-duration: 0.01ms !important; }
}
```

## 8. Иконки

- **Рекомендация: Lucide** (`lucide-react`) — лицензия **ISC** (совместима с коммерческим использованием, требует сохранения уведомления об авторских правах в лицензиях компонентов). Линейные иконки 24×24, stroke 1.5–2 — естественно сочетаются с IDE-стилем. Альтернатива: **Tabler Icons** (MIT). Проверить точную версию/лицензию при добавлении зависимости по правилу CLAUDE.md №3 (обоснование, версия не моложе 2 недель). Вариант без зависимости — копировать нужные ~25 SVG в репозиторий с файлом лицензии.
- Размеры: 16 px в кнопках/таблицах/сайдбаре (stroke 1.75), 20 px в заголовках карточек, 14 px в бейджах. Цвет — `currentColor`.
- Иконка-только кнопка обязательно имеет `aria-label` и tooltip. Иконки статусов дублируются текстом.
- Набор для v1 (имена Lucide): `layout-dashboard` (Обзор), `languages` (Языки), `keyboard` (Горячие клавиши), `square-code` (Macro Studio), `settings` (Настройки), `activity` (Диагностика), `pause`/`play`, `circle-check`, `circle-alert`, `triangle-alert`, `info`, `x`, `lock`, `shield-check` (приватность), `play` (тест), `scan-search`/`check-check` (проверить), `save`, `trash-2`, `copy`, `chevron-down`, `search`.
- Не использовать логотипы сторонних продуктов (в т. ч. иконки браузеров/мессенджеров в журнале) — приложение показывается текстом (`telegram.exe`).

## 9. Компоненты

Общие состояния для всех интерактивных элементов: **default · hover · focus-visible · active (pressed) · disabled**, плюс **error / loading / selected**, где применимо. Фокус для всех: `outline: 2px solid var(--ks-focus-ring); outline-offset: 2px;` только на `:focus-visible` (не `box-shadow` — должен работать в `forced-colors`). Disabled для всех: `color: var(--ks-fg-disabled)`, курсор `not-allowed`, без hover-эффекта, `aria-disabled="true"` (если элемент должен оставаться фокусируемым для подсказки о причине) или `disabled`.

### 9.1. Кнопка (Button)

Высота 32 px (sm 24, lg 40), padding 0 12px, radius-md, `--text-base`/500, иконка 16 px + gap 8. Ширина — по содержимому (`min-width: 72px`), **никогда фиксированная** (RU-строки).

| Вариант | Default | Hover | Active | Disabled |
|---|---|---|---|---|
| **Primary** | bg `accent`, text `on-accent` | bg `accent-hover` | bg `accent-pressed` | bg `bg-hover`, text `fg-disabled` |
| **Secondary** | bg `bg-raised`, 1px `border-strong`, text `fg-primary` | bg `bg-hover` | bg `bg-overlay` | рамка `border-default`, text `fg-disabled` |
| **Ghost** | прозрачный, text `fg-secondary` | bg `bg-hover`, text `fg-primary` | bg `bg-overlay` | text `fg-disabled` |
| **Danger** | bg `danger-solid`, text `on-danger` | яркость −6% | яркость −12% | как primary disabled |
| **Icon** | 32×32 (24×24 sm), ghost-стиль, `aria-label` | как ghost | как ghost | как ghost |

Loading: спиннер 14 px вместо иконки, текст меняется на «Проверка…», `aria-busy="true"`, ширина не прыгает (min-width = прежняя ширина). Одна primary-кнопка на регион.

### 9.2. Поле ввода (Input / Textarea)

32 px, padding 0 12px, bg `bg-raised`, 1px `border-strong`, radius-md, текст `fg-primary`, плейсхолдер `fg-muted`. Подпись над полем (`--text-sm`/600, `fg-secondary`), вспомогательный текст под полем (`--text-sm`, `fg-muted`).

| Состояние | Вид |
|---|---|
| hover | рамка `fg-muted` |
| focus | рамка `accent` + outline focus-ring |
| error | рамка `danger-fg`, иконка `circle-alert` справа, текст ошибки `danger-fg` под полем, `aria-invalid="true"` + `aria-describedby` |
| disabled | bg `bg-surface`, рамка `border-default`, текст `fg-disabled` |
| read-only | bg прозрачный, рамка `border-subtle`, текст `fg-secondary`, выделение текста разрешено |

Поля для пользовательского текста (песочница, тестовые данные): `autocomplete="off" spellcheck="false"`, подпись «Не сохраняется».

### 9.3. Select (выпадающий список)

Как input + `chevron-down` 16 px справа. Список: `bg-overlay`, `shadow-2`, radius-lg, пункты 32 px, hover `bg-hover`, выбранный — галочка + `fg-primary`/600. Клавиатура: ↑/↓, Home/End, ввод первой буквы, Enter/Esc. Рекомендуется нативный `<select>` для простых списков (язык UI) — лучшая доступность.

### 9.4. Тумблер (Toggle / Switch)

Дорожка 36×20, radius-full, бегунок 14 px. `role="switch"` + `aria-checked`. Подпись слева/справа кликабельна.

| Состояние | Дорожка | Бегунок |
|---|---|---|
| off | bg прозрачный, 1px `border-strong` | `fg-secondary` |
| on | bg `accent` | `on-accent` (белый) |
| hover | off: bg `bg-hover`; on: `accent-hover` | — |
| focus | + focus-ring | — |
| disabled | рамка/заливка `border-default` | `fg-disabled` |
| pending (запись в реестр) | как целевое состояние, бегунок со спиннером | — |

Изменение применяется сразу (без кнопки «Сохранить»), ошибка → откат + inline-алерт.

### 9.5. Сегментированный контрол (Segmented control)

Контейнер 32 px, bg `bg-surface`, 1px `border-default`, radius-md, padding 2px; сегменты 28 px, radius-sm, `--text-base`/500. `role="radiogroup"`, сегменты `role="radio"`, стрелки ←/→ перемещают выбор (roving tabindex).

| Состояние | Вид |
|---|---|
| default | text `fg-secondary` |
| hover | bg `bg-hover`, text `fg-primary` |
| selected | bg `bg-raised` (light: `bg-selected`), text `fg-primary`, 1px `border-strong`, `shadow-1` |
| focus | focus-ring вокруг сегмента |
| disabled | text `fg-disabled` |

Используется: тип слота (Текст / JavaScript / Python), тема, фильтры журнала, источник контекста.

### 9.6. Контрол захвата горячей клавиши (Hotkey capture)

Высота 32 px, min-width 220 px, bg `bg-raised`, 1px `border-strong`, radius-md; внутри — клавиши-чипы; справа кнопка-иконка «Очистить» (24×24). `role="button"`, `aria-label="Сочетание для «Исправить раскладку»: Control плюс Shift плюс Пробел. Нажмите, чтобы изменить"`.

**Клавиша-чип (kbd):** `--font-mono`, `--text-xs`/500, padding 1px 6px, min-width 22px, bg `bg-overlay`, 1px `border-default`, нижняя граница 2px (эффект клавиши), radius-sm, text `fg-primary`; разделитель между чипами — 2 px (без «+»; «+» только в тексте подсказок).

| Состояние | Вид / поведение |
|---|---|
| idle | чипы сочетания |
| hover | рамка `fg-muted`, курсор pointer |
| focus | focus-ring |
| recording | рамка 1px `accent` + outline focus-ring, пульсирующая точка `danger-fg` 6 px (reduced-motion: статичная), текст `fg-muted` «Нажмите сочетание… Esc — отмена», чипы появляются по мере нажатия; `aria-live="polite"` |
| validating | спиннер справа (регистрация в ОС) |
| registered | чипы + рядом пилюля «Зарегистрировано» (success) |
| error | рамка `danger-fg`, под полем текст ошибки (`danger-fg`, `--text-sm`), кнопки «Вернуть прежнее»; прежнее сочетание остаётся активным |
| empty | плейсхолдер «Не задано», `fg-muted` |
| disabled | (пауза/выключено) чипы `fg-disabled`, пилюля «Приостановлено» |

Правила валидации: 2–3 клавиши, ≥ 1 не-модификатор (D-14); не совпадает с другим действием; не из списка зарезервированных; успешный `RegisterHotKey`. Esc — отмена записи, Tab/Shift+Tab — выход из записи без изменения, Backspace/Delete в idle — очистить (с подтверждением, если действие станет «Не задано»).

### 9.7. Навигация: пункт сайдбара и вкладки

**Пункт сайдбара:** 32 px, padding 0 10px, radius-md, иконка 16 + подпись `--text-md`/500, `fg-secondary`. Hover: bg `bg-hover`, `fg-primary`. Active (`aria-current="page"`): bg `bg-selected`, text `fg-primary`, иконка `accent-fg`, индикатор-полоска 2×16 px `accent` слева. Focus: focus-ring (внутренний offset −2px). Свернутый сайдбар: 40×32, только иконка, tooltip справа. Подпись переносится максимум на 2 строки.

**Вкладки (Tabs, нижняя панель Macro Studio):** высота 32 px, `--text-sm`/600, `fg-secondary`; выбранная — `fg-primary` + нижняя линия 2px `accent`; счётчик-бейдж («Проблемы 1»). `role="tablist"/"tab"/"tabpanel"`, ←/→, Home/End.

### 9.8. Таблица / строка журнала

Заголовок: 32 px, `--text-sm`/600, `fg-muted`, bg `bg-surface`, липкий, нижняя рамка `border-default`. Строка: 36 px, `--text-base`, рамка снизу `border-subtle`; время и мс — `--font-mono` + `tabular-nums`; приложение — `--font-mono`; код причины — `--font-mono` `--text-sm` в `fg-secondary`.

| Состояние | Вид |
|---|---|
| hover | bg `bg-hover` |
| selected | bg `bg-selected`, полоса 2px `accent` слева |
| focus (строка фокусируема для деталей) | focus-ring внутрь |
| empty table | empty state по центру, `fg-muted`, иконка 24 px |
| loading | 5 скелетон-строк (bg `bg-hover`, без shimmer при reduced-motion) |

Узкое окно: колонки «Объём», «Способ» скрываются < 960 px; таблица прокручивается горизонтально внутри своего контейнера.

### 9.9. Бейдж и статус-пилюля

Пилюля: высота 20 px, padding 0 8px, radius-full, `--text-xs`/600, bg `*-bg`, текст `*-fg`, слева точка/иконка 8–12 px **разной формы**: success — ● круг, warning — ▲, danger — ✕, info — i, neutral (bg `bg-hover`, text `fg-secondary`) — ◌. Бейдж-счётчик: 16 px, min-width 16, `--text-xs`, bg `bg-hover` (или `danger-bg` при ошибках). Не интерактивны.

### 9.10. Inline-алерт и тост

**Inline-алерт** (основной способ сообщать проблемы): bg `*-bg`, рамка слева 3px `*-fg`, radius-md, padding 12px 16px, иконка 16 + заголовок `--text-base`/600 + текст `--text-base` `fg-primary` + опциональная кнопка-ссылка. Варианты info / warning / danger (success — **не используется**, см. UX-правило 1). `role="alert"` только для danger, появившегося в ответ на действие; иначе `role="status"`.

**Тост**: только для асинхронных ошибок без привязки к элементу. Правый нижний угол над строкой состояния, ширина 360 px, `bg-overlay`, `shadow-2`, рамка слева `danger-fg`, кнопка закрыть, автоскрытие **нет** (ошибка не должна исчезать сама), максимум 1 одновременно. Success/info-тосты не используются.

### 9.11. Модальное окно

Ширина 440 px (max 90vw), bg `bg-overlay`, `shadow-3`, radius-xl, padding 20px; заголовок `--text-lg`/600, текст `--text-base`, футер — кнопки справа (Отмена — secondary, действие — primary/danger). Подложка `--ks-scrim`. `role="dialog"` `aria-modal="true"` `aria-labelledby`; ловушка фокуса; Esc = Отмена; начальный фокус — на безопасной кнопке; при закрытии фокус возвращается на инициатор. Анимация: fade + translateY(4px) 240 мс.

### 9.12. Окно выбора языка (Language picker popup)

Отдельное окно ОС (не модалка в главном окне), 320×~132 px, bg `bg-overlay`, `shadow-3`, radius-xl, padding 16px, всегда поверх.

- Заголовок `--text-base`/600 «Смешанный текст — исправить на:».
- Две кнопки-варианта 40 px высотой (lg), равной ширины, secondary-стиль; слева в кнопке — клавиша-чип `1`/`2`, затем название языка (`--text-md`/500) и метка `RU`/`EN` (`fg-muted`, mono). Hover: `bg-hover`; focus: focus-ring; pressed: `bg-selected`. Первая кнопка — **не** выделена как primary (выбор нейтрален).
- Строка отмены: чип `Esc` + «Отмена — ничего не менять» (ghost-кнопка, `fg-secondary`).
- Индикатор таймаута: полоса 2 px `accent` внизу, линейно уменьшается 10 с; reduced-motion — текст «Отмена через 10 с» (обновлять раз в секунду не нужно).
- Появление ≤ 100 мс, закрытие мгновенное. Позиционирование: под кареткой +8 px, иначе над, с прижатием к рабочей области монитора; fallback — центр монитора активного окна.
- `role="dialog"`, `aria-labelledby`, кнопки `aria-keyshortcuts`.

### 9.13. Хром редактора кода (Code editor chrome)

- Контейнер: bg `bg-editor`, 1px `border-default`, radius-lg (только снаружи), `--text-code`.
- Гуттер: ширина ≥ 44 px, номера строк `fg-muted` `tabular-nums` справа, padding-right 12px; текущая строка — номер `fg-primary`, фон строки `bg-editor-line`; маркер ошибки в гуттере — ✕ `danger-fg` (форма, не только цвет).
- Выделение: `accent` с прозрачностью 30% (dark) / `bg-selected` (light). Каретка 2 px `fg-primary`.
- Ошибка: волнистое подчёркивание `danger-fg`, tooltip с сообщением на hover и при фокусе (Ctrl+K Ctrl+I / F8 — следующая ошибка).
- Тулбар над редактором: 40 px, bg `bg-surface`: сегменты типа, тумблер «Включён», горячая клавиша (чипы, ссылка на Hotkeys).
- Нижняя панель: вкладки (§9.7) + список проблем (строка 28 px: иконка, `Стр. X, кол. Y` mono, сообщение, «Перейти»). Высота 160 px по умолчанию, ресайз ручкой 4 px (клавиатура: фокус на ручке + ↑/↓).
- Строка состояния: «Стр. 4, кол. 22 · JavaScript · UTF-8 · Не сохранено» (`--text-xs`).
- CodeMirror 6: тема строится из этих токенов (`EditorView.theme` + `HighlightStyle` с `--ks-syn-*`), отдельная тема на dark/light не нужна — переменные меняются сами.

## Приложение A. Скрипт расчёта контраста

Исходник (запускался из временного каталога, в репозиторий не добавлен; может быть перенесён в `scripts/` по решению оркестратора):

```python
# WCAG 2.x contrast check for KeySwitch design tokens
def lum(h):
    h = h.lstrip('#'); c = [int(h[i:i+2], 16) / 255 for i in (0, 2, 4)]
    c = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]

def cr(a, b):
    la, lb = sorted([lum(a), lum(b)], reverse=True)
    return (la + 0.05) / (lb + 0.05)

# T = {"dark": {...}, "light": {...}}  — hex-значения из §2.2/§2.3
# Пары: fg-primary/secondary/muted × все фоны (≥4.5), on-accent × accent/hover/pressed (≥4.5),
# on-danger × danger-solid (≥4.5), accent-fg × фоны (≥4.5), *-fg × *-bg/surface/raised (≥4.5),
# focus-ring и border-strong × фоны (≥3.0), accent × surface/raised (≥3.0),
# syn-* и fg-primary × bg-editor/bg-editor-line (≥4.5); disabled/decorative — информационно.
```
