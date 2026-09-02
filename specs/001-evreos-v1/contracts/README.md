# Contracts: Evreos v1

**Feature**: `001-evreos-v1` · **Phase**: 1 (design) · **Source of truth**:
`specs/001-evreos-v1/spec.md`, `.specify/memory/constitution.md`,
`docs/adr/0001-rendering-engine.md`

Evreos is a desktop application. It exposes no HTTP endpoints of its own, so
this directory is not an API surface description. What Evreos has instead are
six interfaces across which two parties must agree, where one of them is outside
the change that would break the agreement: a trait two implementations must mean
the same thing by, three signed or encrypted formats produced by one party and
consumed by another, one repository file a CI gate reads, and one service in a
different repository the client depends on.

Those six are the contracts, and this document is their index and definition.

| # | Contract | Format | Produced by | Consumed by | Status |
| --- | --- | --- | --- | --- | --- |
| 1 | The `Engine` trait | Rust trait, `crates/evreos-engine` | Each rendering backend | The shell | Implemented at M0; seven changes owed before the first real backend |
| 2 | The app manifest | Signed, versioned declaration | The app publisher | The shell's verifier and capability intersection | Not built |
| 3 | The delivered app surface | Signed bundle over a fixed preimage | The app publisher, served by the delivery host | The shell's verifier, then the engine | Not built |
| 4 | The diagnostic report formats | Encrypted, padded payloads through a relay | The client | The receiving service's gateway | Not built; gated on a relay contract (**FR-039b**) |
| 5 | The budget file schema | TOML, `budgets.toml` | Pull request authors and recorded founder decisions | `scripts/check-budgets.py`, the release job, SC-013 publication | Implemented in part; four entries of eighteen |
| 6 | The Apivo API surface | HTTP, in another repository | The Apivo service | The shell's money and catalogue surfaces | External dependency, not a design |

## How to read this document

Each contract carries five fixed parts, and they are the parts a contract needs
rather than the parts a design document usually has:

- **Format** — the shape of the thing crossing the boundary, at field level
wherever a requirement fixes the field.
- **Invariants** — what must hold for the contract to be honoured, each stated
  so
that a build can be held to it.
- **Produced by / consumed by** — the two parties, named, because a contract
  with
one party is a data structure.
- **What breaks if it changes** — the concrete consequence, not "callers may
  need
updating".
- **Change control** — whether a change is an ordinary code change, a founder
decision, or an amendment to the specification.

Citation discipline is the same as `data-model.md`'s and is not decorative.
Where a field, a constraint or a value exists because a requirement says so, the
requirement is named at it. Where the design chose it, it is marked
**[design]**. Where the design needs something the specification does not
require, it is marked **[gap]** and repeated in *Gaps* at the end. Nothing here
is presented as required unless a requirement is named for it. Where Phase 0
research established a fact about a platform or a dependency, the finding is
cited as established; where it recorded something as unestablished, it stays
unestablished and appears as an open question rather than as a decision.

**What is deliberately not a contract here.** Evreos exposes no extension API:
**FR-031** requires the wallet delivered as part of the shell and never through
an extension mechanism, and hosting third-party extensions is a Non-Goal on the
evidence ADR-0001 records. The distribution page is verified before each release
(**FR-041**) but is a published artefact rather than an interface two parties
agree on. The FR-035 interface catalogues, the FR-042 brand configuration and
the FR-039c crash-reason enumeration are build constants, and each is described
where it is used rather than given a section of its own — except the reason
enumeration, which is part of contract 4 because a party outside this repository
reads it.

---

## 1. The `Engine` trait

**Forced by**: **Principle III** (rendering goes through an interface the shell
defines as the consumer, with a headless implementation kept working from day
one, so the seam is proved by a second implementation rather than asserted),
**FR-044** (the same, from milestone M0), **FR-015**, **SC-009**, **ADR-0001**.

This contract is implemented. It lives at `crates/evreos-engine/src/lib.rs` on
`feat/engine-seam`, with a second implementation at
`crates/evreos-engine-headless` and its consumer at `crates/evreos-shell`.

### Format, as merged

```rust
pub struct Request { address: String }          // address(&self) -> &str
pub struct Page    { address: String, title: String }

pub enum LoadError {
    Unresolvable        { address: String },
    Certificate         { address: String, detail: String },
    Intercepted         { address: String },
    AuthenticationRequired { address: String },
}

pub trait Engine {
    fn name(&self) -> &'static str;
    fn load(&mut self, request: &Request) -> Result<Page, LoadError>;
    fn current(&self) -> Option<&Page>;
}
```

`Request` carries the address as a string rather than a parsed URL type. Parsing
and policy belong to the shell, which owns **FR-003**'s combined entry field and
**FR-007a**'s rule on what may leave the machine; an engine that parsed
addresses would be taking those decisions where they cannot be reviewed. That is
the seam's own stated reason and it is a design decision the merged code
records. **[design]**

### Invariants an implementation must satisfy

1. **FR-015's four causes are distinguishable.** `LoadError` is closed at
   exactly
the four failures **FR-015** enumerates — an unresolvable address, an untrusted
or expired certificate, an intercepting network, and a request for
authentication. A catch-all variant would let an implementation report every
failure as one indistinguishable cause, which is the state FR-015 exists to
forbid. **SC-009** requires each of the four exercised on every supported
platform, which is why the closure is a contract clause and not a style
preference. Adding a cause is a change to this enum and to **FR-015**.

2. **A failed load never becomes the current page.** An implementation MUST NOT
report success for a load that did not produce a page, and a failure MUST NOT
replace what `current()` returns. **FR-015** names both halves: "treating a
failed load as a successful empty page is a defect", and a failure that silently
blanks the page the member was on is the same defect one step later. The
headless implementation holds this today — on a scripted failure it returns
`Err` and leaves `current` untouched — and
`crates/evreos-shell/tests/navigation_failures.rs` is the exercise.

3. **The address reported is the address that loaded, not the address
   requested.**
`Page::address` is what actually loaded, which may differ from the request after
a redirect, and the shell displays that rather than what was typed. **[design]**
— no requirement states this in terms; the merged seam's own doc comment gives
the reason, that "showing the request while displaying the response is how an
address bar lies", and **FR-015**'s prohibition on presenting a failure as a
success is the nearest requirement in spirit. It is recorded here as a design
decision so a reviewer is not left inferring a requirement that does not exist.

4. **Nothing in this crate names a platform, a runtime or a vendor**, and
   nothing
returns a handle to one. **Principle III** places the interface with the shell
as consumer; a seam that leaks its default implementation's vocabulary is a seam
only until the second implementation arrives.

5. **Failure is a value, not a panic.** A trait that can only succeed or panic
cannot carry **FR-015**.

6. **The second implementation is kept working from M0** (**FR-044**), which
   makes
every change to this contract a change to two implementations in one commit.

