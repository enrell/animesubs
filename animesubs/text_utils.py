import re


def is_technical_command(text: str) -> bool:
    text = text.strip()
    if re.match(r"^m\s+[\d\s\-\.]+l\s+[\d\s\-\.]+$", text, re.IGNORECASE):
        return True
    if re.match(r"^[mlbsp](\s+[\d\s\-\.]+)+$", text, re.IGNORECASE):
        return True
    if re.match(r"^[\d\s\-\.]+$", text) and len(text.split()) > 2:
        return True
    if re.match(r"^\\[a-zA-Z]+(\([^)]*\))?$", text):
        return True
    return False


def is_meaningful_text(text: str) -> bool:
    text = text.strip()
    if re.match(r"^[^\w]*$", text, re.UNICODE):
        return False
    if re.match(r"^([a-zA-Z])\1*$", text):
        return False
    short_interjections = {
        "ah",
        "oh",
        "eh",
        "uh",
        "hm",
        "mm",
        "ng",
        "sh",
        "ha",
        "he",
        "hi",
        "ho",
        "hu",
        "huh",
        "hmm",
        "aha",
        "ooh",
        "aah",
        "err",
        "umm",
        "uhh",
        "ehh",
        "ohh",
        "whoa",
        "wow",
        "ouch",
        "oof",
    }
    if text.lower() in short_interjections:
        return False
    fragments = {
        "ing",
        "ed",
        "er",
        "ly",
        "tion",
        "ness",
        "ment",
        "ful",
        "less",
        "able",
        "ible",
        "ous",
        "ive",
        "ant",
        "ent",
        "ist",
        "ism",
        "ade",
        "age",
        "ary",
        "ate",
        "dom",
        "ery",
        "fy",
        "ify",
        "ize",
        "ise",
        "ward",
        "wise",
        "like",
        "ship",
        "hood",
    }
    if text.lower() in fragments:
        return False
    return True


def clean_subtitle_text(text: str) -> str:
    if not text or text.strip() == "":
        return ""
    text = re.sub(r"<[^>]+>", "", text)
    text = re.sub(r"{[^}]+}", "", text)
    text = re.sub(r"\\N", " ", text)
    text = re.sub(r"\s+", " ", text).strip()
    if not text:
        return ""
    if is_technical_command(text):
        return ""
    if not is_meaningful_text(text):
        return ""
    return text
