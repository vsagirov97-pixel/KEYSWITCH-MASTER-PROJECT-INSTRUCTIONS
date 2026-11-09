---
name: stage-report
description: Produce the KeySwitch stage report in the format required by the master instructions §20 (in Russian). Use after finishing a stage, gate or spike, or when the owner asks for status.
---
# /stage-report

Gather facts first: `git log --oneline origin/main..HEAD` per active branch, `git branch -r`, the last `scripts/check.sh` Summary, CI results you actually observed, open items in `docs/04-plan.md`.

Write in Russian, exactly these sections:
1. Завершённые задачи
2. Работавшие специалисты (роли/субагенты)
3. Ветки и Pull Requests (полные ссылки)
4. Проверенные функции — только с указанием где/как проверено
5. Результаты тестов — фактические цифры
6. Обнаруженные дефекты
7. Результат Security Review
8. Открытые риски
9. Следующий этап и что нужно от владельца

Never mix facts and assumptions: mark anything not executed as «не проверено».
