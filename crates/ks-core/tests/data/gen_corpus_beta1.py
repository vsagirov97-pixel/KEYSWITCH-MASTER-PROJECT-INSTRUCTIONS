#!/usr/bin/env python3
"""Generates corpus_beta1.tsv (BL-QA-B1): synthetic wrong-layout cases for ks-core.

Run from the repository root:  python3 crates/ks-core/tests/data/gen_corpus_beta1.py
All text is invented test data; no real user text. The expected value is written by hand
(the intended text); only the "typed in the wrong layout" input is derived from the layouts.

Modes:  phys = every key pressed physically as on the intended layout (punctuation moves too)
        lit  = letters mistyped, punctuation and digits appear literally (PRD FR-CONV-3 style)
        keep = input is already right and must stay unchanged
"""
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[4]
LAYOUTS = ROOT / "crates/ks-core/layouts"


def load(name):
    to_char, to_key = {}, {}
    for line in (LAYOUTS / name).read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("//"):
            continue
        sc, base, shift = line.split()
        for sh, ch in ((False, base), (True, shift)):
            to_char[(sc, sh)] = ch
            to_key[ch] = (sc, sh)
    return to_char, to_key


EN, RU = load("en-us.layout"), load("ru.layout")


def typed(intended, intended_layout, active_layout, mode):
    out = []
    for ch in intended:
        key = intended_layout[1].get(ch)
        if key is None or (mode == "lit" and not ch.isalpha()):
            out.append(ch)
        else:
            out.append(active_layout[0][key])
    return "".join(out)


CASES = []  # (category, input, expected)


def e2r(cat, mode, items):
    """Russian intended, typed while the English layout was active."""
    for s in items:
        CASES.append((cat, typed(s, RU, EN, mode), s))


def r2e(cat, mode, items):
    """English intended, typed while the Russian layout was active."""
    for s in items:
        CASES.append((cat, typed(s, EN, RU, mode), s))


def keep(cat, items):
    for s in items:
        CASES.append((cat, s, s))


e2r("word_en2ru", "phys", """привет спасибо пожалуйста сегодня завтра работа человек время
большой хорошо компьютер клавиатура раскладка документ проверка вопрос ответ сообщение телефон
встреча договор письмо город страна машина деньги погода музыка книга школа друзья семья
вечером утром никогда всегда потому сейчас очень только можно нужно тоже даже буду себя
программа система новости интересно понедельник""".split())

r2e("word_ru2en", "phys", """hello world thanks please today tomorrow work people time good
computer keyboard layout document check question answer message phone meeting because between
something without another through should important different example morning evening project
review release window button english russian language switch system number family friend school
always never maybe welcome""".split())

E2R_PHRASES = [
    "Привет, как дела?", "Доброе утро!", "Спасибо за помощь.", "Я скоро буду дома.",
    "Позвони мне завтра, пожалуйста.", "Где находится офис?", "Сколько это стоит?",
    "Мы встретимся в пять часов.", "Это очень важно для нас.", "Напиши, когда освободишься.",
    "Отправь мне файл на почту.", "Я не знаю, что делать.", "Всё будет хорошо!",
    "Добрый день, коллеги!", "До свидания.", "Как тебя зовут?", "Мне нужно подумать.",
    "Давай созвонимся вечером.", "Спокойной ночи!", "Хорошего дня",
]
e2r("phrase_en2ru_phys", "phys", E2R_PHRASES)
e2r("phrase_en2ru_lit", "lit", E2R_PHRASES[:12])

R2E_PHRASES = [
    "Hello, how are you?", "Good morning!", "Thanks for your help.", "I will be home soon.",
    "Call me tomorrow, please.", "Where is the office?", "How much does it cost?",
    "See you at five o'clock.", "This is very important.", "Let me know when you're free.",
    "I don't know.", "It's a good idea.", "What's the time?", "Send me the file, please.",
    "Have a nice day!", "Nice to meet you.", "Can you help me?", "Best regards",
    "Thank you very much!", "See you later.",
]
r2e("phrase_ru2en_phys", "phys", R2E_PHRASES)
r2e("phrase_ru2en_lit", "lit", R2E_PHRASES[:12])

