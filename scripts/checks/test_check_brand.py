#!/usr/bin/env python3
"""Tests for the brand seam check. The check is CI's authority to fail a build
over a pasted brand value, so its own behaviour is proved rather than assumed
-- in particular that it reads its forbidden set from the brand files rather
than a second list, that it cannot pass vacuously, and that the two permitted
homes are permitted and nothing beside them is.

Run: python3 scripts/checks/test_check_brand.py
"""
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_brand

PASSED = FAILED = 0


def check(name, condition):
    global PASSED, FAILED
    if condition:
        PASSED += 1
    else:
        FAILED += 1
        print(f"FAIL: {name}", file=sys.stderr)


# --- fixtures -----------------------------------------------------------------

# A brand file in the restricted schema, with one set value per kind the rule
# names -- a name, a colour, an endpoint, a support address -- plus the two
# values the schema declares meaningless.
BRAND = (
    "# a fixture for the tests\n"
    "\n"
    'name = "Wovenlark"\n'
    'colour = "#12ab34"\n'
    'endpoint = "https://wovenlark.invalid/"\n'
    'support = "helpdesk@wovenlark.invalid"\n'
    'pending = "unset"\n'
    'nothing = ""\n'
)

CLEAN_RS = "#![forbid(unsafe_code)]\npub fn f() {}\n"


def scenario(files):
    """Build a tree in a temporary directory and run the check over it.

    Returns (problems, values, scanned) as check_tree does, or the CheckError
    message as a string where the check refuses a verdict.
    """
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        for name, content in files.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            if isinstance(content, bytes):
                path.write_bytes(content)
            else:
                path.write_text(content, encoding="utf-8")
        try:
            return check_brand.check_tree(root)
        except check_brand.CheckError as error:
            return str(error)


def mentions(problems, *fragments):
    return any(all(fragment in problem for fragment in fragments) for problem in problems)


# --- the repository itself ---------------------------------------------------

problems, values, scanned = check_brand.check_tree(check_brand.REPO)
check("the repository passes", problems == [])
check("the repository's brand files forbid something", values > 0)
check("the repository has Rust source to scan", scanned > 0)

# --- a clean tree ------------------------------------------------------------

result = scenario({"brands/one.toml": BRAND, "crates/a/src/lib.rs": CLEAN_RS})
check("a clean tree passes", result[0] == [])
check("...learning every set value", result[1] == 4)
check("...and scanning the source", result[2] == 1)

# --- breaches ----------------------------------------------------------------

result = scenario(
    {
        "brands/one.toml": BRAND,
        "crates/a/src/lib.rs": 'pub const NAME: &str = "Wovenlark";\n',
    }
)
check("a brand value in a string literal fails", result[0] != [])
check(
    "...naming the file, the line, the value and its origin",
    mentions(result[0], "crates/a/src/lib.rs:1", "Wovenlark", "brands/one.toml: name"),
)

result = scenario(
    {
        "brands/one.toml": BRAND,
        "crates/a/src/lib.rs": "// send help to helpdesk@wovenlark.invalid\n" + CLEAN_RS,
    }
)
check("a brand value in a comment fails", result[0] != [])

result = scenario(
    {
        "brands/one.toml": BRAND,
        "crates/a/src/lib.rs": 'const C: &str = "#12AB34";\n',
    }
)
check("a case-shifted brand value fails", result[0] != [])

result = scenario(
    {
        "brands/one.toml": BRAND,
        "crates/a/build.rs": 'const E: &str = "https://wovenlark.invalid/";\n',
    }
)
check("a build script is workspace source too", result[0] != [])

# --- the two permitted homes -------------------------------------------------

result = scenario(
    {
        "brands/one.toml": BRAND,
        "crates/evreos-shell/src/brand.rs": 'const N: &str = "Wovenlark";\n',
    }
)
check("the seam module may carry brand values", result[0] == [])

result = scenario(
    {
        "brands/one.toml": BRAND,
        "brands/notes.rs": 'const N: &str = "Wovenlark";\n',
    }
)
check("nothing under brands/ is scanned", result[0] == [])

result = scenario(
    {
        "brands/one.toml": BRAND,
        "crates/evreos-shell/src/schema.rs": 'const N: &str = "Wovenlark";\n',
    }
)
check("a sibling of the seam module is NOT permitted", result[0] != [])

