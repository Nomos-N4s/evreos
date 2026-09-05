#!/usr/bin/env python3
"""Enforce FR-035's separation of language and place, over the whole tree.

WHAT THIS CHECKS, and why each clause exists.

FR-035: interface text is keyed by the BCP-47 primary language subtag alone --
`de`, `el`, `en` -- with no region subtag in the key and place never fused
into the language value, and language and place are two separate values
wherever either appears: in stored preferences, in interface state, and in
every request Evreos makes to an Apivo service. "Keyed by language alone"
would on its own be satisfied by `de-DE`, which re-fuses the two; that
spelling is exactly what this check exists to refuse. It lands in the
foundational phase because it guards every catalogue key and every request
builder the later phases write, not only the ones that exist today. It reads
the tree and fails on:

  CATALOGUE NAME  a file in any `catalogues/` directory whose name, up to its
                  first dot, is not a bare primary language subtag -- two or
                  three lowercase ASCII letters, nothing else. This is
                  stricter than refusing region subtags alone, deliberately:
                  BCP-47 is case-insensitive, so `de-de.ftl` and `de_DE.json`
                  are the same fused tag in two spellings, and enumerating
                  spellings is how one is missed. A catalogue directory holds
                  catalogues named by subtag and nothing else; a stray file
                  there fails as unnameable rather than passing as clutter.

  CATALOGUE KEY   a message key carrying a fused language-place value, read
                  from every line of every file in a `catalogues/` directory
                  that spells `key = ...` outside a `#` comment -- which is
                  how both the adopted plain keyed table and Fluent spell a
                  message, so the clause holds across a format change. A key
                  like `wallet.de-AT.title` fails here.

  FUSED VALUE     a fused language-place value in Rust source or in a TOML
                  file: `de-DE` in a string literal, `locale=de-DE` in a
                  request builder's query string, `en_us` in a manifest or a
                  configuration value. Two patterns, matched everywhere:
                  any 2-3-letter lowercase subtag joined by `-` or `_` to a
                  canonically-spelled region -- two uppercase letters or
                  three digits -- and, because BCP-47 folds case, the three
                  shipped languages `de`, `el` and `en` joined to a region in
                  ANY case. Rust source is read with comments stripped and
                  literals kept, through the one shared scanner, so a doc
                  comment naming `de-DE` as the forbidden example is not a
                  breach and a string carrying it is.

  FUSED FIELD     the two spellings of building one field from the two
                  values that a scanner can honestly see: a format string
                  interpolating a language-named placeholder and a
                  place-named placeholder joined by `-` or `_`, in either
                  order; and a line that joins identifiers named for
                  language and for place with a bare "-" or "_" literal or a
                  `{}-{}` format -- `format!("{}-{}", language, place)`,
                  `[language, place].join("-")`. A stored preference or an
                  interface-state field built either way is the serialisation
                  FR-035 forbids, whatever the field is called.

An Apivo request builder whose language parameter carries a region falls to
the FUSED VALUE and FUSED FIELD clauses when it is spelled in the tree --
`locale=de-DE` is a fused value on the line that builds it. The deeper
guarantee is structural and lives in crates/evreos-i18n: the only language
type is a closed enum whose serialised form is the bare subtag, so a builder
that takes its language from that crate cannot emit a region at all.

WHAT THIS DOES NOT CATCH, stated so nothing is assumed of it.

A fusion composed across lines, or through variables whose names say nothing
of language or place, passes the FUSED FIELD heuristic and rests on review --
and on the closed enum above, which is why the enum exists. A region
smuggled in as its own key segment (`menu.at.title` meaning Austria) is
semantics no scanner can see. A fused tag in a catalogue message's VALUE is
prose, not a key, and is not read -- the member-facing text may legitimately
never need to name one, but that is the translator's judgement, not this
check's. Fourth-language fusions spelled entirely in lowercase (`fr-fr`)
pass the literal clauses until that language ships and joins the shipped
list here, in the same change that adds its catalogue. Files other than Rust
source, TOML and catalogue directories are not read: workflows build no
Apivo requests, and markdown is where the forbidden examples are quoted.

Rust source is read with comments stripped through rustlex, the one Rust
scanner; directories are matched with case folded where they must be, the
release platforms' filesystems folding case. Dot-directories and `target/`
are not read: nothing under either ships.
"""
import argparse
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent

