# The update manifest

- **Task**: T060 (`specs/001-evreos-v1/tasks.md`) / CAR-118
- **Serves**: FR-014, FR-036a, and research §10.1 (plan decision G9)

FR-014 requires the browser to update itself, verifying the update's
authenticity before applying it, and to release to a proportion of users at a
time. Research §10.1 settles how: the update service publishes a signed
manifest carrying a rollout fraction; the client verifies it against a key
pinned in the shipped binary, in addition to the platform's own code signature;
and it decides its own inclusion against a value drawn locally, which never
leaves the machine. This document fixes the manifest's bytes.

It follows research §6.1's reasoning for signed app surfaces, and borrows its
form: a fixed-layout, length-prefixed, domain-separated preimage signed with
Ed25519, verified strictly. It is its own format, under its own domain string,
so a signature made for a surface or an app manifest can never be presented as
an update's.

## Bytes

All integers are unsigned and big-endian. A manifest is its preimage followed by
the 64-byte Ed25519 signature over that preimage.

| Field | Bytes | Meaning |
| --- | --- | --- |
| domain | 17 | `evreos.update.v1` followed by one zero byte |
| platform length | 2 | the length of the next field, 1 to 64 |
| platform | as stated | the build the update is for, ASCII letters, digits, `-` and `_`, as `windows-x86_64` |
| version | 12 | the update's version: major, minor and patch, 4 bytes each |
| artefact size | 8 | the artefact's length in bytes, greater than zero |
| artefact digest | 32 | the artefact's SHA-256 |
| rollout | 4 | the share of installs offered the update, in millionths, 0 to 1,000,000 |
| not after | 8 | the last moment the manifest is accepted, in seconds since 1970-01-01 UTC |
| signature | 64 | Ed25519 over every byte above |

A manifest is read in one pass and every byte is accounted for. It is refused if
it is shorter or longer than its fields make it, if any field is outside its
range, or if its domain is not exactly the one above.

## What the client accepts

In this order, so that nothing from an unverified manifest is acted on:

1. **The signature.** The preimage is verified against the pinned key with
   Ed25519 strict verification, which rejects small-order points and
   non-canonical encodings (research §6.1). A manifest that fails is refused.
2. **The platform.** A manifest for another build than the one running is
   refused.
3. **Expiry.** A manifest past its `not after` is refused. Without it, a
   replayed old manifest could hold an install on an old version forever.
4. **The version.** A manifest naming an older version than the one installed
   is refused as a downgrade. One naming the installed version means the
   install is up to date.
5. **Inclusion.** The install is offered the update only when its locally drawn
   value, a whole number from 0 to 999,999 drawn once and kept on the machine,
   is below the manifest's rollout. Widening the rollout keeps every install
   already included, and a rollout of 0 holds the release.

An offered update is not applied until its artefact has been read whole and its
length and SHA-256 match the manifest's. The platform's own code signature on
the artefact is checked as well before it is applied, in the change that applies
it.

## Keys

The verifying key is 32 bytes, pinned in the shipped binary as a build constant:
residence class B in `specs/001-evreos-v1/data-model.md`. It cannot be fetched,
replaced or extended at runtime. It is rotated only by a release that pins the
new key, signed under the old one. Which key, who holds it and how it is kept
belong to the signing procedure T171 records, not to this document.

## What is not in the manifest

Nothing that names or distinguishes an install. The check fetches the manifest
and sends nothing about the install: no identifier, no rollout value, and
nothing derived from either. FR-036a forbids deriving a device correlator, and
the locally drawn value is the one place such a correlator could otherwise
live.
