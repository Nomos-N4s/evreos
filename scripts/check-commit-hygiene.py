#!/usr/bin/env python3
"""Check commit authorship and the absence of AI attribution.

Reads commits from git and, when given --pr-body or --pr-title, that text too.
Exits non-zero and prints every violation found.

The rule is about ATTRIBUTION, not vocabulary. Naming a tool the project
integrates with -- the .claude/ directory, Spec Kit's Claude Code integration --
is description and passes. Claiming a tool authored or assisted the work fails.

Two deliberate exceptions to that, both learned from adversarial review:

* Canonical generator footers and attribution trailers are matched even inside
  Markdown code fences. A fenced block still renders as a visible, legible
  attribution on the forge, and git stores the bytes verbatim, so quoting must
  not hide it.
* Everything else is matched only outside code spans and fences, so the rule
  can be documented without the documentation tripping it.
"""
import argparse
import re
import subprocess
import sys
import unicodedata

REQUIRED_AUTHOR = "xcoder-es <capintobe@gmail.com>"
# A forge creates merge commits with its own identity as committer. That is
# infrastructure, not a third party authoring the work.
ALLOWED_COMMITTERS = {REQUIRED_AUTHOR, "GitHub <noreply@github.com>"}

# Identities that must never appear as an attributed party.
TOOL = r"(?:claude|anthropic|copilot|chatgpt|openai|gemini|cursor|codex|llm|\bai\b)"

# Any git trailer. The KEY is not enumerated -- the VALUE is tested for a tool
# identity -- so an unknown trailer key cannot smuggle attribution through.
TRAILER = re.compile(r"^[ \t]*[A-Za-z][A-Za-z-]*[ \t]*:[ \t]*(?P<value>.+)$", re.MULTILINE)

# Verbs of authorship, allowing a few intervening words before the preposition
# ("written entirely by X") and a qualifier after a tool name ("Claude Code").
VERB = r"(?:generated|authored|written|wrote|created|produced|made|implemented|built|developed|drafted|refactored|coded)"
GAP = r"(?:\s+\w+){0,3}"
ATTRIBUTION = re.compile(
    rf"{VERB}{GAP}\s+(?:with|by|using)\s+(?:the\s+)?(?:\w+\s+){{0,3}}?{TOOL}"
    rf"|(?:with|using)\s+(?:the\s+)?(?:help|assistance|aid)\s+(?:of|from|by)\s+(?:\w+\s+){{0,2}}?{TOOL}"
    rf"|(?:thanks\s+to|courtesy\s+of|in\s+collaboration\s+with)\s+(?:\w+\s+){{0,2}}?{TOOL}"
    rf"|\bai[-\s]?(?:generated|authored|assisted|written|made)\b"
    rf"|{TOOL}(?:\s+\w+){{0,2}}\s+{VERB}\b",
    re.IGNORECASE,
)

# Canonical footers, matched even inside code fences.
CANONICAL = re.compile(
    rf"🤖\s*generated|generated\s+(?:with|by)\s+\[?{TOOL}|noreply@anthropic\.com"
    rf"|co-authored-by:\s*(?:\w+\s+)?{TOOL}",
    re.IGNORECASE,
)

CONVENTIONAL = re.compile(
    r"^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)"
    r"(\([a-z0-9._/-]+\))?!?: [a-z].*"
)

# An issue reference needs a linking keyword; a bare #NNN also matches CSS
# colours and URL fragments.
ISSUE_REF = re.compile(r"\b(closes|close|fixes|fix|resolves|resolve|refs|ref|see)\s+#\d+\b", re.IGNORECASE)

CODE_BLOCK = re.compile(r"```.*?```", re.DOTALL)
CODE_SPAN = re.compile(r"`[^`\n]*`")
# Zero-width and other format characters, used to break anchors and word gaps.
INVISIBLE = re.compile(r"[​-‏‪-‮⁠-⁤﻿]")


def normalise(text):
    """Fold unicode tricks that would otherwise defeat the patterns."""
    return INVISIBLE.sub("", unicodedata.normalize("NFKC", text))


