# Hardware & Compatibility Matrix — Windows

Владелец: AGENT 20 (первичная версия — PD, 2026-11-10). macOS — `COMPATIBILITY_MATRIX_MACOS.md`. Поддержка объявляется **только после реального тестирования** (V7 §16.5). Статусы: UNVERIFIED · CI (автоматически на фикстуре) · MANUAL (вручную на устройстве, дата/версия) · SUPPORTED · NOT SUPPORTED.

## ОС
| Платформа | Архитектура | Целевой статус | Текущий статус | Примечание |
|-----------|-------------|----------------|----------------|------------|
| Windows 11 (23H2, 24H2, 25H2) | x64 | основная | UNVERIFIED | CI-раннер `windows-latest` — Windows Server, не клиентская Windows 11 |
| Windows 11 | ARM64 | решение позже (рекомендация: после x64) | UNVERIFIED | — |
| Windows 10 22H2 с ESU / LTSC 2019/2021 | x64 | дополнительная, отдельная матрица редакций | UNVERIFIED | Home/Pro без ESU — вне поддержки обновлений безопасности ОС; поддерживать ли — решение по итогам CLO/CISO |

## Приложения (ожидаемое поведение — `pa-app-matrix.md`)
| Приложение | Ожидаемо | Статус Win11 | Статус Win10 |
|------------|----------|--------------|--------------|
| Notepad (Win11, WinUI) | поле/фрагмент по профилю | UNVERIFIED | — |
| Notepad (Win10, Win32 Edit) | поле/фрагмент | — | UNVERIFIED |
| Chrome | только выделение/фрагмент до S-2 | UNVERIFIED | UNVERIFIED |
| Edge | как Chrome | UNVERIFIED | UNVERIFIED |
| Firefox | экспериментально | UNVERIFIED | UNVERIFIED |
| Word | выделение/фрагмент (отдельный адаптер) | UNVERIFIED | UNVERIFIED |
| Excel | только режим правки ячейки | UNVERIFIED | UNVERIFIED |
| Telegram Desktop | экспериментально (Qt) | UNVERIFIED | UNVERIFIED |
| Discord | профиль, отказ при встроенных объектах | UNVERIFIED | UNVERIFIED |
| WhatsApp Desktop | по данным рынка UIA текст не отдаёт → выделение через Ctrl+C по профилю (D-19d) | UNVERIFIED | UNVERIFIED |
| Консоль / Windows Terminal | отказ | NOT SUPPORTED (решение) | NOT SUPPORTED |
