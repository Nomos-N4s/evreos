# The update check: byte cost and processor time

- **Task**: T060 (`specs/001-evreos-v1/tasks.md`) / CAR-118
- **Date**: 2026-09-27
- **Status**: Evidence from Linux, not yet the tier-1 figures. Two rules
  decide that, and neither admits this instrument.
  `specs/001-evreos-v1/tasks.md` orders every measurement behind the reference
  machines T013 procures, with decisions/0004 as the one exception, for T015
  and T016 alone. SC-005's processor-time bound is a hardware-dependent
  figure, measured on the pinned runner for its platform and on no other
  machine. The figures below are therefore recorded as the evidence the design
  rests on, and as the baseline the tier-1 runs will be compared against.

## The questions

Principle II asks every feature to state its byte and millisecond cost. T060
adds two: the update path's code, which SC-001 counts, and the processor time
of one update check, which `budgets.toml` bounds at 50 ms under its FR-014
wake. That bound covers the check itself, "the fetch, the signature
verification and the version comparison"; fetching and applying an offered
artefact is work the check hands off.

## Bytes

Measured as `docs/measurements/default-browser-registration.md` measures the
tier-1 bindings: the workspace's release profile on `x86_64-pc-windows-gnu`,
cross-built on Linux with rustc 1.94.1 and MinGW-w64 GCC 13, from a scratch
crate outside the workspace that depends on `evreos-platform` by path and uses
the workspace's `Cargo.lock`. Two builds gave the same bytes.

| Executable | What it reaches | Bytes | Over `io_error` |
| --- | --- | ---: | ---: |
| `io_error` | its arguments, one line of output, and an unwrapped `io::Result` from `std::fs` | 262,144 | — |
| `verify_only` | `io_error`, plus a manifest verified under a pinned key | 335,872 | 73,728 |
| `update_reached` | `io_error`, plus a whole check: the request planned, the rollout draw kept in a file, the manifest decided, an offered artefact checked, and the next check's due time | 361,472 | 99,328 |

Ed25519 verification is most of the cost: 73,728 bytes, the curve arithmetic
and SHA-512 `ed25519-dalek` brings, used without its default features, so with
no precomputed tables. The request, the rollout draw, the decision, the
artefact's SHA-256 and the schedule add 25,600 more.

Against `budgets.toml`: SC-001's Windows entries are unmeasured, since no
installer exists. Nothing shipped reaches the update path at this change,
since the shell does not call it yet, so the cost as shipped is 0 bytes. Once
the shell runs the check it is at most 99,328 bytes on this build, 0.095 MB.
That is an upper bound, since the probe counts the standard library's error
formatting and file handling, which the shell already carries, and its own
printing of the request and the due time.

### The crates they come from

The shell does not depend on `evreos-platform` yet. Once it does, the update
code brings these crates into its graph on `x86_64-pc-windows-msvc` and
`aarch64-apple-darwin`, found by comparing `cargo tree -e normal,build
--target <triple>` for `evreos-shell` and for `evreos-platform`:

- **Linked:** `ed25519-dalek`, `ed25519`, `signature`, `curve25519-dalek`,
  `subtle`, `sha2`, `digest`, `block-buffer`, `crypto-common`,
  `generic-array`, `typenum`, `cpufeatures`, `cfg-if` and `getrandom`,
  fourteen in all. The shell links none of them on Windows or macOS today,
  and the bytes above include them all. Nine were already in the lockfile,
  through other crates: `sha2`, `digest`, `block-buffer`, `crypto-common`,
  `generic-array`, `typenum`, `cpufeatures` and `cfg-if`, with `sha2`
  through a test spike's `wry`, and `getrandom` 0.3.4 through `winit` on
  Linux. The other five are new to it,
  and come with `ed25519-dalek`, which adds seven entries to the lockfile in
  all: these five, `curve25519-dalek-derive`, and `fiat-crypto`, which
  nothing links on these targets.
- **At build time only:** `rustc_version`, `semver` and `version_check`, and
  on Windows `curve25519-dalek-derive` too. None reaches the executable.

`windows-registry`, which the same comparison also lists on Windows, is
T059's, recorded in `docs/measurements/default-browser-registration.md`.

## Processor time

A release build with the workspace's size-optimised profile, on this change's
Linux instrument, a cloud container with an Intel Xeon at 2.80 GHz. It timed
2,000 checks in a row, three times, each check reading the kept rollout draw
from its file and deciding a signed manifest, which includes its strict
verification:

| Round | Each check |
| --- | ---: |
| 1 | 2.73 ms |
| 2 | 2.71 ms |
| 3 | 2.73 ms |

The work is single-threaded and does not block, so the wall-clock time per
check stands in for its processor time here. That is 5% of the 50 ms bound.
The fetch is not included: no transport exists yet, since `evreos-net` plans
requests and sends none.

## Owed

- The byte figures for `x86_64-pc-windows-msvc` on the tier-1 pinned runner,
  and the marginal cost inside the shell once it runs the check.
- The processor time of one whole check, the fetch included, measured as
  processor time on the tier-1 and tier-2 pinned runners.

## Reproducing

The bytes: add two more executables to the scratch crate
`docs/measurements/default-browser-registration.md` describes. `verify_only`
calls `VerifiedManifest::verify` under an `UpdateKey`. `update_reached` calls
`check_request`, `RolloutDraw::load_or_draw`, `decide`, on an offered update
`artefact::verify`, and `Schedule::next_due`. Each takes its inputs from its
arguments, so nothing is folded away.

The time: a crate outside the workspace with the same release profile, less
`strip`, depending on `evreos-platform` and on `ed25519-dalek` 2.2 without
default features. It signs one manifest with a fixed key, as
`docs/formats/update-manifest.md` lays it out, and then times 2,000
iterations of `RolloutDraw::load_or_draw` and `decide`.
