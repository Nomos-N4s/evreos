#!/usr/bin/env python3
"""Check commit authorship and block automated AI-attribution artifacts.

WHAT THIS CHECKS, and why the scope is narrow.

The real, recurring problem is mechanical: tooling appends a fixed footer or a
`Co-Authored-By:` trailer to a commit message or a pull request body. Those are
literal strings in known shapes, so they can be detected exactly, with no false
positives.

An earlier version of this script also tried to detect attribution written as
free English ("assisted by X", "X's implementation"). Three rounds of adversarial
review showed that is not tractable for a browser project: `cursor` is everyday
vocabulary ("the text cursor is created before layout"), an AI sidebar is a
product feature, and a bare https://platform.openai.com/... URL parses as a
trailer. Each pattern that caught more phrasings also rejected more legitimate
commits. Blocking a real commit is worse than missing prose no tool emits, so
prose detection was removed.

Free-prose attribution remains forbidden by CLAUDE.md and by Principle I of the
constitution. It is enforced by review, not by this script, and this script does
not pretend otherwise.

CHECKED (deterministic, no false positives by construction):
  * commit author is the founder; committer is the founder or the forge
  * a git trailer in the trailing trailer block whose value names an AI identity
  * a literal generator footer, matched even inside code fences
  * Conventional Commits subject, and an issue reference
"""
import argparse
import re
import subprocess
import sys
import unicodedata

REQUIRED_AUTHOR = "xcoder-es <capintobe@gmail.com>"
# A forge authors the merge commits it creates. That is infrastructure.
ALLOWED_COMMITTERS = {REQUIRED_AUTHOR, "GitHub <noreply@github.com>"}

# Identities, tested ONLY against a git trailer's value, where "Claude
# <noreply@anthropic.com>" is unambiguous. Never tested against prose.
TRAILER_IDENTITY = re.compile(
    r"anthropic\.com|\bclaude\b|\bcopilot\b|\bchatgpt\b|\bgithub-actions\[bot\]\b"
    r"|\bopenai\b|\bgemini\b|\bdevin\b",
    re.IGNORECASE,
)

# A git trailer line: a token key, a colon, a value. Keys never contain spaces,
# which is what separates "Co-Authored-By: x" from "Note: some sentence".
TRAILER_LINE = re.compile(r"^(?P<key>[A-Za-z][A-Za-z-]*)-(?:by|with):[ \t]*(?P<value>.+?)[ \t]*$")

# Literal footers that tooling emits. Matched anywhere, including inside code
# fences, because a fence still renders as a visible attribution.
CANONICAL_FOOTER = re.compile(
    r"🤖\s*generated"
    r"|generated\s+(?:with|by)\s+\[?(?:claude|copilot|chatgpt|codex|cursor|gemini|devin)\b"
    r"|noreply@anthropic\.com"
    r"|co-authored-by:\s*(?:claude|copilot|chatgpt|codex|gemini|devin)\b",
    re.IGNORECASE,
)

CONVENTIONAL = re.compile(
    r"^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)"
    r"(\([a-z0-9._/-]+\))?!?: [a-z].*"
)
# A linking keyword is required; a bare #NNN also matches CSS colours and URL
# fragments.
ISSUE_REF = re.compile(
    r"\b(closes?|fixes|fixed|resolves?|refs?|see)\s+#\d+\b", re.IGNORECASE
)
# Messages a forge or git writes itself, exempt from subject and issue rules.
GENERATED_SUBJECT = re.compile(r"^(Merge |Revert \"|fixup! |squash! )")

CODE_SPAN_OR_BLOCK = re.compile(r"```.*?```|`[^`\n]*`", re.DOTALL)


def normalise(text):
    """Fold unicode tricks: compatibility forms, then all invisible formatting."""
    folded = unicodedata.normalize("NFKC", text)
    return "".join(
        ch for ch in folded
        if unicodedata.category(ch) != "Cf" and ch not in "­͏"
    )


