# Branch protection for `main`

- **Status**: Required. NOT APPLIED on the forge as of 2026-09-02; see the
  status section at the end.
- **Applies to**: the `main` branch of `Nomos-N4s/evreos` on GitHub
- **Serves**: Principle I of `.specify/memory/constitution.md`; issue #26

## Why this file exists

Principle I makes two requirements NON-NEGOTIABLE, and the constitution's
preamble requires that anything measurable be gated in CI rather than left to
intent: every commit is signed, and nothing lands on `main` except through a
pull request. The first is now checked by `scripts/check-commit-hygiene.py`
against the founder's public key in `.github/allowed-signers`. The second
cannot be checked by anything in this tree. Branch protection is configuration
the forge holds; no file here can assert it, and no workflow can refuse a push
that bypasses workflows. What a file can do is record the settings exactly, so
that a reviewer opens the forge, compares, and reports a deviation as a finding
rather than an opinion.

Until the settings below are applied, the rule rests on review. That is stated
here plainly, because a rule that appears enforced and is not fails silently,
which is the failure Principle I's rationale names.

## Required settings

Names are the forge's own, as shown under Settings → Branches → Branch
protection rules → `main`. Every row is required. A live setting that differs
is a deviation, to be corrected or recorded as a founder decision in the pull
request that amends this file.

| Setting | Required value |
| --- | --- |
| Require a pull request before merging | On |
| Required approvals | 0 |
| Require status checks to pass before merging | On |
| Required status checks | `Authorship and attribution`, `Build, test and budgets`, `Repository checks` |
| Require branches to be up to date before merging | On |
| Do not allow bypassing the above settings | On |
| Allow force pushes | Off, for everyone |
| Allow deletions | Off |

And under Settings → General → Pull Requests, which is repository
configuration rather than branch protection but guards the same property:

| Setting | Required value |
| --- | --- |
| Allow merge commits | On |
| Allow squash merging | Off |
| Allow rebase merging | Off |

### The three required checks

The name the forge matches is the job's `name:` field, not the workflow's.
Renaming a job silently detaches it from the requirement, and every pull
request then waits on a check that never reports. A job rename is therefore a
change to this file and to the forge settings in the same step.

- `Authorship and attribution` is the job in
  `.github/workflows/commit-hygiene.yml`. It checks author, committer,
  attribution, message format and — once `.github/allowed-signers` on `main`
  lists a key — the signature of every commit the pull request adds.
- `Build, test and budgets` is the job in `.github/workflows/build.yml`:
  format, lint, tests, the release build and the budget gates of Principle II.
- `Repository checks` is the job in `.github/workflows/checks.yml`, which runs
  every check under `scripts/checks/`. Every later task in
  `specs/001-evreos-v1/tasks.md` that adds a repository check enforces through
  this one job and no other. Until it is required, none of them can block a
  merge: a red check that is not required is advice. One ordering constraint:
  a required check whose workflow does not exist yet leaves every pull request
  waiting indefinitely, so this row is applied the moment `checks.yml` is on
  `main`, and not before.

### Why each row

**A pull request, with zero required approvals.** The constitution's merge
gate is the recorded review round on the pull request, not a forge approval.
Every commit is the founder's, and the forge does not let an author approve
their own pull request, so any count above zero either blocks every merge or
invites a second account whose approval attests nothing.

**Branches up to date.** The hygiene check runs over `origin/<base>..HEAD`,
and a review round covers one (merge base, head) pair; both are computed
against the base as it stood when they ran. If `main` moves afterwards, neither
covers what would merge. The constitution already voids a round whose merge
base has moved; this row makes the forge refuse the merge rather than relying
on someone noticing.

**No bypass.** The founder holds administrator rights. Without this row, every
row above binds nobody who could break it. The override the constitution
permits is a stated merge over confirmed findings, on the pull request, before
the merge — never a direct push.

**No force pushes, no deletions.** The history of `main` is what the merge
gate's recorded SHAs point into. A rewrite silently invalidates every recorded
round and every signature verification that ran against a base that no longer
exists.

**Merge commits only.** A squash or rebase merge creates new commit objects on
the forge. A new object cannot carry the founder's signature, because only the
founder's key can produce one, so the commits the check verified would not be
the commits that land. Only the merge-commit method leaves the verified
commits untouched underneath the forge's own merge commit.

### Not required, and why

- **Require signed commits**, the forge's own setting, verifies against keys
  registered on forge accounts — a trust root this repository does not hold
  and cannot review. It may be turned on without conflict, but Principle I's
  check does not rest on it, so this record neither requires nor forbids it.