# Letters that live on punctuation keys: б ю ж э х ъ ё  (, . ; ' [ ] `)
e2r("punct_keys_en2ru", "phys", """люблю обычно жизнь это хлеб объект съезд юрист экран хотя
работаю подъезд бюджет объявление жёлтый необходимо любой каждый уже был ещё ёлка хорошо.
эхо жюри объём съёмка бюро убеждён""".split())
# Beta 2: sentence punctuation at the end of the text. `ytn?` is `нет?` typed literally or
# `нет,` typed physically; at the end of the text `?` `!` `.` after a dictionary word are kept
# as sentence punctuation (`нет?`, `да.`, `привет.`).
CASES += [("punct_sentence_end", typed(s, RU, EN, "lit"), s) for s in [
    "нет?", "да.", "да!", "привет.", "тест.", "конечно.", "спасибо!", "почему?", "где?",
    "как?", "хорошо?", "сейчас.", "завтра?", "нет!", "всё.",
]]
# A letter still wins when the word without the mark is not a dictionary word, and also when
# both readings are dictionary words (`ltkf.` -> `делаю`, as `hf,jnf.` -> `работаю` above;
# QA-B1-03): the corpus has no evidence that the sentence mark is more likely there.
e2r("punct_sentence_end", "phys", ["знаю", "люблю", "этаж", "делаю", "помощью"])
e2r("punct_keys_en2ru", "lit", ["нет.", "да, конечно", "люблю, обычно.", "это просто!",
                                 "хорошо; ладно", "(хлеб)", "\"жизнь\"", "объект: экран"])
# English punctuation typed while Russian was active turns into letters.
r2e("punct_keys_ru2en", "phys", ["don't", "it's", "i'm", "we're", "can't", "that's", "[draft]",
                                  "items; more", "yes, no", "end.", "a, b, c", "wait...",
                                  "one; two; three", "list[0]", "o'clock", "well, okay.",
                                  "'quoted'", "first, second", "ok.", "hi, there"])

e2r("digits", "phys", ["Тест 123", "2 часа", "число 3.14", "100 рублей", "5 минут", "дом 7",
                        "версия 2.0", "25 декабря", "8 марта", "1 января 2025 года",
                        "в 15:30", "этаж 3, офис 12"])
e2r("digits", "lit", ["в 15:30", "этаж 3, офис 12", "скидка 50%", "номер 7"])
r2e("digits", "phys", ["Test 123", "room 12", "version 2.0", "at 10 am", "5 minutes", "top 10",
                        "24 hours", "page 3", "3 times", "100 percent", "call 911", "chapter 7"])

e2r("mixed_case", "phys", ["ПРИВЕТ", "Москва", "ПРИВЕТ мир", "Иван Петров", "СРОЧНО",
                            "Добрый День", "НЕ надо", "Да", "Нет", "Санкт-Петербург",
                            "ВАЖНО", "Россия", "Спасибо Большое"])
r2e("mixed_case", "phys", ["HELLO", "London", "Hello World", "OK", "John Smith", "URGENT",
                            "Yes", "No", "THANK YOU", "New York", "Monday", "iPhone",
                            "Dear Anna"])

e2r("short_en2ru", "phys", """я и в не на да он мы ты но за из по что как так все его для или
вот где кто нет там""".split())
r2e("short_ru2en", "phys", """i a is it to of in on we he me my no ok the and you are for not
but yes can how why""".split())

# Beta 2: short words (1-3 letters) in caret mode. Correct short words must stay; wrong-layout
# short words should convert. One-letter words are ambiguous by nature (`a` is English, `ф`
# is not a Russian word, but `f` -> `а` is): the numbers are measured, not assumed.
SHORT_RU = """я в и а о у к с не на да он мы ты но за из по что как так все его для или вот где кто
нет там она они это ещё уж же ли бы мой""".split()
SHORT_EN = """a I to ok is it in on at of be we do go no so up me my he us an or if as by the and
you are for not but yes can how why all any was has""".split()
keep("short_keep_ru", SHORT_RU)
keep("short_keep_en", SHORT_EN)
e2r("short_fix_1letter", "phys", ["я", "в", "и", "а", "о", "у", "к", "с", "Я", "В", "И", "А"])
r2e("short_fix_1letter", "phys", ["a", "I", "i"])
e2r("short_fix_2_3", "phys", """уж же ли бы мой она они это ещё""".split())
r2e("short_fix_2_3", "phys", """at be do go so up us an or if as by all any was has""".split())

keep("keep_tech", [
    "https://example.com/path?q=1", "http://localhost:8080/api", "www.example.org",
    "ftp://files.example.net", "user@example.com", "test.user@mail.example", "example.com",
    "snake_case_name", "camelCaseName", "getUserById", "MAX_VALUE", "README.md", "index.html",
    "main.rs", "C:\\Users\\test\\file.txt", "/usr/local/bin", "v1.2.3", "localhost:8080",
    "git commit -m", "npm install", "foo.bar()", "x86_64", "#include", "std::fmt::Debug",
    "0xFF", "UTF-8", "id=42&name=x", "a1b2c3", "192.168.0.1", "--help", "SELECT * FROM users",
    "user_id", "app.config.json", "<div>", "printf(\"%d\")",
])