sys.path.insert(0, str(HERE))
from casefs import folded_in, is_rust_source  # noqa: E402
from rustlex import strip_non_code  # noqa: E402

# The directory name a crate keeps its catalogues under, matched with case
# folded like every other name the filesystem is asked for.
CATALOGUE_DIR = "catalogues"

# A bare primary language subtag: what a catalogue file's stem must be.
SUBTAG = re.compile(r"^[a-z]{2,3}$")

# A fused language-place value in its canonical spelling: a primary subtag
# joined to a region subtag as BCP-47 canonically cases one -- two uppercase
# letters or three digits. `de-DE`, `de_AT`, `es-419`, `locale=de-DE`.
CANONICAL_FUSION = re.compile(
    r"(?<![A-Za-z0-9])[a-z]{2,3}[-_](?:[A-Z]{2}|[0-9]{3})(?![A-Za-z0-9])"
)

# The same fusion for the three shipped languages with the region in ANY
# case, because BCP-47 folds case and `de-de` is `de-DE` in another spelling.
# Restricted to the shipped subtags so that ordinary hyphenated words --
# `to-do`, `opt-in` -- are not read as tags; a fourth language extends this
# alternation in the change that adds its catalogue.
SHIPPED_FUSION = re.compile(
    r"(?<![A-Za-z0-9])(?:de|el|en)[-_](?:[A-Za-z]{2}|[0-9]{3})(?![A-Za-z0-9])"
)

# A format string that fuses a language-named placeholder and a place-named
# placeholder into one value, in either order: `"{language}-{place}"`,
# `"{place}_{lang}"`.
LANG_HINT = r"[^{}]*lang[^{}]*"
PLACE_HINT = r"[^{}]*(?:place|region|country|territory)[^{}]*"
FORMAT_FUSIONS = (
    re.compile(r"\{" + LANG_HINT + r"\}\s*[-_]\s*\{" + PLACE_HINT + r"\}", re.IGNORECASE),
    re.compile(r"\{" + PLACE_HINT + r"\}\s*[-_]\s*\{" + LANG_HINT + r"\}", re.IGNORECASE),
)

# The join spelling of the same fusion: a line that holds a bare "-" or "_"
# literal, or a positional `{}-{}` format, beside identifiers named for
# language and for place. `format!("{}-{}", language, place)`,
# `[language, place].join("-")`, `lang + "-" + place`.
JOIN_GLUE = re.compile(r'"[-_]"|\{\}[-_]\{\}')
LANG_IDENT = re.compile(r"\w*lang\w*", re.IGNORECASE)
PLACE_IDENT = re.compile(r"\w*(?:place|region|country|territory)\w*", re.IGNORECASE)


def read_text(path):
    """The file's text, or None when it is not UTF-8; the caller reports.

    The BOM is stripped for the reason the other checks strip it: an editor
    that writes one is not a way past a check.
    """
    try:
        return path.read_text(encoding="utf-8").lstrip("﻿")
    except UnicodeDecodeError:
        return None


def fused_values(text):
    """Every fused language-place value in `text`, in order, deduplicated."""
    found = []
    for pattern in (CANONICAL_FUSION, SHIPPED_FUSION):
        for match in pattern.finditer(text):
            if match.group(0) not in found:
                found.append(match.group(0))
    return found


