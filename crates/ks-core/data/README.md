# Language model data

| File | Source | License |
|------|--------|---------|
| `en.freq`, `ru.freq` | Top 30 000 words (letters only) from [FrequencyWords](https://github.com/hermitdave/FrequencyWords) `content/2018/{en,ru}/*_50k.txt`, built from OpenSubtitles 2018, by Hermit Dave | **CC-BY-SA-4.0** |

The files are data, distributed under their own license, separately from KeySwitch code.
**Release blocker (R-7):** the licensing approach for shipped model data must be approved by the owner before release — see `docs/spikes/S-1-converter-model.md`.

Regenerate: keep lines matching `^[а-яё]+(-[а-яё]+)?$` (ru) or `^[a-z]+('[a-z]+)?(-[a-z]+)?$` (en), first 30 000, format `word count`.