- **Require linear history** forbids merge commits, and so contradicts the
  merge method above.

## Comparing against the live settings

The classic branch protection endpoint reports every row above. With the
GitHub CLI authenticated as the founder:

```sh
gh api repos/{owner}/{repo}/branches/main/protection
```

A `404` means no rule exists for `main`, which is the state as of this record.
Otherwise the fields that matter, with other fields present and ignored:

```json
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "Authorship and attribution",
      "Build, test and budgets",
      "Repository checks"
    ]
  },
  "required_pull_request_reviews": {
    "required_approving_review_count": 0
  },
  "enforce_admins": { "enabled": true },
  "allow_force_pushes": { "enabled": false },
  "allow_deletions": { "enabled": false }
}
```

The merge method is repository configuration:

```sh
gh api repos/{owner}/{repo} \
  --jq '{allow_merge_commit, allow_squash_merge, allow_rebase_merge}'
```

expected `{"allow_merge_commit":true,"allow_squash_merge":false,
"allow_rebase_merge":false}`.

If the founder configures `main` through a ruleset rather than a classic rule,
the endpoint is `repos/{owner}/{repo}/rulesets` and the field names differ; the
required values do not. Record which form is in use in the status section.

## What the check reaches, and what only this protection reaches

The hygiene check runs over `origin/<base>..HEAD`: the commits the pull request
would add. That range never contains the merge commit the forge creates when
the pull request lands, because at the time the check runs that commit does not
exist. The same blind spot means an unsigned merge commit reaches `main`
whatever this check says: only branch protection can require a signature on
what lands. So the signature check gates the branch, and only branch protection
gates what lands on `main`.

The merge commits on `main` authored `Carlos Pinto` are no longer an instance of
that blind spot. `decisions/0002` accepts the founder's forge display name as an
author and a committer alongside the canonical one -- two spellings of one
identity, bound by the one address Principle I names -- so those commits are
compliant rather than escaping a rule.

The consequence is worth stating in full. The merge commit the forge creates is
authored with the account's display name, committed by `GitHub
<noreply@github.com>`, and signed with the forge's own key — not with a key in
`.github/allowed-signers`, and it never will be, since the forge does not hold
the founder's key. The signature rule therefore reaches every commit a pull
request adds and does not reach the merge commit above them. What covers that
commit is the forge's signature and the settings in this file, which are what
make it the only thing besides founder-signed commits that can reach `main`.

## Enabling the signature check

What is in place: `scripts/check-commit-hygiene.py` verifies, when given
`--allowed-signers-file`, that every commit in the range carries an SSH
signature that git verifies against a key in that file, under the founder's
address as principal. `.github/workflows/commit-hygiene.yml` passes it the
copy of `.github/allowed-signers` on the base branch. A file with no key entry
is reported as "signing is not yet enabled" and skipped — never failed — so
the mechanism could land before the key without breaking every pull request.
That is the state as of this record: the file carries no key.

What remains, and who can do it: the founder adds their public key to
`.github/allowed-signers` in a signed commit and signs every commit from then
on. The file's own comment gives the git configuration. Nobody else holds the
key, so nobody else can take this step; a change anyone else makes to that
file authorises nothing, because the copy the check trusts is the one already
on `main`.

Three consequences of trusting the base branch's copy:

- The pull request that adds the first key is checked against no key, and
  passes as not yet enabled. Enforcement begins with the next pull request
  after it merges. This is what makes enabling the check immediately safe
  rather than a flag day: commits before it are unsigned and stay that way,
  and the range the check reads only ever contains commits made after it.
- A key rotation is two pull requests: one adding the new key, signed with the
  old; one removing the old key, signed with the new.
- A lost key cannot be replaced by a pull request, because the replacement
  cannot be signed with a key the base branch trusts. Recovering from it is a
  founder decision: temporarily lift the `Authorship and attribution`
  requirement on the forge, land the new key, and restore the requirement in
  the same sitting, with the decision recorded on the pull request.

## Status

| Date | State | Verified how |
| --- | --- | --- |
| 2026-09-02 | Not applied. `main` carries no protection rule; `.github/allowed-signers` lists no key. | Recorded from the repository state; the forge endpoint above has not been queried by this change. |

When the settings are applied, add a row naming the date, whether a classic
rule or a ruleset is in use, and the endpoint output compared. Until a row
says applied, every rule in this file rests on review.