keep("keep_correct_ru", [
    "привет", "спасибо", "хорошо", "сегодня", "люблю", "обычно", "это", "я", "и", "не",
    "Привет, как дела?", "Доброе утро!", "Спасибо за помощь.", "Я скоро буду дома.",
    "Всё будет хорошо!", "Тест 123", "ПРИВЕТ", "Москва", "да, конечно", "До свидания.",
    "объявление", "жизнь", "ёлка",
])
keep("keep_correct_en", [
    "hello", "thanks", "good", "today", "don't", "keyboard", "the", "i", "a", "ok",
    "Hello, how are you?", "Good morning!", "Thanks for your help.", "I will be home soon.",
    "It's a good idea.", "Test 123", "HELLO", "London", "yes, no", "See you later.",
    "because", "something", "welcome",
])

# Correct bilingual text: nothing may change.
keep("mixed_lang_keep", [
    "привет world", "Hello мир", "отправь email сегодня", "мой iPhone сломался",
    "встреча в Zoom в 10", "файл README лежит тут", "купить ticket онлайн",
    "версия для Windows и macOS", "напиши в Telegram", "это OK", "меню File, пункт Save",
    "пароль от Wi-Fi", "скачай update", "наш office в Москве", "USB кабель",
])
# Part of the line is in the wrong layout. Beta 1 refused these (MIXED); Beta 2 converts the
# confidently wrong tokens per token and asks only when a token is unclear.
CASES += [
    ("mixed_lang_fix", "ghbdtn мир", "привет мир"),
    ("mixed_lang_fix", "привет vbh", "привет мир"),
    ("mixed_lang_fix", "hello цщкдв", "hello world"),
    ("mixed_lang_fix", "руддщ world", "hello world"),
    ("mixed_lang_fix", "я ljvf", "я дома"),
    ("mixed_lang_fix", "ок, cgfcb,j", "ок, спасибо"),
    ("mixed_lang_fix", "good ьщктштп", "good morning"),
    ("mixed_lang_fix", "да, rjytxyj", "да, конечно"),
    ("mixed_lang_fix", "ghbdtn, как дела?", "привет, как дела?"),
    ("mixed_lang_fix", "see нщг later", "see you later"),
]



def mistype(intended, wrong):
    """Words at indexes `wrong` typed in the other layout (letters only, punctuation literal)."""
    out = []
    for i, w in enumerate(intended.split(" ")):
        if i in wrong:
            ru = any("а" <= c.lower() <= "я" or c.lower() == "ё" for c in w)
            w = typed(w, RU, EN, "lit") if ru else typed(w, EN, RU, "lit")
        out.append(w)
    return " ".join(out)


# Beta 2: per-token decision on mixed text (synthetic). Wrong words are converted, correct
# words, technical tokens and digits stay; the popup is the fallback when a token is unclear.
for intended, wrong in [
    ("я сегодня буду дома", [1]), ("мы идём в кино", [1]), ("встреча завтра утром", [2]),
    ("это очень хорошо", [1, 2]), ("привет, как дела", [2]), ("hello my friend", [2]),
    ("good night everyone", [1]), ("thank you very much", [3]), ("please call me", [1]),
    ("see you tomorrow", [2]), ("the meeting is today", [1]), ("открой файл project", [1]),
    ("версия 2.0 готова", [2]), ("отправь на user@example.com сегодня", [3]),
    ("файл README.md открыт", [2]), ("позвони в 15:30", [0]), ("работа в Windows", [0]),
    ("спасибо, John", [0]), ("my name is Иван", [0]), ("сделай git commit", [0]),
    ("ok, понял", [1]), ("check почту please", [2]),
]:
    CASES.append(("mixed_lang_fix_b2", mistype(intended, wrong), intended))
keep("mixed_lang_keep_b2", [
    "звони на номер 123", "открой https://example.com сейчас", "мой email user@example.com",
    "функция getUserById готова", "запусти npm install", "версия v1.2.3 вышла", "я в office",
    "a и b", "Привет, Anna!", "тест OK", "файл config.json", "the слово",
])

seen = set()
lines = ["# BL-QA-B1 conversion corpus. Generated by gen_corpus_beta1.py - do not edit by hand.",
         "# Synthetic test data only. Columns: category<TAB>input<TAB>expected"]
for cat, inp, exp in CASES:
    assert "\t" not in inp + exp and "\n" not in inp + exp
    if (cat, inp) in seen:
        continue
    seen.add((cat, inp))
    lines.append(f"{cat}\t{inp}\t{exp}")
out = pathlib.Path(__file__).with_name("corpus_beta1.tsv")
out.write_text("\n".join(lines) + "\n", encoding="utf-8")
print(len(lines) - 2, "cases ->", out)