# --- what is never forbidden -------------------------------------------------

result = scenario(
    {
        "brands/one.toml": BRAND,
        "crates/a/src/lib.rs": '// the `unset` sentinel\nconst S: &str = "unset";\n',
    }
)
check("the unset sentinel is not a brand value", result[0] == [])

result = scenario(
    {
        "brands/one.toml": BRAND,
        "crates/a/src/lib.rs": 'const S: &str = "";\n',
    }
)
check("the empty string forbids nothing", result[0] == [])

# --- no verdict --------------------------------------------------------------

result = scenario({"crates/a/src/lib.rs": CLEAN_RS})
check("no brands/ directory is no verdict", isinstance(result, str) and "brands/" in result)

result = scenario({"brands/readme.txt": "not a brand file\n"})
check("no brand file is no verdict", isinstance(result, str) and ".toml" in result)

result = scenario({"brands/one.toml": 'pending = "unset"\n'})
check(
    "every value unset is no verdict, not a pass",
    isinstance(result, str) and "unset" in result,
)

result = scenario({"brands/one.toml": "[table]\nname = { nested = true }\n"})
check(
    "a brand file outside the restricted schema is no verdict",
    isinstance(result, str) and "one.toml:1" in result,
)

result = scenario({"brands/one.toml": 'name = "a" # trailing comment\n'})
check("a trailing comment is outside the schema", isinstance(result, str))

result = scenario({"brands/one.toml": b"\xff\xfe not text"})
check("an unreadable brand file is no verdict", isinstance(result, str) and "UTF-8" in result)

# --- an unreadable source file is a breach, not a crash ----------------------

result = scenario(
    {
        "brands/one.toml": BRAND,
        "crates/a/src/lib.rs": b"\xff\xfe not text",
    }
)
check(
    "an unreadable source file is reported, not skipped",
    mentions(result[0], "crates/a/src/lib.rs", "UTF-8"),
)

# --- case-folded walking -----------------------------------------------------

result = scenario(
    {
        "brands/one.toml": BRAND,
        "crates/a/src/LEAK.RS": 'const N: &str = "Wovenlark";\n',
    }
)
check("an upper-case .RS file is scanned", result[0] != [])

result = scenario(
    {
        "BRANDS/one.toml": BRAND,
        "crates/a/src/lib.rs": 'const N: &str = "Wovenlark";\n',
    }
)
check("an upper-case BRANDS/ still teaches the forbidden set", result[0] != [])

result = scenario(
    {
        "brands/one.toml": BRAND,
        "CRATES/EVREOS-SHELL/SRC/BRAND.RS": 'const N: &str = "Wovenlark";\n',
    }
)
check("the seam module is permitted with case folded", result[0] == [])

result = scenario(
    {
        "brands/one.toml": BRAND,
        "TARGET/generated.rs": 'const N: &str = "Wovenlark";\n',
        "crates/a/target/copy.rs": 'const N: &str = "Wovenlark";\n',
    }
)
check("build output is never scanned, whatever its case", result[0] == [])

result = scenario(
    {
        "brands/one.toml": BRAND,
        ".hidden/copy.rs": 'const N: &str = "Wovenlark";\n',
    }
)
check("hidden directories are never scanned", result[0] == [])

# --- more than one brand file ------------------------------------------------

result = scenario(
    {
        "brands/one.toml": BRAND,
        "brands/two.toml": 'name = "Thistledown"\n',
        "crates/a/src/lib.rs": 'const N: &str = "Thistledown";\n',
    }
)
check("every brand file's values are forbidden", result[0] != [])

result = scenario(
    {
        "brands/one.toml": BRAND,
        "brands/two.toml": 'name = "Wovenlark"\n',
        "crates/a/src/lib.rs": 'const N: &str = "Wovenlark";\n',
    }
)
check(
    "a value shared by two brand files names both origins",
    mentions(result[0], "brands/one.toml: name", "brands/two.toml: name"),
)

# --- a value learned today is forbidden today --------------------------------

result = scenario(
    {
        "brands/one.toml": BRAND + 'added_later = "brand-new-value"\n',
        "crates/a/src/lib.rs": 'const N: &str = "brand-new-value";\n',
    }
)
check("a new field's value is forbidden with no edit to the check", result[0] != [])

print(f"{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
