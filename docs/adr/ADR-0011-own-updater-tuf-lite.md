# ADR-0011 — Собственный механизм обновлений (TUF-lite) вместо `tauri-plugin-updater`

Статус: **Proposed** (нужны одобрения CISO, CLO; корень доверия — LEVEL 3 владелец) · Дата: 2026-11-10 · Связано: ADR-0009, P-U1…P-U6 · Автор: AGENT 03

## Контекст
Нужно реализовать D-25 с каналами STABLE/EMERGENCY, анти-downgrade, защитой от freeze, emergency-политиками и разделением процессов.

## Проблема
`tauri-plugin-updater` 2.14.x (проверено по исходникам crate) подписывает только артефакт: манифест не подписан, нет сроков, `min_safe_version`, отзыва, ротации/порогов ключей; `requireSignedVersion` по умолчанию выключен; работает в процессе с WebView и тянет `reqwest`/`rustls`; на macOS запрашивает пароль администратора.

## Альтернативы
1. Плагин в основном процессе с `requireSignedVersion`.
2. Плагин во вспомогательном Tauri-процессе + наш подписанный надманифест.
3. Velopack / Sparkle+WinSparkle / `tough`.
4. **Свой `ks-update-core` (без I/O) + `ks-updater` (HTTP ОС, AppContainer `internetClient`), формат артефактов и подписи Tauri.**

## Решение
Вариант 4; запасной — 2. Роли `root` K-1 (офлайн, ≥ 2 корня, порог в `root.json`), `release` K-2, `policy` K-3, `timestamp` K-4 (7 дней) — по `SIGNING_KEY_MANAGEMENT.md`; крипто — `minisign-verify =0.3.0`.

## Обоснование
Полный контроль правил доверия при минимуме зависимостей; формат Tauri позволяет использовать штатный `tauri signer`.

## Последствия
~1,5–2,5 тыс. строк своего кода с fuzz-тестами; нужна офлайн-церемония ключей и ежедневное переподписание `timestamp` в CI.

## Проверка
Спайк U-1 (BL-UPD-03) и критерии `UPDATE_SECURITY_ARCHITECTURE.md` §18.
