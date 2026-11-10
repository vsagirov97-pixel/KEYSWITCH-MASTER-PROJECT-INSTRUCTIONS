#!/usr/bin/env bash
# Generates docs/SBOM.md: shipped (normal, non-dev, non-build) dependencies per target + npm runtime deps.
# Machine-readable CycloneDX output is a release task (BL-REL-02); this file is the human-readable inventory.
set -euo pipefail
cd "$(dirname "$0")/.."
out=docs/SBOM.md
{
  echo "# SBOM — перечень программных компонентов"
  echo
  echo "Сгенерировано \`scripts/sbom.sh\` $(date -u +%Y-%m-%d) из \`Cargo.lock\` и \`app/package-lock.json\`. Компоненты графа normal-зависимостей (без dev/build), включая proc-macro, которые используются только при компиляции и в бинарь не попадают — поэтому число строк больше, чем число крейтов в бинаре по оценке CLO (≈130). Лицензионная оценка — \`docs/SOFTWARE_LICENSE_REGISTER.md\` (CLO)."
  for t in x86_64-pc-windows-msvc aarch64-apple-darwin; do
    echo; echo "## Rust — приложение \`keyswitch\`, target \`$t\`"; echo
    echo "| Пакет | Версия | Лицензия |"; echo "|---|---|---|"
    cargo tree -p keyswitch -e normal --target "$t" --prefix none -f '{p}|{l}' 2>/dev/null \
      | sed 's/ (\*)//; s/ (proc-macro)//' | sort -u \
      | awk -F'|' '{split($1,a," "); v=a[2]; sub(/^v/,"",v); printf "| %s | %s | %s |\n", a[1], v, ($2==""?"(не указана)":$2)}'
  done
  echo; echo "## Данные"; echo
  echo "| Компонент | Версия/источник | Лицензия |"; echo "|---|---|---|"
  echo "| ks-core/data/en.freq, ru.freq | FrequencyWords (OpenSubtitles 2018), top 30 000 | CC-BY-SA-4.0 |"
  echo "| ks-core/layouts/*.layout | по раскладкам Windows 00000409 / 00000419 | собственные данные |"
  echo; echo "## npm — runtime-зависимости UI"; echo
  echo "| Пакет | Версия | Лицензия |"; echo "|---|---|---|"
  python3 - <<'PY'
import json
lock=json.load(open('app/package-lock.json'))
pk=lock.get('packages',{})
for path,m in sorted(pk.items()):
    if not path or m.get('dev'): continue
    print(f"| {path.split('node_modules/')[-1]} | {m.get('version','')} | {m.get('license','(не указана)')} |")
PY
} > "$out"
echo "wrote $out ($(grep -c '^| ' "$out") rows)"