### Invariants the contract does not yet carry, and owes

Phase 0 examined the merged trait against both shipping backends and against the
requirements already merged. Four properties are required by requirements or by
ADR-0001 and have no expression in the contract as written. They are recorded
here as owed clauses rather than as new requirements, because none of them adds
a requirement — each is an existing one the interface currently cannot express.

- **Navigation is not one outcome per call.** `load` is synchronous, and neither
shipping backend can produce a navigation outcome synchronously; on both tiers
the engine is affine to the UI thread, so the only implementation route for a
synchronous `load` is a nested message loop on that thread, which **SC-006**
admits no trial over 16 ms for. Separately, engine-initiated navigation — link
clicks, script, form posts, redirects — produces no `load` call at all, so
invariant 3 above is unattainable at the moment it matters most; there is no
in-flight state, so **SC-009**'s "zero loading indicators that do not resolve
within 30 s" is not testable against this trait; and there is no correlation
between a request and its outcome. The replacement carries a navigation id and a
small closed event set, with `LoadError` unchanged inside the failure event.
**[design]** — the requirements are FR-015, SC-006 and SC-009; the shape is the
plan's.

- **`Intercepted` is not derivable from any platform error code on either
  shipping
tier.** Phase 0 established this exhaustively against both platforms' own
enumerations. A captive portal answers, so the navigation succeeds; detecting it
is a shell-level inference, and any probe-based inference is an outbound request
**FR-007a** governs and **Principle VI** constrains. The contract clause is that
a backend never synthesises `Intercepted` from a platform error code; the
headless implementation scripts it, so **SC-009**'s fourth case stays testable.
Whether the shell may make a probe at all is a founder decision. **[gap]**

- **A host above `Engine` owning the shared platform context.** ADR-0001 records
as an accepted cost that "environment sharing must be an explicit requirement of
the `Engine` trait, on Windows", because the default is one context per view.
There is no construction seam in the trait at all. Ten tabs each minting their
own context is the shape that loses **SC-004** before product code is written,
and the interface currently cannot say otherwise. That accepted cost is scoped
to Windows and stays scoped there: ADR-0001 records in the same bullet that
"Sharing a process pool is not the macOS equivalent: that interface has been a
documented no-op for several OS versions and has no binding in `wry`", and that
"what actually governs macOS memory at ten tabs is unestablished and belongs to
the spikes" — ADR-0001 risk 9, which is also why **SC-004**'s tier-2 entry is
provisional. So the trait owes a construction seam on ADR-0001's terms for tier
1; what such a seam would share on tier 2, and whether sharing is the mechanism
there at all, is unestablished and is not established here. **[gap]**

- **No `Send` bound anywhere on the engine path.** Both shipping tiers are
UI-thread-affine, so a `Send` bound is unimplementable on either and is cheaper
to forbid now than to unwind later. **[design]**

Three further additions are owed by requirements outside FR-015, and are listed
here so the seam is grown once rather than three times: a rendering-surface
handle (create, activate, suspend, close, and a data-store selector) because
**FR-001**, **FR-002**, **FR-007** and **FR-016** each need independently
addressable contexts and the merged trait is single-surface; a blocking-policy
surface (install or replace a compiled policy, exempt a site, report what was
blocked) because **FR-008** requires blocking active on first launch and a
visible per-site control, and because a per-request veto is implementable on
tier 1 and not on tier 2, so a veto-shaped method would make the seam
Windows-shaped; and a way to host a verified surface from shell-supplied bytes
under a shell-chosen identity, because **FR-019a** requires verification before
rendering, which means the bytes reach the engine from the shell rather than
over the wire.

### Produced by / consumed by

**Implemented by** the system-webview backend on each supported platform and by
`evreos-engine-headless`. **Consumed by** `evreos-shell`, which owns the
interface: **Principle III** places definition with the consumer, so a backend
never widens this contract to expose something a platform happens to offer.

### What breaks if it changes

A change to `LoadError`'s variants changes what **SC-009** exercises and what
**FR-015** requires named, so it is a specification change and not a refactor. A
change to the trait's methods must land in both implementations in the same
commit, or **FR-044**'s "kept working from M0" is broken for the duration. A
change that lets a backend return a synthetic `Page` at navigation start is the
defect **FR-015** names by its own words, whatever it is called. ADR-0001's
revisit triggers record that the trait's swap intention "is untested until a
second real backend exists beside the headless one" — the first real backend is
therefore the test of this contract, and Phase 0's finding is that test arriving
early.

### Change control

Variants and their meaning: specification amendment. Method set and signatures:
ordinary change, in one commit with both implementations, stating its cost under
**FR-043**. Anything that makes a property **SC-004**, **SC-006** or **SC-009**
depends on invisible at the seam: not an ordinary change, because the second
implementation exists precisely to convert those assumptions into facts.

---

## 2. The app manifest

