# Repository rules (non-negotiable)

## Authorship

- Every commit MUST be authored and committed solely as `xcoder-es <capintobe@gmail.com>`.
- NO `Co-Authored-By` trailers of any kind. No generator, tool, session, or
  AI attribution anywhere: not in commit messages, PR titles or bodies, issues,
  comments, code comments, or generated files. No exceptions, ever.
- Before committing, verify `git config user.name` is `xcoder-es` and
  `git config user.email` is `capintobe@gmail.com`.

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

## Spec-driven development

- This repo uses GitHub Spec Kit. Features flow through the skills in
  `.claude/skills/`: constitution → specify → plan → tasks → implement
  (with clarify/checklist/analyze as optional quality gates).
- Feature specs live under `specs/`, one directory per feature.
