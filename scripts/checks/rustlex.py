#!/usr/bin/env python3
"""Blank the non-code regions of Rust source, keeping every offset.

One scanner, in one place, because two of them was the defect. The crate policy
check needs it to decide whether a `#![forbid(unsafe_code)]` is real; the engine
prohibition check needs it to decide whether a `#![feature(...)]` is. A second,
weaker copy grew up beside the first with no char-literal rule and no escape
handling, and it both rejected compliant crate roots and hid a genuine nightly
gate -- the same defect class the first copy had already been fixed for.

It is a scanner rather than a set of patterns because the regions nest and
interleave, and the order below is the order Rust's lexer uses.
"""

import re

# A char literal: one character, or one escape. `'a'` and `'\''` are literals;
# `'a` in `&'a str` is a lifetime and opens nothing. The distinction matters
# because a literal may CONTAIN a quote -- `'"'` -- and reading that quote as a
# string opener desynchronises everything after it.
CHAR_LITERAL = re.compile(r"'(?:\\.|[^'\\])'")
# A raw string opener, with its hash count captured so the matching close can
# be found: r"..", r#".."#, b"..", br#".."#, rb#".."#.
RAW_OPENER = re.compile(r'(?:br|rb|r)(#*)"')
PLAIN_OPENER = re.compile(r'b?"')


def strip_non_code(source):
    """`source` with every non-code region blanked, newlines kept.

    An inner attribute inside a comment or a string is not an attribute -- it
    is text the compiler never reads -- and one inside a function body is
    block-scoped rather than crate-scoped, so none of them forbids anything.
    Matching them let a crate pass by commenting its own forbid out, which is
    the realistic way in: comment it, try `unsafe`, forget to restore it.

    This is a scanner rather than a set of patterns because the regions nest
    and interleave, and the order below is the order Rust's lexer uses:

    - Line comments FIRST. A `//` comment ends at the newline, so a `/*` or an
      `r"` written inside prose opens nothing. Reading those as real openers
      blanked everything to the end of the file, and a crate root that plainly
      carried its forbid was reported as omitting it.
    - Char literals BEFORE strings. A literal may contain a quote -- `'"'` --
      and reading that quote as a string opener opened a phantom string that
      ran to the next quote in the file, blanking real code and leaving a
      genuine string to be scanned as code. A crate with no forbid at all then
      passed. A lifetime (`&'a str`) is not a literal and is left alone.

    Indices are passed to the matchers rather than slicing, so the scan is
    linear: slicing the remainder at every character made it quadratic, which
    is invisible on a hand-written crate root and not on a generated one.
    """
    out, i, n = [], 0, len(source)

    def blank(text):
        return "".join(character if character == "\n" else " " for character in text)

    while i < n:
        if source.startswith("//", i):
            end = source.find("\n", i)
            end = n if end == -1 else end
            out.append(blank(source[i:end]))
            i = end
        elif source.startswith("/*", i):
            depth, j = 1, i + 2          # Rust block comments nest
            while j < n and depth:
                if source.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif source.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            out.append(blank(source[i:j]))
            i = j
        elif (literal := CHAR_LITERAL.match(source, i)) is not None:
            out.append(blank(literal.group(0)))
            i = literal.end()
        elif (raw := RAW_OPENER.match(source, i)) is not None:
            close = '"' + raw.group(1)
            end = source.find(close, raw.end())
            end = n if end == -1 else end + len(close)
            out.append(blank(source[i:end]))
            i = end
        elif (plain := PLAIN_OPENER.match(source, i)) is not None:
            j = plain.end()
            while j < n and source[j] != '"':
                j += 2 if source[j] == "\\" else 1
            j = min(j + 1, n)
            out.append(blank(source[i:j]))
            i = j
        else:
            out.append(source[i])
            i += 1
    return "".join(out)
