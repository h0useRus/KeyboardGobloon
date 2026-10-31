"""Writes the word lists of the bundled language packs: packs/en/words.txt and packs/ru/words.txt,
one `word count` per line (count = uses per billion words, 0 = unknown), for kbg-pack-builder.

- en: SCOWL word forms up to size 70 (US, UK, Canadian, Australian; words, contractions, names),
  frequencies from wordfreq; also wordfreq's common words SCOWL lacks.
- ru: every OpenCorpora word form (via pymorphy3-dicts-ru), each with ё also spelled with е;
  frequencies from wordfreq; also wordfreq's common words OpenCorpora lacks.

The lists are generated (gitignored); the SCOWL notice is copied to packs/en/LICENSE-SCOWL.txt.
Needs: pip install wordfreq pymorphy3 pymorphy3-dicts-ru
Run from the repository root: python tools/pack-words.py
"""

import pathlib
import re
import tarfile
import urllib.request

import pymorphy3
import wordfreq

PACKS = pathlib.Path("packs")
CACHE = pathlib.Path("target/pack-data")
SCOWL = "https://downloads.sourceforge.net/wordlist/scowl-2020.12.07.tar.gz"
SCOWL_SIZE = 70
SCOWL_FILE = re.compile(
    r"/final/(english|american|british|british_z|canadian|australian)"
    r"-(words|upper|proper-names|contractions)\.(\d+)$"
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


if __name__ == "__main__":
    main()
