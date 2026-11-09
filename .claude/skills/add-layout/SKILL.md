---
name: add-layout
description: Add a new keyboard layout / language to KeySwitch as data (D-15) without changing core code. Use when asked to support a new language (e.g. Ukrainian, German, French) or a layout variant.
---
# /add-layout <language> <layout-id>

Layouts are data keyed by physical scan code (see `docs/02-architecture.md` §3). Core code must not change; if it seems necessary, stop and raise it with the system-architect.

1. Take the reference mapping from the official OS layout definition (e.g. the Microsoft keyboard layout for Windows); cite the source in the file header.
2. Add the layout data file next to the existing ones, following the EN/RU files exactly: scan code → base and shift characters.
3. Language model: build the n-gram model only from a corpus whose license allows commercial use; record it in `THIRD_PARTY.md`.
4. Tests: round-trip for every mapped key, case preservation, punctuation-context cases, and at least 10 real-phrase cases in the language.
5. UI: add the language to Languages & Layouts via i18n keys only.
6. Run `/check` and report.
