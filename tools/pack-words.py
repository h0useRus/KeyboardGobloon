"""Writes the word lists of the language packs, packs/<code>/words.txt,
one `word count` per line (count = uses per billion words, 0 = unknown), for kbg-pack-builder.

- en: SCOWL word forms up to size 70 (US, UK, Canadian, Australian; words, contractions, names),
  frequencies from wordfreq; also wordfreq's common words SCOWL lacks.
- ru: every OpenCorpora word form (via pymorphy3-dicts-ru), each with ё also spelled with е;
  frequencies from wordfreq; also wordfreq's common words OpenCorpora lacks.
- es, fr, de, pl: the LibreOffice Hunspell dictionaries (as packaged by wooorm/dictionaries)
  expanded into every word form (stems x suffixes x prefixes, two suffix levels), frequencies from
  wordfreq; also wordfreq's common words the dictionary lacks (German compounds, mostly).

The lists are generated (gitignored); the SCOWL and Hunspell notices are copied to
packs/<code>/LICENSE-SCOWL.txt and LICENSE-HUNSPELL.txt.
Needs: pip install -r tools/pack-requirements.txt
Run from the repository root: python tools/pack-words.py
"""

import pathlib
import re
import tarfile
import urllib.request

import pymorphy3
import wordfreq
from spylls.hunspell import Dictionary

PACKS = pathlib.Path("packs")
CACHE = pathlib.Path("target/pack-data")
SCOWL = "https://downloads.sourceforge.net/wordlist/scowl-2020.12.07.tar.gz"
SCOWL_SIZE = 70
SCOWL_FILE = re.compile(
    r"/final/(english|american|british|british_z|canadian|australian)"
    r"-(words|upper|proper-names|contractions)\.(\d+)$"
)
# A fixed commit of wooorm/dictionaries, so every build gets the same words.
HUNSPELL = (
    "https://raw.githubusercontent.com/wooorm/dictionaries/"
    "8cfea406b505e4d7df52d5a19bce525df98c54ab/dictionaries/{code}/{file}"
)
# wordfreq words missing from the dictionary are added from this Zipf value on (about 3 uses
# per million words): common enough to be real words, not typos or foreign names.
COMMON = 3.5


def scowl_words() -> set[str]:
    CACHE.mkdir(parents=True, exist_ok=True)
    archive = CACHE / "scowl.tar.gz"
    if not archive.exists():
        urllib.request.urlretrieve(SCOWL, archive)
    words = set()
    with tarfile.open(archive) as tar:
        for member in tar.getmembers():
            if member.name.endswith("/Copyright"):
                text = tar.extractfile(member).read().decode("utf-8")
                (PACKS / "en" / "LICENSE-SCOWL.txt").write_text(text, encoding="utf-8", newline="\n")
            match = SCOWL_FILE.search(member.name)
            if match and int(match.group(3)) <= SCOWL_SIZE:
                text = tar.extractfile(member).read().decode("iso-8859-1")
                words.update(w.lower() for w in text.split())
    return words


def opencorpora_words() -> set[str]:
    words = set()
    for word, *_ in pymorphy3.MorphAnalyzer().dictionary.iter_known_words():
        words.add(word)
        if "ё" in word:
            words.add(word.replace("ё", "е"))
    return words


def hunspell_words(code: str) -> set[str]:
    cache = CACHE / code
    cache.mkdir(parents=True, exist_ok=True)
    for file in ("index.aff", "index.dic", "license"):
        if not (cache / file).exists():
            urllib.request.urlretrieve(HUNSPELL.format(code=code, file=file), cache / file)
    (PACKS / code / "LICENSE-HUNSPELL.txt").write_text(
        (cache / "license").read_text(encoding="utf-8"), encoding="utf-8", newline="\n"
    )
    dictionary = Dictionary.from_files(str(cache / "index"))
    aff = dictionary.aff
    # Forms that are no words on their own: parts of compounds, or needing another affix.
    skip = {f for f in (aff.NEEDAFFIX, aff.ONLYINCOMPOUND, aff.FORBIDDENWORD) if f}

    def suffixed(stem: str, flags) -> list[tuple[str, object]]:
        out = []
        for flag in flags:
            for sfx in aff.SFX.get(flag, []):
                if sfx.cond_regexp.search(stem) and stem.endswith(sfx.strip):
                    out.append((stem[: len(stem) - len(sfx.strip)] + sfx.add, sfx))
        return out

    def prefixed(stem: str, flags, cross_only: bool) -> list[str]:
        out = []
        for flag in flags:
            for pfx in aff.PFX.get(flag, []):
                if cross_only and not pfx.crossproduct:
                    continue
                if pfx.cond_regexp.search(stem) and stem.startswith(pfx.strip):
                    out.append(pfx.add + stem[len(pfx.strip) :])
        return out

    words = set()
    for word in dictionary.dic.words:
        flags = set(word.flags)
        if not flags & skip:
            words.add(word.stem)
        for form, sfx in suffixed(word.stem, flags):
            if not set(sfx.flags) & skip:
                words.add(form)
            # Twofold suffixes: the suffix brings its own suffixes.
            words.update(f for f, s in suffixed(form, sfx.flags) if not set(s.flags) & skip)
            if sfx.crossproduct:
                words.update(prefixed(form, flags | set(sfx.flags), True))
        words.update(prefixed(word.stem, flags, False))
    return {w.lower() for w in words}


def write(code: str, words: set[str]) -> None:
    freq = wordfreq.get_frequency_dict(code)
    words |= {w for w, f in freq.items() if f >= 10 ** (COMMON - 9)}
    path = PACKS / code / "words.txt"
    with path.open("w", encoding="utf-8", newline="\n") as out:
        for word in sorted(words):
            out.write(f"{word} {round(freq.get(word, 0) * 1e9)}\n")
    print(f"{path}: {len(words)} words")


def main() -> None:
    write("en", scowl_words())
    write("ru", opencorpora_words())
    for code in ("es", "fr", "de", "pl"):
        write(code, hunspell_words(code))


if __name__ == "__main__":
    main()