**Forced by**: **FR-017** (each app declares its capabilities in a signed,
versioned manifest and MUST NOT be able to widen them from inside), **FR-018**
(every declarable capability classified in the published catalogue; an
unclassified capability is never granted), **FR-016a** (dismissal is keyed to
the app identity declared in that manifest; the identity MUST NOT change across
an update, and an app republished under a new identity is treated as dismissed
for everyone who dismissed its predecessor), **FR-019a** (the surface signature
binds the manifest's digest), **FR-036a**, **Principle IX**.

### Format

| Field | Type | Forced by |
| --- | --- | --- |
| `app_id` | app identity | **FR-016a** keys dismissal to it; **FR-019a** binds it into the surface signature |
| `manifest_version` | version | **FR-017** ("versioned") |
| `declared_capabilities` | set of capability names | **FR-017** |
| `signature` | signature over the manifest preimage | **FR-017** ("signed") |
| `digest` | SHA-256 of the manifest bytes | **FR-019a** binds this value, so it must be well defined |

The signing construction, and the choice of SHA-256 and Ed25519, are
**[design]**: **FR-017** requires the manifest signed and **FR-019a** requires
one signature over a four-part binding, and neither fixes an algorithm or an
encoding. The design signs a fixed-layout, length-prefixed, domain-separated
preimage rather than a canonicalised JSON document, so that a reviewer reads the
whole preimage builder rather than a canonicalisation rule; the manifest's
domain string is `evreos.manifest.v1`, distinct from the surface's, so a
manifest signature cannot be presented as a surface signature. Verification is
strict — small-order public keys and points rejected, non-canonical encodings
rejected — so that one surface does not have two valid signature encodings a
delivery host could hold. If the plan later adopts a standard container instead,
that is a change to this contract and not to any requirement.

### The two artefacts beside it

- **The capability catalogue** — a **build constant**, shipped in the release,
classifying every declarable capability page-adjacent or not. **FR-018**
requires the catalogue published with the manifest format and requires that a
capability it does not classify is never granted; shipping it in the release
rather than fetching it is what makes that hold, because a fetched catalogue is
one the delivery host can extend. **[design]**
- **The app registry and its publishing delegation** — carrying, per `app_id`,
  the
publishing key, the capability ceiling, and any `supersedes` relation.
**[design/gap]**: the ceiling is stronger than **FR-017** requires. FR-017
forbids an app widening its capabilities *from inside*; it does not bound what a
signed manifest may declare. Without a ceiling, compromise of a publishing key
yields a manifest declaring every catalogued capability. Recording `supersedes`
here rather than in the app's own manifest is what makes **FR-016a**'s
anti-rename rule bind the publisher, since a lineage field written by the party
the rule constrains holds only while convenient; the honest residual is that it
does not bind the holder of the root key, where the remaining guarantee is that
the registry is version-controlled and reviewed.

### Invariants

1. **An app cannot widen its capabilities from inside** (**FR-017**). The
   effective
set is computed by the shell and never read from anything the app controls at
runtime. The design computes it as an intersection of the shipped registry
ceiling, the shipped catalogue, the verified manifest's declaration, and the
member's per-app grants; a name outside the shipped catalogue is never granted
(**FR-018**), and a capability the shipped registry does not list for that
`app_id` is never granted however the manifest is signed **[design]**.
2. **Page-adjacent capabilities additionally require a per-app grant**, asked
   for
when the capability is first used (**FR-018**, Story 3 scenario 5). "Touches
page content" is narrower than Principle IX's "anything page-adjacent" and MUST
NOT be substituted for it: reading the current tab's URL touches no page content
and requires the grant.
3. **`app_id` does not change across an update** (**FR-016a**), and dismissal
follows the identity rather than a home-surface slot.
4. **No manifest may declare a capability whose effect is to place device,
display, font, network or timing characteristics in the hands of a party that
could derive an identifier or correlator**, including at one remove by
forwarding the inputs (**FR-036a**). A per-app grant authorises page-adjacent
access and never this.
5. **A grant never authorises injection.** **FR-018a** is explicit that a
   per-app
grant authorises an app to *respond* to a qualifying action and does not
authorise injection in its absence.

### Produced by / consumed by

**Produced by** the app publisher, signed under a key the root-signed delegation
authorises. **Consumed by** the shell's verifier and by the capability
intersection. Neither the delivery host nor the app itself is a party that may
alter what the manifest means after signing.

### What breaks if it changes

A change to `app_id` is a new app: every dismissal for its predecessor must
follow it (**FR-016a**), or an operator clears every dismissal by renaming,
which **Principle IV** makes a release blocker rather than a bug. A change to
the manifest's byte layout changes its digest, which the surface signature binds
(**FR-019a**), so every surface signed against the old digest is refused —
correct behaviour, and the reason a manifest and its surfaces are republished
together. A capability name that reaches a manifest without reaching the shipped
catalogue is never granted (**FR-018**), which makes an unclassified capability
a silent no-capability rather than an error; that is the intended failure
direction and should be visible to the app, which is why an app must be able to
ask the shell what it actually holds. **[design]**

### Change control

Adding a capability *name* to the catalogue: a browser release, since the
catalogue is a build constant. Widening an app's ceiling: a browser release,
defensible because **Principle IX** keeps app *content* off the release cycle
and what an app may do is not content. Changing the classification of a
capability from non-page-adjacent to page-adjacent: ordinary change. The
reverse: an argument against **FR-018**'s definition, which is a specification
question.

---

## 3. The delivered app surface

**Forced by**: **FR-019** (updatable without a browser release), **FR-019a**
(signed; verified before rendering *or* before writing to the FR-020 cache;
three named properties), **FR-019b** (no app surface and no cached copy ships in
an installer or a browser update; the cache is populated only from surfaces
delivered after installation), **FR-020** (a stated offline state rather than a
blank surface), **Principle IX**.

### Format

| Field | Type | Forced by |
| --- | --- | --- |
| `app_id` | app identity | **FR-019a** — must equal the app about to be rendered |
| `surface_version` | monotonic integer | **FR-019a**'s no-downgrade comparison. **[design]** — a monotonic integer rather than a display version, so ordering is total and unambiguous |
| `bytes` | surface content | Never on the release path (**FR-019b**) |
| `bytes_digest` | SHA-256 of `bytes` | The value the signature covers |
| `manifest_digest` | SHA-256 of the FR-017 manifest | **FR-019a** — must be the manifest whose capabilities it would run under |
| `signature` | one signature over the whole binding | **FR-019a** |

**FR-019a** requires "a single signature over the surface bytes, the app's
identity, the digest of that app's FR-017 manifest, and the surface version
together". The design makes *together* literal by signing one preimage in which
the four sit as length-prefixed fields, so none can be substituted without
invalidating the signature and no concatenation ambiguity exists between
adjacent fields:

```
DOMAIN("evreos.surface.v1\0")
  || len || app_id
  || len || manifest_digest      (SHA-256)
  || u64_be surface_version
  || len || surface_digest       (SHA-256 of bytes)
  || u64_be not_after
```

**[design]** for the layout, the domain separation and the algorithm.
**[design]** also for `not_after`: **FR-019a** requires no expiry, and one is
added because whoever controls delivery can otherwise replay a correctly signed
current surface indefinitely and undetectably. Its scope is deliberately narrow
— it bounds *acceptance of a fetched surface* and never rendering, because an
expiry that stopped a cached surface rendering would convert **FR-020**'s
offline guarantee into a blank surface after some interval, which is the outcome
FR-019a and FR-020 both name as the failure.

### Invariants

**FR-019a**'s three properties, each stated as the check that fails:

1. **Pinned trust root.** The root is pinned in the shipped shell and is never
fetched, replaced or updated from the host that serves the surface or any host
under the same control. A change of root reaches the member only in a browser
release. Otherwise a compromised host serves its own root alongside its own
modified surface and the client verifies both.
2. **One signature over the whole binding.** Refuse a surface whose signed app
identity is not the app about to be rendered, or whose signed manifest digest is
not the manifest whose capabilities it would run under. Otherwise one app's
signed surface runs with another app's capabilities.
3. **No downgrade.** The delivered version MUST be greater than or equal to the
cached copy's; a lower version is refused, the cached copy retained, and the
refusal stated.

And, from the requirements around it:

4. **Verification precedes both rendering and the cache write** (**FR-019a**),
   so
unverified bytes never reach the cache.
5. **An unverifiable surface is refused, the cached copy retained, and the
   refusal
stated** — not shown as a blank surface (**FR-019a**, **FR-020**).
6. **No surface and no cached copy ships in an installer or an update**
(**FR-019b**). This is not enforceable by signature: **FR-019b** says so itself
— a pre-cached surface would carry a valid signature and would satisfy FR-019a.
Enforcement is therefore by provenance and by artefact contents. The design's
four checks: a verified-surface type whose only producer takes bytes handed over
by the delivery client; a release-artefact scan, in the idiom of
`scripts/check-budgets.py`, that fails the release job on any surface bundle,
app manifest or surface-cache path in the installer or the installed tree; a
post-install acceptance test that finds the cache absent or empty before any
network activity and every app presenting **FR-020**'s offline state; and an
SC-014-style capture asserting that the first render of a surface is preceded by
that surface's delivery fetch. **[design]**
7. **A downgrade floor survives a cache clear.** **[design/gap]** — **FR-019a**
compares against "the version of the cached copy it would replace", and on a
fresh install or after a cache clear there is no cached copy and so no floor.
Holding the floor in a store separate from the FR-020 cache closes that window
and is stronger than the specification requires; the honest residual is that the
floor is a local file and an attacker with profile write access can lower it,
and that attacker already has the machine.

### On FR-007a and the surface fetch

A surface fetch carries an `app_id` and a version. **FR-007a**'s closed
enumeration is over transmissions that carry an address the member navigated to,
a term typed into the FR-003 field, page content, or a value derived from any of
those, and a surface fetch carries none of them — so on the enumeration's own
subject matter it is not a transmission that list is about. That settles one
half of the question and not the other. FR-007a's conformance clause is not
scoped to history-bearing payloads: it requires a network-capture test committed
to this repository and run in CI, and that test "MUST fail on any outbound
request from Evreos that no entry above accounts for". A surface fetch is an
outbound request from Evreos that no entry above accounts for, so as FR-007a is
written a conforming capture test fails on it — whether or not it carries
anything the enumeration is about. The two halves of the requirement point
opposite ways, and nothing here may settle which governs. **SC-014**'s stated
test is over "every URL-bearing payload" in the capture, which read literally
reaches a surface fetch and the **FR-014** update check as well, and lands in
the same place. Which reading governs is a founder decision, recorded in *Gaps*
(**G7**). It is not an implementer's call: it changes what FR-007a's conformance
test and the criterion both mean.

### Produced by / consumed by

**Produced by** the app publisher and served by the delivery host — two roles
the contract deliberately keeps distinct, because the delivery host is untrusted
by construction and the publisher is trusted only through the root. **Consumed
by** the shell's verifier, and then by the engine, which receives
already-verified bytes from the shell rather than fetching them (**FR-019a**).

### What breaks if it changes

A change to the preimage layout invalidates every signature made under the old
one, so the domain string carries a version and a client accepts exactly the
versions it ships support for. Serving a surface whose signed identity or
manifest digest does not match refuses the surface and retains the cached copy —
the member sees a stated refusal, not a blank screen, and the app does not
silently run with another app's capabilities. Losing the pinned root's private
key is recoverable only by a browser release on a staged **FR-014** channel;
that residual is accepted and stated rather than engineered around.

### Change control

Preimage layout or algorithm: ordinary change, with a version bump in the domain
string and a stated cost under **FR-043**. The pinned root: a browser release,
by **FR-019a**. The rule that the cache is populated only after installation:
**FR-019b**, a specification amendment.

---

## 4. The diagnostic report formats

**Forced by**: **FR-039** (opt-in, off until the member turns it on; every
transmission and its occasion disclosed in plain language before consent; the
signal carries no browsing history, URLs, page content or search terms),
**FR-039a**, **FR-039b**, **FR-039c**, **FR-039d**, **FR-039e**, **FR-039f**,
**Principle VI** (opt-in, aggregate, EU-hosted), **Q-E6**, **Q-E16**.

Four payloads, and the list is closed by **FR-039a** and **FR-039c** together:
enrolment, retention, withdrawal, crash. **Q-E6** settles the diagnostic set as
"retention reports (FR-039a) and crash counters (FR-039c, FR-039d), **and
nothing else**", and states that a further datum "requires its own justification
under Principle VI before it may be added, **which is an amendment to this
specification rather than an implementation choice**". Both limbs are its own
words. So adding a fifth payload, or a field to any of the four, is a change to
this specification made in the pull request that would add it — never a decision
taken while implementing. FR-039a's and FR-039c's exhaustive enumerations of
their contents are what make that checkable field by field.

### 4.1 Enrolment, retention and withdrawal reports

| Field | Type | Forced by |
| --- | --- | --- |
| `kind` | `Enrolment \| Retention \| Withdrawal` | **FR-039a** |
| `enrolment_week` | ISO-8601 week, Monday–Sunday, UTC | **FR-039a** ("carrying only the enrolment week"); Assumptions fix the week |

That is the whole payload. **FR-039a** says "carrying only the enrolment week"
of the enrolment report and "Both carry only the same enrolment week" of the
other two, so a field added here is a change to that requirement.

At most one enrolment per install and at most two reports per enrolment
(**FR-039a**). Both are keyed to enrolment, never to install; nothing about the
install date is transmitted.

### 4.2 The crash report

| Field | Type | Forced by |
| --- | --- | --- |
| `frames` | list of `{ module, symbol, file, line }` | **FR-039c** — only these four, drawn from Evreos's own debug information |
| `browser_version` | string | **FR-039c** |
| `os_version` | coarsened version | **FR-039c** requires the version; the granularity is **[design/gap]** |
| `reason_code` | value from the closed enumeration committed to this repository | **FR-039c**; a code outside it is discarded on receipt rather than counted |

Forbidden by **FR-039c**, and each named because it is where a URL otherwise
arrives: frame arguments, register contents, strings read from the heap or the
stack, full process memory, and a free-form reason string — "a field that
accepts arbitrary text accepts a URL". The design's consequence, marked
**[design]** because FR-039c bans the contents and not the capture shape: no
minidump of any kind is written, not even a minimal one, because a minidump
includes thread stack memory and FR-039c's own reasoning for banning full memory
applies to a partial image too. What is captured is a return-address list, which
is provably free of those contents by construction, symbolised on the next
launch from a symbol table shipped in the installer.

Symbolisation on the device is settled by the specification rather than chosen:
**FR-039c** enumerates the three things a frame may carry, and a raw address or
a module-relative offset is not among them; a client-queried symbol server would
additionally be a per-crash request keyed to the crashing code path that no
entry in **FR-007a**'s closed list accounts for.

An engine-process failure produces no Evreos stack, because on tier 1 web
content does not run in Evreos's process. The reason enumeration therefore
carries two disjoint families — Evreos-process reasons, and engine-process
failure kinds mapped one-to-one from the runtime's own closed enumeration — and
an engine-process report carries a reason code with an **empty frame list**,
which **FR-039d**'s counter key tolerates because the key is the symbol list
itself. **[design]**

**The crash-reason enumeration is itself part of this contract.** **FR-039c**
requires it committed to this repository and makes adding a code a change to
that file; the receiving service reads it to decide what to discard on receipt.
The `os_version` granularity is committed beside it, so widening it is as
visible as adding a code. **[design/gap]**

### 4.3 The envelope

**Forced by** **FR-039b**, which fixes the arrangement rather than the protocol:
no report carries an identifier; identifier-free holds at the network layer too;
reports reach the service through a relay that is structurally unable to read
what it forwards; the client encrypts to the receiving service's public key,
**pinned in the build and rotated only by a release**; the relay sees only the
ciphertext, its length and the destination; the relay is operated by a legal
entity distinct from the one operating the receiving service, named with its
jurisdiction in the pre-consent disclosure, under a written contract whose
existence and effective date are stated there.

The design implements this as Oblivious HTTP over HPKE, with one deliberate
departure: the gateway key configuration is compiled into the release rather
than fetched at runtime. The departure is required and is strictly stronger —
the common deployment fetches key configurations from the gateway, which is an
unencapsulated request from the client straight to the gateway revealing exactly
the source address the relay exists to hide, and one **FR-007a**'s closed list
does not account for. **[design]**

Two further envelope properties are **[design]**, added because **FR-039b**
permits the relay to see length and destination but does not require length to
be *informative*: every payload is padded to one fixed ciphertext length
identical across all four kinds, and at most one report is sent per connection.
With four kinds of naturally different size, a relay that does see the source
address would otherwise learn, per address, whether that member withdrew or
crashed; and batching would let it link a crash and a retention report to one
address by co-occurrence. The fixed size must exceed the largest crash report,
which is a cost stated under **FR-043** and an argument for bounding the
captured frame count.

### Invariants

1. **No identifier of any kind, in any report** (**FR-039b**) — and the client's
   own
local state holds no generated value either: no UUID, no nonce, no salt, no
counter, no hash. **[design]** — what makes a value an identifier is that it has
more entropy than the calendar, and "no generated value exists" is checkable
where "we will not send the generated value" is a promise.
2. **No browsing history, URL, page content or search term**, in the diagnostic
signal (**FR-039**) or in a crash report (**FR-039c**), on **FR-007a**'s
definition of browsing history, which **FR-007a** states is the definition every
other requirement uses.
3. **No retransmission.** An unacknowledged enrolment is abandoned and emits
nothing further (**FR-039b**), because **FR-039d** counts on receipt with no
identifier, so a retransmission cannot be deduplicated and would inflate the
denominator. The acknowledgement is encapsulated, so acceptance and rejection
are indistinguishable to the relay **[design]**.
4. **Counters, not reports, are the retained artefact** (**FR-039d**). A report
   is
added to its counter on receipt and discarded by the end of the following
calendar day. The two permitted counter keys — `(report type, enrolment week)`
and `(symbolised stack, release, operating-system version, reason code)` — are
the only ones, and adding or widening a key is a specification amendment.
5. **At most one crash report per key for the life of the install**, enforced on
the device and stated in the pre-consent disclosure (**FR-039e**). A per-day cap
would not carry the claim.
6. **Received, processed and retained only on EU infrastructure** (**FR-039f**),
which reaches the relay ingress, the gateway, the counter store, the publication
pipeline, and the gateway's own operational instrumentation. The last is the one
that gets missed and fails two requirements at once: a per-request record
exported to an observability vendor carries a receipt timestamp finer than the
day, which **FR-039b** bans wherever it is hosted.
7. **Publication is governed by FR-039e**, and the disclosure unit is a crash
   stack
or an enrolment week, gated on the week's enrolment count less its withdrawal
count. Below the threshold a unit is held whole and nothing drawn from it may be
published in any form, not a count, not a rate, not a range, not an interval.

### Produced by / consumed by

**Produced by** the client, only where the member has turned the signal on
(**FR-039**), and crash reporting only where it has been separately consented
(**FR-039c**). **Forwarded by** a relay operated by a distinct legal entity
under a written contract (**FR-039b**). **Consumed by** the receiving service's
gateway, which increments a counter and discards the report.

### What breaks if it changes

Adding a field to any of the four payloads is a change to **FR-039a** or
**FR-039c**, both of which enumerate contents exhaustively. Changing the pinned
key without a release makes every install that has not yet updated undecryptable
and loses a cohort invisibly, which biases the retention figure by an amount
nobody can see; the design pins two configurations per release, current and
next, so rotation has a release cycle of overlap **[design]**. **The whole
contract is gated on a relay contract**: **FR-039b** states that where no
operator is named or no contract is in force, "the diagnostic signal MUST NOT be
offered and no report may be transmitted". The milestone that ships diagnostics
must therefore be able to ship with the feature dark and no report path present
at all — which is also the state **SC-014**'s capture exercises on a fresh
profile.

### Change control

Payload fields: specification amendment (**FR-039a**, **FR-039c**). Counter
keys: specification amendment (**FR-039d**). Reason codes and the `os_version`
granularity: a change to the committed enumeration file, visible in review, per
**FR-039c**. The pinned key: a release, per **FR-039b**. The relay operator: a
contract and a change to the pre-consent disclosure, per **FR-039b**.

---

## 5. The budget file schema

**Forced by**: **Principle II** (hard budgets live in one budget file and are
enforced by CI gates that fail the build on regression), **FR-043** (the same,
naming the file, and requiring every pull request that changes a measured
quantity to state its byte and millisecond cost against it), and the Success
Criteria preamble, which defines the entry, the three gates and the closed list
of entries and states that the gates are defined there and only there.

This contract is implemented in part. The file is `budgets.toml` and the gate is
`scripts/check-budgets.py`, both on `feat/budget-gate`.

### Format, as it stands

```toml
[meta]      milestone, spec
[runners.tier1] platform, os_floor, model, identity
[runners.tier2] platform, os_floor, model, identity
[[entry]]   criterion, name, platform, figure_mb, status, condition,
            baseline_mb, tolerance_pct
```

### The contract the gate enforces today

`check-budgets.py` implements the three gates the preamble defines and adds none
of its own. Its enforced clauses, verified against the script:

- **Budget-file gate** (unconditional from M0, needs no hardware): runners must
  be
declared and each must carry a non-empty durable `identity`; entries must exist;
no `(criterion, name, platform)` may be declared twice; `status` must be
`ratified` or `provisional`; `figure_mb` and `baseline_mb` must both be present;
a `baseline_mb` above the entry's `figure_mb` fails, because a provisional
figure binds a baseline exactly as a ratified one does; `tolerance_pct` may not
exceed 5% and may not be negative.
- **Absolute gate**: fails when a measured figure exceeds the entry's stated
figure; advisory rather than blocking on a hardware-dependent entry (SC-002,
SC-004, SC-005, SC-006) while that tier's runner is unpinned.
- **Regression gate**: fails when a measured figure is worse than the baseline
  by
more than the declared tolerance. It compares one machine against itself, so it
blocks from M0 on every entry.
- **An undeclared tolerance is zero, not unbounded** — the opposite reading lets
  an
entry disable its own regression gate by omission.
- **An unmeasured entry is reported as unmeasured and is not a pass**, which the
script now enforces rather than only asserting. `run_gates` classifies every
entry it holds no measurement for. The one honest exception is a
hardware-dependent entry — SC-002, SC-004, SC-005 or SC-006 — whose tier has no
pinned runner: there is no machine to measure it on, the budget-file gate
already reports that runner's missing identity, and the entry is listed with
that reason and not blocked. Every other unmeasured entry is classified
BLOCKING, printed as a `FAIL [budget file]` line and appended to
`file_gate.blocking`, so the run fails on it. The single deferral is
`--allow-unmeasured`, which suppresses exactly that clause and nothing else; the
build workflow passes it today on the blocking invocation, visibly and naming
what retires it — the harness that produces each figure, today SC-001's
installed footprint, which needs an installer that does not exist yet.

### What the preamble requires and the file does not yet carry

Recorded as the contract's current state rather than as a criticism, because the
schema must land before the measurements do.

| Owed | Required by |
| --- | --- |
| Fourteen of the eighteen entries — SC-002 warm and cold, SC-004 ten-tab, SC-005 window and wake-free sample, SC-006 tab switch and keystroke, per platform | The preamble's closed list; the budget-file gate fails on a stated entry that is missing |
| A `unit` beside the figure — SC-002 and SC-006 are milliseconds, SC-005 is a percentage of one core plus two processor-time bounds; the schema is `figure_mb` throughout | The preamble's entries |
| `founder_decision` on every `ratified` entry | The budget-file gate fails on "a ratified entry naming no founder decision" |
| `cross_check_margin` on SC-004 | **SC-004**; an undeclared margin is zero |
| The SC-005 wake enumeration, each wake carrying a period, a processor-time bound and a justifying requirement | **SC-005**; the budget-file gate fails on its absence |
| `spike_exemption { pull_request, figure }`, and a release job that refuses an artefact built from a commit with an unretired exemption | The preamble's spike rule |
| `baseline_reset { date, measured_cost, requirement_served, founder_decision }`, landing in its own commit | The preamble's upward-reset rule |
| `display_refresh` in each runner block | **SC-006** measures on a display driven at 60 Hz |

Two defects in the current implementation, both confirmed by reading the script:

- `measure_download_size()` reads `target/release/evreos-shell`, and `run_gates`
keys measurements on `(criterion, name)` with **no platform** — so one Linux ELF
built on a hosted Linux runner is compared against **both** the `windows` and
the `macos` download-size entries. Linux is the deferred platform, and neither
entry's stated condition ("the installer artefact CI publishes") is met by it.
- Every `baseline_mb` is `0.0`, and the regression comparison is guarded on
`baseline > 0`, so the regression half of the four SC-001 entries is inert until
a first real measurement writes a baseline. The commit that first measures must
also set it, or the gate stays inert indefinitely.

### Invariants

1. **The entry, not the criterion, is the unit** — one number, one criterion,
   one
platform, one stated measurement condition. Nine entries per platform, eighteen
in all; the list is closed and adding one is a specification amendment made in
the change that states the figure.
2. **Status describes the figure alone and never whether a gate exists.** Every
entry is gated from M0 by all three gates whatever its status.
3. **A ratified figure may afterwards only be tightened.** Relaxing one requires
   an
amendment recording the founder decision, the measured evidence, and what
discipline replaces the budget it removes. A provisional figure may be replaced
once, by recorded founder decision on spike evidence, and is tighten-only from
that moment.
4. **The pinned runner is one machine per tier of stable identity**, recorded by
model, operating-system version, memory configuration and a durable machine
identifier. A fungible hosted machine is not one. An empty identity fails the
budget-file gate by design — that is what bounds the advisory period on the
absolute gates, and `--allow-unpinned-runners` in the build workflow is a
deliberate, visible suppression of exactly that clause rather than a schema
feature.
5. **A spike exemption lifts one entry's absolute gate and nothing else** —
   never
the regression gate, never the budget-file gate, never another entry — and is
available only to a change that ships no behaviour, where code reachable in a
shipped binary is behaviour whatever flag guards it.
6. **Figures MUST NOT be compared across platforms**: SC-004's two counters are
different quantities (**SC-004**).

### Produced by / consumed by

**Produced by** pull request authors, who state each change's byte and
millisecond cost against this file (**FR-043**), and by recorded founder
decisions, which are the only thing that ratifies a figure or resets a baseline
upward. **Consumed by** `scripts/check-budgets.py` in CI, by the release job
(which must refuse an artefact built from a commit with an unretired spike
exemption), and by the SC-013 publication, which republishes the run record
against the pinned runner's identity.

### What breaks if it changes

Adding an entry adds a gate to every subsequent build and is a specification
amendment. Removing one removes a gate **Principle II** requires and is an
amendment relaxing a principle, which must state what replaces the discipline it
removes. Renaming an entry's `name` or changing its `condition` changes the
entry's identity, so its baseline series restarts — which is also true of
swapping a pinned runner, and is why a spare machine per tier and a written swap
procedure are worth having before a laptop dies **[design]**. Changing the
schema without changing the script fails no gate and is therefore the dangerous
direction: the script iterates over declared entries and does not compare them
against the closed list, so a missing entry is currently silent.

### Change control

Entries, units and the closed list: specification amendment. Statuses and
baselines: recorded founder decision, on the terms above. Gate implementation:
ordinary change — but the gates themselves are defined in the Success Criteria
preamble and only there, so a script that adds a gate of its own has exceeded
this contract.

---

## 6. The Apivo API surface the client depends on

**Forced by**: **FR-021**–**FR-033**, **FR-035**, **FR-040**, **Principle V**
(all money is server-side), **Q-E11a**, **Q-E14**.

This section is a **dependency, not a design**. The Apivo API lives in another
repository and is not changed by this feature. What is stated here is what the
client requests, what it renders, and what the client holds true regardless — so
that a change on the far side is recognised as a break rather than discovered as
a bug. Where the contract's shape is not known from this side, it is an open
question and not an assumption.

### What the client requests

| Request | Forced by | Carries the FR-040 marker? |
| --- | --- | --- |
| Sign in / sign out | **FR-021**, **FR-022**, **FR-023** | Yes — signing in is one of the four acts |
| Wallet snapshot: entries, per-state totals, payable amount, pending reasons | **FR-026**, **FR-026a**, **FR-027** | Yes when the member opens the wallet; **no** on a refresh at launch |
| Merchant catalogue, by language and place as separate parameters | **FR-024**, **FR-035** | No |
| Click-out URL issuance for a specific offer on a specific occasion | **FR-025**, **FR-030** | Yes — following a click-out is one of the four acts |
| Withdrawal request, and its status until a terminal state | **FR-028** | **No.** FR-040 puts the marker on requests that a deliberate member act on an Apivo surface initiates "and on no others", and its acts are a closed enumeration of four — signing in, opening the wallet, redeeming a claim code, following a click-out. A withdrawal request is member-initiated and is none of the four, so it MUST NOT carry the marker and counts towards no retention figure. Whether the enumeration is short by an act is a specification question, not one answered here: **G17** **[gap]** |
| Claim-code redemption | **FR-029** | Yes — redeeming a claim code is one of the four acts. Ships **disabled** pending **Q-E11a** |

### What the client renders

Every amount the service reports, in the state the service reports it — pending,
confirmed, declined and reversed, and payable where the service reports a
payable amount — exactly as reported (**FR-026**). Plus a plain-language
explanation of why an amount is pending (**FR-027**), which requires the wallet
to explain and fixes neither the wording's source nor its shape: whether the
service supplies the string or the client renders a stable code from the
**FR-035** catalogues is open question 3 below and is not decided here. And the
withdrawal's status (**FR-028**).

### Invariants the client holds, whatever the service does

1. **The client never computes a balance** (**Principle V**, **FR-026**). It
   does
not compute, estimate, aggregate or omit any amount whatever its source, and it
presents no amount the service did not report. A client-computed total is
prohibited even when it is arithmetically correct, so **every total the wallet
shows must be a field in the response**.
2. **No state is dropped.** A wallet that shows pending and confirmed but drops
declined and reversed states a larger entitlement than the ledger holds, and the
member believes the wallet (**FR-026**).
3. **The four Principle V invariants stay behind the API** (**FR-026a**): the
   client
does not post, pair or balance ledger entries; does not generate, hold or infer
evidence; does not approve, pre-approve or predict a payout outcome; and does
not deduplicate money actions or treat a retry of its own as having settled one.
4. **A held value is stale, never current** (**FR-026a**): presented with the
   time
it was last received, and replaced outright on reconnection — never reconciled,
merged or diffed, because a client that resolves a disagreement with the ledger
has computed a balance. The design makes this a type property rather than a
discipline: an amount with no arithmetic and no constructor but the API
deserialiser, and a cached value in a distinct stale type with no path back.
**[design]**
5. **A click-out URL is passed to navigation byte for byte** (**FR-025**). The
client does not construct, template or modify an affiliate link or any of its
parameters. The plausible accident is a URL round-tripped through a parser that
normalises it, which is why the design carries a newtype constructible only from
the response and a test asserting byte equality. **[design]**
6. **Attribution follows an explicit member action for that occasion**
(**FR-030**, **FR-018a**, and the Permanent Prohibition on silent affiliate
attribution). No weaker connection carries a claim — not a visit, not a search,
not a session already in progress.
7. **Language and place are two separate values in every request** (**FR-035**,
**Principle VII**), not only in the catalogue. `de-DE` re-fuses the two and does
not satisfy the rule.
8. **The client follows a withdrawal's status and does not decide terminality**
(**FR-028**). FR-028 requires the status followable to a terminal state and says
nothing about who publishes the terminal set; that the service publishes it is
open question 5 below rather than anything FR-028 states. What the requirement
does settle is the negative — a client that decided terminality for itself would
be acting on an authority FR-028 does not give it.

### The two changes Q-E14 records the founder has committed to

**Q-E14**, settled 2026-08-30: *yes* — and the specification records the
dependency as **accepted rather than assumed**, which is why both appear here as
contract terms and not as wishes.

- **A client-type field on member-initiated requests.** **FR-040** requires the
origin marker to be a client-type field carried on requests that a deliberate
member act on an Apivo surface initiates, **and on no others**. The acts are a
closed enumeration: *signing in, opening the wallet, redeeming a claim code, and
following a click-out to a merchant*. A request the client makes without such an
act — a wallet or catalogue refresh at launch, a background token renewal, an
update check, a retry — MUST NOT carry the marker and MUST NOT count towards
retention, "or the criterion measures browser launches rather than members
returning". Adding an act to that enumeration is a specification amendment. The
marker MUST NOT be a device fingerprint (**FR-040**, **FR-036a**). Enforcing
this is a client obligation and is enforceable by construction: two request
builders, one that carries the marker and one structurally unable to.
**[design]**
- **EU-hosted retention computation.** **FR-040** requires signed-in retention
  to
be derived from existing account and wallet activity the service records as
originating from an Evreos client rather than from the diagnostic signal, so
that members who decline diagnostics are still counted in the figure that
matters most, and it records that computation's hosting as a change to a service
outside this repository, "recorded as Q-E14". FR-040 states no hosting rule of
its own, and **FR-039f**'s EU-hosting rule reaches the diagnostic signal, crash
reports and what is derived from them — which this figure, drawn from account
activity, is not. EU hosting is a term here because **Q-E14** commits the
founder to it, on that footing and no other. **SC-011**'s signed-in figure rests
on the computation existing.

Both are changes to a service outside this repository. If either does not land,
**SC-011**'s signed-in figure has no mechanism — the specification is explicit
that without the marker, "a member who uninstalls Evreos and keeps using the
existing web wallet produces the same account activity as a retained member".

### Produced by / consumed by

**Produced by** the Apivo service. **Consumed by** the shell's money surfaces —
wallet, claim and the offer control — which ship in the release rather than as
delivered app surfaces. Only the wallet is placed there by requirement:
**FR-031** requires it delivered as part of the shell, never installed, enabled,
updated or removed through any extension mechanism, and present and usable in a
build with no extension host and no extension installed, which is also why the
invariants above are checkable in the reviewed build and inside the budget
gates. **FR-018b** is what puts the cashback offer surface in the browser's own
chrome and never in the page. Nothing requires the claim surface to be
shell-native; that it is, rather than a delivered surface under contract 3, is
**[design]**.

### What breaks if it changes, and what is not yet known

The open questions below are the contract's unresolved terms. Each is settled by
reading the API's contract or by a founder decision with the service owner, and
none is settled by choosing here.

1. **Does the service report per-state totals and a payable amount, or only
entries?** If only entries, **FR-026**'s ban on client aggregation makes the
wallet unbuildable as specified, and either the API gains total fields or FR-026
is amended. This is the single largest external dependency in the money model.
2. **Can a wallet hold amounts in more than one currency?** If so, no single
   total
exists that the client could show even were it permitted to compute one, and
per-currency service-supplied totals become mandatory.
3. **Is the FR-027 pending-reason set a closed enumeration with stable codes,
   and
is its text keyed by primary language subtag with place separate?** A closed
code set lets the explanation ship in the **FR-035** catalogues and stay
available in the offline and stale states; a free-form server string does not,
and would also need checking against **FR-039**'s content bans if it were ever
logged.
4. **Does the service issue a withdrawal token before submission?** If it does,
exactly-once stays wholly server-side and the client never needs an idempotency
key of its own, which is where **FR-026a** puts it. If it does not, the plan
must record which reading of FR-026a it takes and why.
5. **Does the service publish the terminal status set?** Without it the client
would be deciding terminality, which **FR-028** does not authorise.
6. **Does the service render a display string for a (language, place) pair
   beside
the structured amount?** **FR-026** forbids the client altering a reported
amount; formatting is presentation rather than computation, but it puts rounding
and symbol placement in the client while **FR-035** requires the two axes kept
separate. Which axis drives number, date and currency formatting is a founder
decision. **[gap]**
7. **Q-E11a** — the existing service is not yet confirmed to hold campaign
   records
and accept a redemption. **FR-029** ships present and disabled by a **build
constant** rather than a fetched flag, because **FR-029a**'s observer test is
stated over "a build with no backing service"; activating it makes no request to
any service. **SC-010** is not measurable until it is confirmed, so it must not
be scheduled as an acceptance gate.

---

## Gaps

Everything below is something the design needs that the specification does not
require, or something a requirement leaves for a founder decision. None of it is
smuggled in above as though it were required.

| # | Gap | Contract | Settled by |
| --- | --- | --- | --- |
| G1 | `Intercepted` is derivable from no platform error code on either shipping tier. Keeping it in the closed enum and forbidding a backend from synthesising it is the design's answer; whether the shell may make an outbound probe to classify it is a founder decision under **Principle VI** and **FR-007a**, and whether the variant survives is a **FR-015** question | 1 | Driving each tier's backend through a real captive portal on that tier's pinned runner and recording the full signal tuple; then a founder decision |
| G2 | `Page::address` reporting the address that loaded rather than the one requested is a design decision, not a requirement | 1 | Nothing needs settling; recorded so it is not cited as a requirement |
| G3 | The capability ceiling in the app registry is stronger than **FR-017**, which forbids widening *from inside* and does not bound what a signed manifest may declare | 2 | A plan decision, recorded with its cost |
| G4 | `supersedes` in the root-signed delegation binds the publisher and the delivery host but not the holder of the root key; the remaining guarantee against that party is procedural | 2 | Stated, not closed. **FR-016a** cannot reach further |
| G5 | `not_after` on the surface signature, and the version floor held outside the FR-020 cache, are both stronger than **FR-019a** | 3 | Plan decisions, each with its residual stated |
| G6 | Where the root signing key lives, who holds it, and the recorded procedure for signing a delegation | 2, 3 | A founder decision recorded as an ADR. The two-level key design is unimplementable without it |
| G7 | Whether **SC-014**'s "every URL-bearing payload" is read literally — in which case a conforming build fails on its own **FR-014** update check and on an app-surface fetch — or as history-bearing, which matches **FR-007a**'s enumeration but not its conformance clause, which fails on any outbound request no entry accounts for | 3, 4 | A founder decision landing as a specification amendment. Not an implementer's call: it changes what the criterion and FR-007a's conformance test both mean |
| G8 | `os_version` granularity in the crash report, and the two-family reason enumeration | 4 | A founder reading committed beside the enumeration file |
| G9 | Whether **FR-039c**'s "MUST carry only the module name, the symbol name, and the source file and line" is a ceiling or a floor — which decides whether line tables ship, a measurable cost against **SC-001** | 4 | A founder reading, taken with the measured byte cost in hand |
| G10 | Fixed-size padding and one-report-per-connection are design additions **FR-039b** permits but does not require | 4 | Plan decisions, with the payload size stated under **FR-043** |
| G11 | Which relay operator, in which jurisdiction, will contract to **FR-039b**'s three obligations; and whether a US-incorporated operator running EU-only ingress satisfies FR-039b's naming rule and **FR-039f**'s hosting rule | 4 | A signed contract with a named operator, plus a recorded opinion from counsel. Release-gating procurement, not an engineering task |
| G12 | The lawful basis for transmitting the withdrawal report after the member turns diagnostics off. Disclosure under **FR-039** is not the same as a basis | 4 | A recorded opinion from counsel alongside the DPIA |
| G13 | `budgets.toml` carried four of eighteen entries and no `unit`, `founder_decision`, `cross_check_margin`, `spike_exemption`, wake enumeration or display refresh | 5 | **Settled.** Phase 1 setup lands all eighteen entries and every field. What remains is not schema and no change here can supply it: both runner identities stay empty until the machines are procured, and every baseline is zero, which the regression gate reads as no baseline yet. The commit that first measures also sets the baseline |
| G14 | `check-budgets.py` compared one Linux binary against both the Windows and the macOS download-size entries | 5 | **Settled.** Keyed on `(criterion, name, platform)` in Phase 1 setup, measuring the artefact each entry's condition names and measuring nothing on a host of no tier, which Linux is |
| G15 | Which axis — language or place — drives number, date and currency formatting, and whether the service supplies a rendered display string | 6 | A founder decision against **FR-035** and **FR-026**, plus the API contract |
| G16 | Six unresolved terms of the Apivo contract (§6, items 1–6) | 6 | Reading the API's contract, or a founder decision with the service owner |
| G17 | A withdrawal request under **FR-028** is a deliberate member act on an Apivo surface that **FR-040**'s closed enumeration of four acts does not reach, so it carries no origin marker and counts towards neither **SC-011**'s signed-in figure nor **SC-012**'s denominator. Whether that omission is intended or the enumeration is short by an act is not settled here | 6 | A specification amendment adding the act — FR-040 makes adding one an amendment — or a recorded decision that the omission is deliberate |

## Change-control summary

| Change | Class |
| --- | --- |
| A variant of `LoadError`, or what a cause means | Specification amendment (**FR-015**, **SC-009**) |
| A method on `Engine` | Ordinary change, both implementations in one commit (**FR-044**), cost stated (**FR-043**) |
| A capability name in the shipped catalogue, or an app's ceiling | Browser release (**FR-018**, **[design]** for the ceiling) |
| An app identity | A new app, inheriting its predecessor's dismissals (**FR-016a**) |
| The pinned trust root | Browser release only (**FR-019a**) |
| A field in any diagnostic payload | Specification amendment (**FR-039a**, **FR-039c**) |
| A diagnostic counter key | Specification amendment (**FR-039d**) |
| A crash reason code, or the `os_version` granularity | A change to the committed enumeration file (**FR-039c**) |
| The receiving service's public key | Release (**FR-039b**) |
| A budget entry | Specification amendment, in the change that states the figure |
| A budget status or an upward baseline reset | Recorded founder decision, in its own commit |
| An act in **FR-040**'s member-initiated enumeration | Specification amendment |
| An entry in **FR-007a**'s closed transmission list | Specification amendment, made in the pull request that would add the transmission and checked against the Permanent Prohibition there |
