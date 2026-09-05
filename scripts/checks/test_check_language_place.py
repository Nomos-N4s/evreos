#!/usr/bin/env python3
"""Tests for the language-place check. The check is CI's authority to fail a
build over a catalogue name, a message key or a request builder, so its own
behaviour is checked rather than assumed -- above all that it FAILS the four
shapes T030 names: a passing tree passes, a `de-DE.ftl` filename fails, a
`wallet.de-AT.title` key fails, and a request builder emitting `locale=de-DE`
fails. The fused counter-examples in this file are Python string literals;
the check reads Rust source, TOML and catalogue directories, never Python,
so quoting them here is not a breach and needs no assembly trick.

Run: python3 scripts/checks/test_check_language_place.py
"""
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_language_place as check  # noqa: E402

PASSED = FAILED = 0


def report(name, condition):
    global PASSED, FAILED
    if condition:
        PASSED += 1
    else:
        FAILED += 1
        print(f"FAIL: {name}", file=sys.stderr)


# --- fixtures -----------------------------------------------------------------

CLEAN_CATALOGUE = (
    "# a comment\n"
    "error.unresolvable.cause = The address {address} could not be found.\n"
    "error.unresolvable.next_step = Check the spelling and try again.\n"
    "menu.home_surface = Apps\n"
)

CLEAN_RUST = (
    "#![forbid(unsafe_code)]\n"
    "pub fn label() -> &'static str {\n"
    '    "opt-in to-do next_step"\n'
    "}\n"
)


def tree(files):
    """Build the files in a temporary tree and run the check over it.

    `files` maps a relative POSIX path to its content; bytes are written raw,
    so a test can plant a file the check cannot decode. Returns what
    `check_tree` returns.
    """
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        for relative, content in files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if isinstance(content, bytes):
                path.write_bytes(content)
            else:
                path.write_text(content, encoding="utf-8")
        return check.check_tree(root)


def passing_tree(extra=None):
    files = {
        "crates/x/catalogues/de.messages": CLEAN_CATALOGUE,
        "crates/x/catalogues/el.messages": CLEAN_CATALOGUE,
        "crates/x/catalogues/en.messages": CLEAN_CATALOGUE,
        "crates/x/src/lib.rs": CLEAN_RUST,
        "crates/x/Cargo.toml": '[package]\nname = "x"\n',
    }
    if extra:
        files.update(extra)
    return files


def mentions(problems, *fragments):
    return any(all(fragment in problem for fragment in fragments) for problem in problems)


# --- the repository itself ---------------------------------------------------

problems, catalogues, keys, sources = check.check_tree(check.REPO)
report("the repository passes", problems == [])
report("...having read the three catalogues", catalogues == 3)
report("...their message keys", keys > 0)
report("...and the workspace's Rust source", sources > 0)

# --- a passing tree -----------------------------------------------------------

problems, catalogues, keys, sources = tree(passing_tree())
report("a clean tree passes", problems == [])
report("...reading its catalogue files", catalogues == 3)
report("...its message keys", keys == 9)
report("...and its Rust source", sources == 1)

# --- CATALOGUE NAME -----------------------------------------------------------

problems, _, _, _ = tree(passing_tree({"crates/x/catalogues/de-DE.ftl": CLEAN_CATALOGUE}))
report("a de-DE.ftl catalogue filename fails", problems != [])
report("...naming the fused value", mentions(problems, "de-DE", "filename"))

problems, _, _, _ = tree(passing_tree({"crates/x/catalogues/de_DE.messages": CLEAN_CATALOGUE}))
report("an underscore spelling of the same fusion fails", mentions(problems, "de_DE"))

problems, _, _, _ = tree(passing_tree({"crates/x/catalogues/de-de.messages": CLEAN_CATALOGUE}))
report("a lowercase region is the same fused tag and fails", problems != [])

problems, _, _, _ = tree(passing_tree({"crates/x/catalogues/README.md": "# notes\n"}))
report("a stray file in a catalogue directory fails as unnameable",
       mentions(problems, "README.md", "subtag"))

problems, _, _, _ = tree(passing_tree({"crates/x/catalogues/deu.messages": CLEAN_CATALOGUE}))
report("a three-letter primary subtag alone is a legal name", problems == [])

# --- CATALOGUE KEY ------------------------------------------------------------

problems, _, _, _ = tree(passing_tree({
    "crates/x/catalogues/de.messages": CLEAN_CATALOGUE + "wallet.de-AT.title = Konto\n",
}))
report("a wallet.de-AT.title key fails", problems != [])
report("...naming the key's fused value and its line",
       mentions(problems, "de-AT", "catalogues/de.messages:5"))

problems, _, _, _ = tree(passing_tree({
    "crates/x/catalogues/en.messages": CLEAN_CATALOGUE + "wallet.en-us.title = Account\n",
}))
report("a lowercase-region key fails for a shipped language", mentions(problems, "en-us"))

problems, _, _, _ = tree(passing_tree({
    "crates/x/catalogues/es.messages": "wallet.es-419.title = Cuenta\n",
}))
report("a numeric region subtag in a key fails", mentions(problems, "es-419"))

problems, _, _, _ = tree(passing_tree({
    "crates/x/catalogues/de.messages":
        "# de-DE is quoted in this comment and quoted only\n" + CLEAN_CATALOGUE,
}))
report("a catalogue comment quoting the forbidden example passes", problems == [])

# --- FUSED VALUE in Rust source -----------------------------------------------

