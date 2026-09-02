#!/usr/bin/env python3
"""Tests for rustlex, the one Rust scanner both checks read code through.

It is not a check and has no verdict of its own, so nothing here asserts a pass
or a failure of a rule. What it asserts is the property both checks rest on:
that a region of a Rust file which is not code comes back blanked, that a region
which IS code comes back untouched, and that every offset is preserved so a
match's position still names the right line.

Two checks were wrong at once because a weaker second copy of this scanner
existed, so the rules below are stated as inputs that distinguish this scanner
from the one that had no char-literal rule and no escape handling.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rustlex

PASSED = 0
FAILED = []


def check(name, condition):
    global PASSED
    if condition:
        PASSED += 1
    else:
        FAILED.append(name)
        print(f"FAIL: {name}", file=sys.stderr)


def blanked(source, fragment):
    """Whether `fragment` is gone from the scanned source."""
    return fragment not in rustlex.strip_non_code(source)


def kept(source, fragment):
    return fragment in rustlex.strip_non_code(source)


# --- offsets ------------------------------------------------------------------
# Both checks report a line number by counting newlines up to a match, and one
# of them matches over a window rather than line by line. A scanner that
# changed a length would move every report after it.
for label, source in (
    ("a line comment", "let a = 1; // note\nlet b = 2;\n"),
    ("a block comment", "let a = 1; /* note\nmore */ let b = 2;\n"),
    ("a string", 'let s = "text";\nlet b = 2;\n'),
    ("a raw string", 'let s = r#"te"xt"#;\nlet b = 2;\n'),
    ("a char literal", "let c = 'x';\nlet b = 2;\n"),
):
    scanned = rustlex.strip_non_code(source)
    check(f"{label} keeps the source length", len(scanned) == len(source))
    check(f"{label} keeps every newline position",
          [i for i, c in enumerate(scanned) if c == "\n"]
          == [i for i, c in enumerate(source) if c == "\n"])

# --- line comments come first -------------------------------------------------
# A `//` comment ends at the newline. Reading a `/*` or an `r"` written inside
# one as a real opener blanked the rest of the file, and a crate root plainly
# carrying its forbid was reported as omitting it.
check("a block-comment opener inside a line comment opens nothing",
      kept("// see /* here\n#![forbid(unsafe_code)]\n", "#![forbid(unsafe_code)]"))
check("a raw-string opener inside a line comment opens nothing",
      kept('// see r" here\n#![forbid(unsafe_code)]\n', "#![forbid(unsafe_code)]"))
check("a quote inside a line comment opens nothing",
      kept('// it\'s "quoted"\n#![forbid(unsafe_code)]\n', "#![forbid(unsafe_code)]"))
check("the comment itself is blanked",
      blanked("// forbid(unsafe_code)\nfn f() {}\n", "forbid"))

# --- block comments nest ------------------------------------------------------
# The text between the inner close and the outer close is what separates the two
# readings: a scanner that does not count depth ends the comment at the FIRST
# `*/` and reads that text as code. Asserting only that the attribute after the
# comment survives distinguishes nothing -- it survives either way.
check("a nested block comment closes at the outer end",
      blanked("/* a /* b */ marker */ #![forbid(unsafe_code)]\n", "marker"))
check("...and the code after the outer close is code",
      kept("/* a /* b */ marker */ #![forbid(unsafe_code)]\n", "#![forbid(unsafe_code)]"))
check("...and the inner contents are blanked",
      blanked("/* a /* forbid */ c */ fn f() {}\n", "forbid"))
check("an unterminated block comment runs to the end of the file",
      blanked("/* a\n#![forbid(unsafe_code)]\n", "#![forbid(unsafe_code)]"))

# --- char literals, before strings --------------------------------------------
# A literal may CONTAIN a quote. Reading that quote as a string opener opened a
# phantom string that ran to the next quote in the file, blanking real code and
# leaving a genuine string to be scanned as code. A crate with no forbid at all
# then passed, holding `unsafe`.
check("a quote inside a char literal does not open a string",
      kept("let q = '\"'; #![forbid(unsafe_code)]\n", "#![forbid(unsafe_code)]"))
# The escape must be an escaped DOUBLE quote for the two readings to differ. A
# scanner with no escape branch fails to match the literal, emits its opening
# quote as code, and then reads the `"` inside it as a string opener -- which
# runs to the next quote in the file, blanking real code and leaving a genuine
# string to be scanned as code. `'\''` distinguishes nothing: a single quote
# opens nothing either way.
check("an escaped double quote in a char literal does not open a string",
      kept('let q = \'\\"\'; let s = "text"; #![forbid(unsafe_code)]\n',
           "#![forbid(unsafe_code)]"))
check("...and the genuine string beside it is still blanked",
      blanked('let q = \'\\"\'; let s = "text";\n', "text"))
check("an escaped single quote in a char literal is one literal",
      blanked("let q = '\\''; let s = \"text\";\n", "text"))
check("an escaped backslash in a char literal is one literal",
      kept("let q = '\\\\'; #![forbid(unsafe_code)]\n", "#![forbid(unsafe_code)]"))
# A lifetime is not a literal and opens nothing, so the code after it is code.
for label, source in (
    ("a lifetime parameter", "fn f<'a>(s: &'a str) -> &'a str { s }\n#![forbid(unsafe_code)]\n"),
    ("two lifetimes", "struct S<'a, 'b>(&'a str, &'b str);\n#![forbid(unsafe_code)]\n"),
    ("a static lifetime", "const S: &'static str = \"x\";\n#![forbid(unsafe_code)]\n"),
):
    check(f"{label} leaves the following code alone",
          kept(source, "#![forbid(unsafe_code)]"))

# --- raw strings and their hash count -----------------------------------------
# The close must match the opener's hash count exactly. Closing early leaves the
# remainder of the string to be scanned as code; closing late blanks real code.
check("a raw string with no hashes closes at the first quote",
      kept('let s = r"a"; #![forbid(unsafe_code)]\n', "#![forbid(unsafe_code)]"))
check("a raw string ignores a quote inside it",
      blanked('let s = r#"a " b"#; fn g() {}\n', "a \" b"))
check("...and the code after it survives",
      kept('let s = r#"a " b"#; #![forbid(unsafe_code)]\n', "#![forbid(unsafe_code)]"))
check("a doubled hash count does not close on a single one",
      kept('let s = r##"a "# b"##; #![forbid(unsafe_code)]\n', "#![forbid(unsafe_code)]"))
check("an unterminated raw string runs to the end of the file",
      blanked('let s = r#"a\n#![forbid(unsafe_code)]\n', "#![forbid(unsafe_code)]"))
# Every prefix the opener admits. A byte raw string is a raw string, and both
# orderings of the prefix are read the same way.
for label, opener in (("br", 'br#"'), ("rb", 'rb#"'), ("r", 'r#"')):
    check(f"{label} opens a raw string",
          kept(f'let s = {opener}a " b"#; #![forbid(unsafe_code)]\n',
               "#![forbid(unsafe_code)]"))

# --- plain strings and their escapes ------------------------------------------
check("a plain string is blanked", blanked('let s = "forbid";\n', "forbid"))
check("an escaped quote does not close a plain string",
      kept('let s = "a \\" b"; #![forbid(unsafe_code)]\n', "#![forbid(unsafe_code)]"))
# What distinguishes the two readings is the prefix character, not the quoted
# text: without `b?` the opener matches at the quote instead, so the literal is
# still blanked but its `b` is emitted as code. The literal is one token and is
# blanked as one.
check("a byte string is blanked with its prefix",
      rustlex.strip_non_code('let s = b"a";') == "let s =     ;")
check("...and its contents are blanked", blanked('let s = b"forbid";\n', "forbid"))
check("an unterminated plain string runs to the end of the file",
      blanked('let s = "a\n#![forbid(unsafe_code)]\n', "#![forbid(unsafe_code)]"))

# --- code is left exactly as it was -------------------------------------------
for source in (
    "#![forbid(unsafe_code)]\npub fn f(x: u32) -> u32 { x + 1 }\n",
    "#![feature(let_chains)]\n",
    "let v = a[i];\nfn feature(n: u32) -> u32 { n }\n",
):
    check(f"code with no non-code region is unchanged: {source.splitlines()[0]!r}",
          rustlex.strip_non_code(source) == source)

check("an empty source is unchanged", rustlex.strip_non_code("") == "")

total = PASSED + len(FAILED)
print(f"\n{PASSED}/{total} passed")
sys.exit(1 if FAILED else 0)
