# Repository rules (non-negotiable)

## Authorship

- Every commit MUST be authored by `xcoder-es <capintobe@gmail.com>` and MUST be signed.
  Merge commits created by the forge record the forge as committer; that is
  infrastructure, not a third party, and is the only permitted committer besides
  the founder.
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
  Every commit message references the issue it serves (`Closes #N` or `Refs #N`).
- Commits are atomic: exactly one logical change per commit. Never bundle
  unrelated changes; split mechanical moves and refactors from behavior
  changes so each commit stands, builds, and reverts on its own.
- After EVERY push of commits to an open pull request, run an adversarial
  review of the PR's full current diff: independent reviewers instructed to
  refute the changes (correctness, security, consistency, rules compliance),
  with findings verified before they count. Confirmed findings are fixed and
  pushed — which triggers a new review round — before the PR is ready to
  merge. Record the outcome of each round on the PR or to the founder.

## Spec-driven development

- This repo uses GitHub Spec Kit. Features flow through the skills in
  `.claude/skills/`: constitution → specify → plan → tasks → implement
  (with clarify/checklist/analyze as optional quality gates).
- Feature specs live under `specs/`, one directory per feature.
- Spec Kit resolves a feature by directory, never by git branch: keep the
  `NNN-slug` directory name its scripts generate (e.g. `001-evreos-v1`), which
  `.specify/feature.json` records. Branch names are unconstrained by Spec Kit
  and follow the prefix rule above.