def strip_code(text):
    return CODE_SPAN.sub(" ", CODE_BLOCK.sub(" ", text))


def strip_git_comments(text):
    """Drop the comment block and the -v diff git appends to a message file."""
    out = []
    for line in text.splitlines():
        if line.startswith("# ------------------------ >8 ------------------------"):
            break
        if not line.startswith("#"):
            out.append(line)
    return "\n".join(out)


def attribution_problems(text, where):
    """Return attribution problems in text, as (where, description) strings."""
    full = normalise(text)
    prose = strip_code(full)
    found = []
    if CANONICAL.search(full):
        found.append(f"{where}: carries a generator footer or AI attribution trailer")
    for match in TRAILER.finditer(prose):
        if re.search(TOOL, match.group("value"), re.IGNORECASE):
            found.append(f"{where}: a trailer attributes the work to {match.group('value').strip()!r}")
            break
    if ATTRIBUTION.search(prose):
        found.append(f"{where}: attributes the work to an AI or generator tool")
    return found


def run(*args):
    return subprocess.run(["git", *args], capture_output=True, text=True, check=True).stdout


def check_commit(sha, problems):
    author = run("log", "-1", "--format=%an <%ae>", sha).strip()
    committer = run("log", "-1", "--format=%cn <%ce>", sha).strip()
    message = run("log", "-1", "--format=%B", sha)
    parents = run("log", "-1", "--format=%P", sha).split()
    is_merge = len(parents) > 1
    subject = message.splitlines()[0] if message.splitlines() else ""
    where = f"commit {sha[:8]} ({subject[:50]})"

    if author != REQUIRED_AUTHOR:
        problems.append(f"{where}: author is {author!r}, must be {REQUIRED_AUTHOR!r}")
    if committer not in ALLOWED_COMMITTERS:
        problems.append(f"{where}: committer is {committer!r}, must be {REQUIRED_AUTHOR!r}")
    problems.extend(attribution_problems(message, where))

    # A forge writes merge subjects itself; they are not authored prose.
    if not is_merge:
        if not CONVENTIONAL.match(subject):
            problems.append(
                f"{where}: subject is not a Conventional Commit "
                "(type(scope): lowercase imperative subject)"
            )
        if not ISSUE_REF.search(strip_code(normalise(message))):
            problems.append(
                f"{where}: message does not reference an issue "
                "(e.g. 'Closes #12' or 'Refs #12')"
            )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--range", help="git rev range to check, e.g. base..head")
    parser.add_argument("--pr-body", help="file holding the pull request body")
    parser.add_argument("--pr-title", help="file holding the pull request title")
    parser.add_argument(
        "--commit-msg", help="file holding a commit message, for the local hook"
    )
    args = parser.parse_args()

    problems = []

    if args.range:
        try:
            # Merges are included: a third party merging is a real violation.
            shas = run("log", "--format=%H", args.range).split()
        except subprocess.CalledProcessError as error:
            print(
                f"Cannot resolve rev range {args.range!r}: "
                f"{error.stderr.strip() or 'unknown git error'}",
                file=sys.stderr,
            )
            return 2
        if not shas:
            print(f"No commits in {args.range}; nothing to check.")
        for sha in shas:
            check_commit(sha, problems)

    for flag, label in (
        (args.pr_body, "pull request body"),
        (args.pr_title, "pull request title"),
    ):
        if flag:
            with open(flag, encoding="utf-8") as handle:
                problems.extend(attribution_problems(handle.read(), label))

    if args.commit_msg:
        with open(args.commit_msg, encoding="utf-8") as handle:
            message = strip_git_comments(handle.read())
        subject = message.strip().splitlines()[0] if message.strip() else ""
        problems.extend(attribution_problems(message, "commit message"))
        if not CONVENTIONAL.match(subject):
            problems.append(
                "commit message: subject is not a Conventional Commit "
                "(type(scope): lowercase imperative subject)"
            )
        if not ISSUE_REF.search(strip_code(normalise(message))):
            problems.append(
                "commit message: does not reference an issue "
                "(e.g. 'Closes #12' or 'Refs #12')"
            )

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