problems, _, _, _ = tree(passing_tree({
    "crates/x/src/request.rs":
        "pub fn url(base: &str) -> String {\n"
        '    format!("{base}?locale=de-DE")\n'
        "}\n",
}))
report("a request builder emitting locale=de-DE fails", problems != [])
report("...naming the fused value and the file",
       mentions(problems, "de-DE", "src/request.rs:2"))

problems, _, _, _ = tree(passing_tree({
    "crates/x/src/state.rs": 'pub const DEFAULT: &str = "en_US";\n',
}))
report("a fused default in interface state fails", mentions(problems, "en_US"))

problems, _, _, _ = tree(passing_tree({
    "crates/x/src/lib.rs":
        "#![forbid(unsafe_code)]\n"
        "// never de-DE: the region belongs in Place\n"
        "/// and a doc example naming de-AT is documentation\n"
        'pub const SUBTAG: &str = "de";\n',
}))
report("a Rust comment quoting the forbidden example passes", problems == [])

problems, _, _, _ = tree(passing_tree({
    "crates/x/src/lib.rs":
        "#![forbid(unsafe_code)]\n"
        'pub const WORDS: &str = "opt-in check-in en-route to-do";\n',
}))
report("ordinary hyphenated words are not read as tags", problems == [])

# --- FUSED FIELD --------------------------------------------------------------

problems, _, _, _ = tree(passing_tree({
    "crates/x/src/prefs.rs":
        "pub fn stored(language: &str, place: &str) -> String {\n"
        '    format!("{language}-{place}")\n'
        "}\n",
}))
report("a format string fusing language and place fails",
       mentions(problems, "format string fuses"))

problems, _, _, _ = tree(passing_tree({
    "crates/x/src/prefs.rs":
        "pub fn stored(place: &str, lang: &str) -> String {\n"
        '    format!("{place}_{lang}")\n'
        "}\n",
}))
report("...in either order and either glue", problems != [])

problems, _, _, _ = tree(passing_tree({
    "crates/x/src/prefs.rs":
        "pub fn stored(language: &str, place: &str) -> String {\n"
        '    format!("{}-{}", language, place)\n'
        "}\n",
}))
report("the positional spelling of the fusion fails", mentions(problems, "joins language and place"))

problems, _, _, _ = tree(passing_tree({
    "crates/x/src/prefs.rs":
        "pub fn stored(language: &str, region: &str) -> String {\n"
        '    [language, region].join("-")\n'
        "}\n",
}))
report("the join spelling of the fusion fails", mentions(problems, "joins language and place"))

problems, _, _, _ = tree(passing_tree({
    "crates/x/src/range.rs":
        "pub fn span(start: u32, end: u32) -> String {\n"
        '    format!("{start}-{end}")\n'
        "}\n",
}))
report("a hyphenated range of unrelated values passes", problems == [])

# --- TOML ---------------------------------------------------------------------

problems, _, _, _ = tree(passing_tree({
    "crates/x/Cargo.toml": '[package]\nname = "x"\nmetadata_locale = "de-DE"\n',
}))
report("a fused value in a TOML file fails", mentions(problems, "de-DE", "Cargo.toml:3"))

problems, _, _, _ = tree(passing_tree({
    "crates/x/Cargo.toml": '[package]\n# de-DE is quoted here and quoted only\nname = "x"\n',
}))
report("a TOML full-line comment quoting the example passes", problems == [])

# --- what is not read ---------------------------------------------------------

problems, _, _, _ = tree(passing_tree({
    "target/vendored/src/lib.rs": 'pub const T: &str = "de-DE";\n',
    ".hidden/notes/thing.rs": 'pub const T: &str = "de-DE";\n',
    "docs/notes.md": "the forbidden spelling is de-DE\n",
}))
report("target/, dot-directories and markdown are not read", problems == [])

# --- unreadable and empty trees -----------------------------------------------

problems, _, _, _ = tree(passing_tree({
    "crates/x/src/bad.rs": b"\xff\xfe\x00pub fn f() {}",
}))
report("a Rust file that is not UTF-8 is reported", mentions(problems, "bad.rs", "not valid UTF-8"))

problems, _, _, _ = tree(passing_tree({
    "crates/x/catalogues/xx.messages": b"\xff\xfekey = value",
}))
report("a catalogue that is not UTF-8 is reported", mentions(problems, "xx.messages", "not valid UTF-8"))

problems, _, _, _ = tree({"docs/notes.md": "nothing the check reads\n"})
report("a tree with nothing to read is not a pass", mentions(problems, "nothing is not a pass"))

problems, _, _, _ = check.check_tree(Path(tempfile.gettempdir()) / "no-such-tree-anywhere")
report("a missing root is reported rather than raised", problems != [])

# --- the script's exit codes --------------------------------------------------

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    for relative, content in passing_tree().items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    clean = subprocess.run(
        [sys.executable, str(HERE / "check_language_place.py"), "--root", str(root)],
        capture_output=True, text=True,
    )
    report("a clean tree exits 0", clean.returncode == 0)
    report("...saying what it read", "passed" in clean.stdout)

    (root / "crates/x/catalogues/de-DE.ftl").write_text(CLEAN_CATALOGUE, encoding="utf-8")
    breached = subprocess.run(
        [sys.executable, str(HERE / "check_language_place.py"), "--root", str(root)],
        capture_output=True, text=True,
    )
    report("a breached tree exits 1", breached.returncode == 1)
    report("...naming the breach on stderr", "de-DE" in breached.stderr)

print(f"{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
