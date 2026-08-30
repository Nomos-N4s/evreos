# Repository rules (non-negotiable)

## Authorship

- Authorship, commit signing, and the prohibition on attributing authorship or
  assistance to an AI or generator tool are governed by Principle I of
  `.specify/memory/constitution.md`. That document supersedes this one, so the
  rules are stated there and NOT restated here.
- Operational detail this file does add. None of it grants a permission the
  constitution withholds; where it is stricter, it narrows what the constitution
  leaves open, which supersession allows:
  - No session, run or conversation identifier in any commit message, pull
    request field, issue or comment. Principle I reaches attribution to an AI or
    generator tool; this file additionally forbids identifiers that trace a
    change to a generating session, which name no tool and so fall outside it.
  - NO `Co-Authored-By` trailer of any kind, including one naming a human.
    Principle I forbids trailers that attribute to an AI or generator tool; this
    file goes further and forbids co-authorship trailers outright, because sole
    authorship is the point. Nothing mechanical enforces this yet — the checker
    matches AI identities only — so it is a review obligation.
  - Before committing, verify `git config user.name` is `xcoder-es` and
    `git config user.email` is `capintobe@gmail.com`.
  - Merge commits created by the forge record the forge as committer AND the
    founder's forge display name as author. The forge is infrastructure rather
    than a third party, so it is the only committer permitted besides the founder
    — the set `ALLOWED_COMMITTERS` in `scripts/check-commit-hygiene.py`.
    Principle I constrains the author and is silent on the committer, so naming
    the permitted set here is one of those narrowings.
  - The author half is not handled by that set, and it is not currently clean.
    Nine merge commits on `main` are authored `Carlos Pinto`, the GitHub account's
    display name, which is not `xcoder-es <capintobe@gmail.com>` and so does not
    satisfy Principle I. They pass CI only because the check runs over
    `origin/<base>..HEAD`, which never contains the merge commit being created.
    The fix is to set the GitHub account's display name to `xcoder-es`; until
    then, history does not meet the rule the check appears to enforce.
  - `scripts/check-commit-hygiene.py` checks, on every pull request: author and
    committer identity, AI identities inside git trailer values, a fixed list of
    literal generator-footer strings, Conventional Commits subjects, and issue
    references — on each commit, and attribution on the pull request title and
    body. Three limits worth knowing before you write a commit message:
    - The footer strings are matched against the whole text, not just a trailer
      block, so they fire on ordinary prose. A layout sentence about a text
      caret is rejected, because one everyday browser word sits in the pattern.
      So is a sentence naming a tool this project actually integrates with, in
      plainly descriptive terms — which Principle I's carve-out permits. The
      script has no carve-out for it; such a mention must be phrased around the
      footer list. Tracked as #25.
    - It makes no attempt at general free-English attribution detection, so
      phrasings outside that list pass. The trailer check is narrower still: it
      reads only the message's trailing paragraph, and only keys ending `-by` or
      `-with`, so an AI trailer earlier in the message, or under a key like
      `Assisted:`, is not inspected. Principle I's prose prohibition rests on
      review, not on the script.
    - Subjects beginning `Merge `, `Revert "`, `fixup! ` or `squash! ` are exempt
      from the subject and issue-reference checks — and the exemption keys on the
      subject string, not on who wrote it, so a hand-written `Merge …` subject
      bypasses both. The merge commits on `main` consequently do not satisfy
      Principle I's Conventional-Commits and issue-reference requirement, and
      are not caught. Nothing checks signatures, and nothing else does either:
      `main` carries no branch protection, so Principle I's signing requirement
      is currently unenforced. Both tracked as #26.

## Branches

- Branch names are purpose-prefixed: `feat/`, `bug/`, `chore/`, `experiment/`,
  `docs/`, `test/`, `refactor/` — pick the prefix that matches the work.
- NEVER use `claude/` (or any agent/session-derived name) as a branch name or prefix.

## Workflow: main is protected

- NEVER push directly to `main`. Every change reaches `main` only through a
  pull request.
- Every pull request MUST be linked to a GitHub issue (`Closes #N` in the PR
  body). Open the issue first if one does not exist.
- Commit messages follow conventional commits: `type(scope): lowercase
  imperative subject` (e.g. `chore(speckit): set up Spec Kit`), matching the
  style used across this account's other repositories (see Nomos-N4s/nomos).
  Every commit message references the issue it serves (`Closes #N` or `Refs #N`).
- Commits are atomic: exactly one logical change per commit. Never bundle
  unrelated changes; split mechanical moves and refactors from behavior
  changes so each commit stands, builds, and reverts on its own.
- Adversarial review after every push, and the merge gate that depends on it,
  are governed by the Development Workflow section of
  `.specify/memory/constitution.md`. That document supersedes this one, so the
  rules are stated there and NOT restated here — a stricter copy in this file
  would be void wherever the two disagreed, which is how this note came to
  exist.
- The first four bullets of this section restate rules the constitution also
  states. That is deliberate and it is the narrower case: they are short
  reminders of settled rules that no reviewer has to weigh wording against, kept
  here for reach — two of them are gated mechanically by the hygiene check, so
  the wording is settled by the script rather than by argument. Where a rule's
  exact wording is what a human gate turns on — the review round, the merge
  record, the override — it is stated in the constitution only. When the
  constitution is amended, the reminders above MUST be re-checked against it.

## Spec-driven development

- This repo uses GitHub Spec Kit. Features flow through the skills in
  `.claude/skills/`: constitution → specify → plan → tasks → implement
  (with clarify/checklist/analyze as optional quality gates).
- Feature specs live under `specs/`, one directory per feature.
- Spec Kit resolves a feature by directory, never by git branch: keep the
  `NNN-slug` directory name its scripts generate (e.g. `001-evreos-v1`), which
  `.specify/feature.json` records. Branch names are unconstrained by Spec Kit
  and follow the prefix rule above.