def check_catalogue_file(where, name, text, problems):
    """The CATALOGUE NAME and CATALOGUE KEY clauses over one file."""
    stem = name.split(".", 1)[0]
    if not SUBTAG.match(stem):
        fused = fused_values(name)
        if fused:
            problems.append(
                f"{where}: catalogue filename carries the fused value "
                f"{fused[0]!r}; FR-035 names a catalogue by the primary "
                "language subtag alone"
            )
        else:
            problems.append(
                f"{where}: not named by a primary language subtag alone; a "
                "catalogue directory holds one file per language, named "
                "`de`, `el`, `en`"
            )
    if text is None:
        problems.append(f"{where}: not valid UTF-8, so its keys cannot be read")
        return 0
    keys = 0
    for number, line in enumerate(text.splitlines(), 1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        key, separator, _ = stripped.partition("=")
        if not separator:
            continue
        keys += 1
        for fused in fused_values(key.strip()):
            problems.append(
                f"{where}:{number}: message key carries the fused value "
                f"{fused!r}; a key is language-neutral and a catalogue is "
                "keyed by subtag alone"
            )
    return keys


def check_source_line(where, number, line, problems):
    """The FUSED VALUE and FUSED FIELD clauses over one line of Rust."""
    for fused in fused_values(line):
        problems.append(
            f"{where}:{number}: fused language-place value {fused!r}; language "
            "and place are two values, in requests as everywhere else"
        )
    for pattern in FORMAT_FUSIONS:
        match = pattern.search(line)
        if match:
            problems.append(
                f"{where}:{number}: format string fuses language and place "
                f"into one value ({match.group(0)!r})"
            )
    if (
        JOIN_GLUE.search(line)
        and LANG_IDENT.search(line)
        and PLACE_IDENT.search(line)
    ):
        problems.append(
            f"{where}:{number}: joins language and place into one field; "
            "FR-035 keeps them as two values in every stored preference, "
            "interface-state field and request"
        )


def check_tree(root):
    """Every clause over the tree at `root`.

    Returns (problems, catalogue files read, message keys read, source files
    read). An empty `problems` is a pass -- unless nothing at all was read,
    which is reported, because a check over nothing is not a pass.
    """
    root = Path(root).resolve()
    problems = []
    catalogue_files = 0
    message_keys = 0
    source_files = 0

    if not root.is_dir():
        problems.append(f"{root}: not a directory this check can read")
        return problems, 0, 0, 0

    for directory in sorted(walk(root)):
        for path in sorted(directory.iterdir()):
            if not path.is_file():
                continue
            where = path.relative_to(root).as_posix()
            in_catalogues = folded_in(directory.name, [CATALOGUE_DIR])
            if in_catalogues:
                catalogue_files += 1
                message_keys += check_catalogue_file(
                    where, path.name, read_text(path), problems
                )
                continue
            if is_rust_source(path):
                source_files += 1
                text = read_text(path)
                if text is None:
                    problems.append(f"{where}: not valid UTF-8, so it is not Rust this check can read")
                    continue
                code = strip_non_code(text, keep_literals=True)
                for number, line in enumerate(code.splitlines(), 1):
                    check_source_line(where, number, line, problems)
            elif path.suffix.lower() == ".toml":
                text = read_text(path)
                if text is None:
                    problems.append(f"{where}: not valid UTF-8, so it is not TOML this check can read")
                    continue
                for number, line in enumerate(text.splitlines(), 1):
                    if line.lstrip().startswith("#"):
                        continue
                    for fused in fused_values(line):
                        problems.append(
                            f"{where}:{number}: fused language-place value "
                            f"{fused!r}; a stored preference or configuration "
                            "value carries language and place separately"
                        )

    if catalogue_files == 0 and source_files == 0:
        problems.append(
            f"{root}: no catalogue file and no Rust source; a check over "
            "nothing is not a pass"
        )
    return problems, catalogue_files, message_keys, source_files


def walk(root):
    """Every directory under `root` this check reads, `root` included.

    Dot-directories and `target/` are pruned: nothing under either ships,
    and `target/` holds every vendored dependency's source, which is not
    this tree's to fix. The fold on `target` is casefs's rule -- `TARGET/`
    is the same directory on the platforms that build the release.
    """
    kept = [root]
    for directory in kept:
        for path in sorted(directory.iterdir()):
            if not path.is_dir():
                continue
            if path.name.startswith(".") or folded_in(path.name, ["target"]):
                continue
            kept.append(path)
    return kept


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root", default=str(REPO), help="tree to check; the repository by default"
    )
    args = parser.parse_args()

    problems, catalogue_files, message_keys, source_files = check_tree(args.root)

    if problems:
        print("Language-place check FAILED:\n", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        print(
            "\nSee the docstring of scripts/checks/check_language_place.py for "
            "the clauses and what satisfies each.",
            file=sys.stderr,
        )
        return 1

    print(
        f"Language-place check passed: {catalogue_files} catalogue files, "
        f"{message_keys} message keys, {source_files} Rust source files."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
