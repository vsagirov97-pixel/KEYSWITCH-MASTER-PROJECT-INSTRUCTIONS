# ADR-0008 — Разделение «ядро / адаптеры» для Windows и macOS

Статус: **Accepted** · Дата: 2026-11-10 · Решения: D-24 (V7 §2), принцип 1 `SYSTEM_ARCHITECTURE.md` · Автор: AGENT 03

## Контекст
V7 делает macOS обязательной полноценной платформой; Windows 11 — основная, Windows 10 — отдельная матрица.

## Проблема
Если логика политики, конвейера и безопасности смешана с Win32/UIA, macOS потребует второго продукта и удвоит ошибки безопасности.

## Альтернативы
1. Отдельные кодовые базы — дублирование политики.
2. Кроссплатформенные библиотеки ввода/буфера (`enigo`, `arboard`) — не дают нужных гарантий (сигнал потребления, отпечаток фокуса, метки D-13).
3. **Общие `ks-core` (без I/O), `ks-engine` (политика), `ks-types`; ОС-специфика — за трейтами `ks-platform` с сессиями и непрозрачными ID; адаптеры `ks-platform-win`, `ks-platform-mac`; `ks-platform-fake` для тестов.**

## Решение
Трейты (`Environment`, `FieldAccess`, `InputSynth`, `ClipboardAccess`, `KeyboardLayouts`, `Chooser`, `Hotkeys`, `Autostart`, `SandboxLauncher`, `UpdateInstaller`) без Windows-типов. `unsafe` — только в адаптерах. macOS-адаптер: Accessibility (AXUIElement), NSPasteboard, Input Sources (TIS), Menu Bar, Login Items, Permission Management (TCC), CGEvent.

## Обоснование
Инварианты безопасности проверяются один раз на fake-платформе; адаптер отвечает только за точное отображение API.

## Последствия
- Различия (глобальное переключение раскладки на macOS, TCC-разрешения) выражаются через возможности (`FieldCaps`, `Tri::Unknown`), а не ветвления в движке.
- CI на macOS-раннерах обязателен; отдельный план выпуска (GATE 8, 16).

## Проверка
Тесты `ks-engine` на `ks-platform-fake` (журнал вызовов, I-1); компиляция трейтов без `cfg(windows)`; `MACOS_PLATFORM_MAP.md` (Senior macOS Platform Lead).