def trailer_block(text):
    """The trailing paragraph, which is where git puts trailers."""
    paragraphs = [p for p in re.split(r"\n\s*\n", text.strip()) if p.strip()]
    return paragraphs[-1] if paragraphs else ""


def attribution_problems(text, where):
    """Return attribution problems as description strings."""
    full = normalise(text)
    found = []
    if CANONICAL_FOOTER.search(full):
        found.append(f"{where}: carries a generator footer or AI attribution trailer")
    for line in trailer_block(full).splitlines():
        match = TRAILER_LINE.match(line.strip())
        if match and TRAILER_IDENTITY.search(match.group("value")):
            found.append(
                f"{where}: trailer {match.group('key')!r} attributes the work to "
                f"{match.group('value')!r}"
            )
            break
    return found


def run(*args):
    return subprocess.run(["git", *args], capture_output=True, text=True, check=True).stdout


def check_message(message, where, problems):
    problems.extend(attribution_problems(message, where))
    lines = message.strip().splitlines()
    subject = lines[0] if lines else ""
    if GENERATED_SUBJECT.match(subject):
        return
    if not CONVENTIONAL.match(subject):
        problems.append(
            f"{where}: subject is not a Conventional Commit "
            "(type(scope): lowercase imperative subject)"
        )
    if not ISSUE_REF.search(CODE_SPAN_OR_BLOCK.sub(" ", normalise(message))):
        problems.append(
            f"{where}: does not reference an issue (e.g. 'Closes #12' or 'Refs #12')"
        )


def check_commit(sha, problems):
    author = run("log", "-1", "--format=%an <%ae>", sha).strip()
    committer = run("log", "-1", "--format=%cn <%ce>", sha).strip()
    message = run("log", "-1", "--format=%B", sha)
    subject = message.splitlines()[0] if message.splitlines() else ""
    where = f"commit {sha[:8]} ({subject[:50]})"

    if author != REQUIRED_AUTHOR:
        problems.append(f"{where}: author is {author!r}, must be {REQUIRED_AUTHOR!r}")
    if committer not in ALLOWED_COMMITTERS:
        problems.append(f"{where}: committer is {committer!r}, must be {REQUIRED_AUTHOR!r}")
    check_message(message, where, problems)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--range", help="git rev range to check, e.g. base..head")
    parser.add_argument("--pr-body", help="file holding the pull request body")
    parser.add_argument("--pr-title", help="file holding the pull request title")
    parser.add_argument("--commit-msg", help="file holding a commit message, for the hook")
    args = parser.parse_args()

    problems = []

    if args.range:
        try:
            shas = run("log", "--format=%H", args.range).split()
        except subprocess.CalledProcessError as error:
            print(
                f"Cannot resolve rev range {args.range!r}: "
                f"{(error.stderr or '').strip().splitlines()[0] if error.stderr else 'git error'}",
                file=sys.stderr,
            )
            return 2
        if not shas:
            print(f"No commits in {args.range}; nothing to check.")
        for sha in shas:
            check_commit(sha, problems)

    for path, label in ((args.pr_body, "pull request body"), (args.pr_title, "pull request title")):
        if path:
            with open(path, encoding="utf-8") as handle:
                problems.extend(attribution_problems(handle.read(), label))

    if args.commit_msg:
        with open(args.commit_msg, encoding="utf-8") as handle:
            raw = handle.read()
        body = "\n".join(
            line for line in raw.split("# ------------------------ >8")[0].splitlines()
            if not line.startswith("#")
        )
        check_message(body, "commit message", problems)

    if problems:
        print("Commit hygiene check FAILED:\n", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        print(
            "\nSee the Authorship rules in CLAUDE.md and Principle I of "
            ".specify/memory/constitution.md.",
            file=sys.stderr,
        )
        return 1

    print("Commit hygiene check passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
