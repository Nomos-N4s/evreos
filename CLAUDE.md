# Repository rules (non-negotiable)

## Authorship

- Authorship, commit signing, and the prohibition on attributing authorship or
  assistance to an AI or generator tool are governed by Principle I of
  `.specify/memory/constitution.md`. That document supersedes this one, so the
  rules are stated there and NOT restated here.
- Operational detail this file does add, which grants no permission and imposes
  no rule the constitution does not:
  - Before committing, verify `git config user.name` is `xcoder-es` and
    `git config user.email` is `capintobe@gmail.com`.
  - Merge commits created by the forge record the forge as committer. The
    constitution constrains the author, not the committer, so this is permitted;
    the forge is infrastructure rather than a third party.
  - `scripts/check-commit-hygiene.py` enforces the mechanical part of Principle I
    on every pull request. Its carve-out for naming an integrated tool is the
    one Principle I grants, not an exception to it.

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

## Spec-driven development

- This repo uses GitHub Spec Kit. Features flow through the skills in
  `.claude/skills/`: constitution → specify → plan → tasks → implement
  (with clarify/checklist/analyze as optional quality gates).
- Feature specs live under `specs/`, one directory per feature.
- Spec Kit resolves a feature by directory, never by git branch: keep the
  `NNN-slug` directory name its scripts generate (e.g. `001-evreos-v1`), which
  `.specify/feature.json` records. Branch names are unconstrained by Spec Kit
  and follow the prefix rule above.
