# Data Model: Evreos v1

**Feature**: `001-evreos-v1` · **Phase**: 1 (design) · **Source of truth**:
`specs/001-evreos-v1/spec.md`, `.specify/memory/constitution.md`,
`docs/adr/0001-rendering-engine.md`

This document is the entity model for Evreos v1. It is extracted from the spec's
Key Entities section and from every requirement that implies state. It fixes the
shape of each entity, what it may hold, what it may never hold, where it lives,
and how it moves between states.

It is a design artefact, not a schema. Field names here are the names the design
uses to argue about the thing; the wire and on-disk encodings are settled in the
contracts and in the pull requests that land them, each stating its byte and
millisecond cost under **FR-043**.

## How to read this document

Every entity carries five fixed parts.

- **Forced by** — the requirements that determine the entity's shape. Where a
field, a constraint or a transition exists because a requirement says so, the
requirement is named at that field. Where it exists because the design chose it,
it is marked **[design]**, and where the design needs something the spec does
not require it is marked **[gap]** and repeated in *Gaps and open questions* at
the end. Nothing in this document is presented as required unless a requirement
is named for it.
- **Fields** — with the type in the sense of "what kind of value", not in the
sense of a Rust declaration.
- **Relationships** — what it points at and what points at it.
- **Validation** — the rules a build can be held to.
- **State** — transitions, where the entity has them.
- **Residence** — one of the five classes below. This is the load-bearing part
of the model, and it is why residence appears on every entity rather than only
on the ones where it is interesting.

### Residence classes

| Class | Meaning | Consequence |
| --- | --- | --- |
| **L — local-only** | The value lives on the member's machine and never leaves it, in any form, derived or not. | No serialiser on any egress path. Named individually in *Invariant A*. |
| **LR — local cache of remote state** | A copy of something a service reported. | Presented as stale with the time it was received (**FR-026a** for money); replaced outright on refresh, never merged. |
| **R — remote-owned** | The service holds it; Evreos renders it and requests changes to it. | The client never computes, infers or completes it (**Principle V**, **FR-026**, **FR-026a**). |
| **B — build constant** | Shipped inside the release, changed only by a release. | Cannot be fetched, replaced or extended at runtime. Includes the trust root (**FR-019a** pins it in the shipped shell), the crash-reason enumeration (**FR-039c** commits it to this repository), the receiving service's public key (**FR-039b**: pinned in the build, rotated only by a release) and the brand configuration (**FR-042**). Two are this design's placement rather than a requirement's, and are marked as such where they are defined: the capability catalogue (**[design]**, §3.3 — FR-018 requires it published with the manifest format and requires an unclassified capability never granted, and does not say where it ships) and the interface catalogues (**[design]**, §2.1 — FR-035 requires the three languages and the key, not a build constant). |
| **T — transient** | Exists for one occasion and is never persisted. | Persisting it would defeat the requirement that created it — the FR-018a occasion is the case to watch. |

### Vocabulary fixed once here

- **Profile** — the member data one installation of Evreos holds for one
operating-system user. Named in FR-007 ("no browsing trace"), FR-023 ("no Evreos
profile file … may hold the credential") and SC-001 ("member data the first run
creates — profile, cache and downloads — is excluded").
- **Site** — the unit **FR-006** prompts per and **FR-008** exempts per. Its
exact key is not fixed by the spec; see *Gaps*.
- **Language** and **Place** — two separate values, everywhere either appears
(**FR-035**, **Principle VII**). No entity in this model carries a fused value,
and none may be added.
- **Amount** — a money value the service reported. It is a distinct type with
no arithmetic (*Invariant B*).
- **Occasion** — one page load, in the sense **FR-018a** fixes: a later
navigation, a reload and a restored tab are each a new occasion, and a
navigation is any change to the address including one the page performs without
fetching a new document.

---

## Invariant A — browsing history is local-only, by construction

**Forced by**: FR-007a, FR-004, FR-003, Principle VI, and the Permanent
Prohibition on server-side collection of browsing history.

FR-007a defines browsing history as the record of where the member has been
*together with any store or transmission from which that record could be
reconstructed, and any value derived from that record* — a classification, a
cohort, a score, a count, a summary, an embedding, a hash, or any other function
of what the member visited, whether or not the record can be reconstructed from
the value.

That definition is wider than a table of visited URLs, and it decides the
residence of far more entities than the obvious one. **Every entity below is
class L. None of them has a representation on any outbound path.**

| Entity | Why it is browsing history under FR-007a |
| --- | --- |
| `HistoryEntry` | The record itself. |
| `SuggestionIndex` | A function of the history and bookmark stores (FR-003, FR-007a's "suggestions … produced only from data already on the machine"). |
| `Bookmark`, `BookmarkFolder` | Addresses the member chose to keep; FR-004 names all three stores as local and FR-007a's enumeration carries none of them. |
| `Download` | Carries the source address of each downloaded file. |
| `Tab`, `Window`, `Session` | The set of addresses currently open, and the order they were opened in. |
| `SitePermission` | A per-site record; the set of sites it names is a partial record of where the member has been. |
| `SiteBlockingException` | Same; FR-008's per-site control produces one row per site the member visited and found broken. |
| `Grant` | Per-app, but a page-adjacent grant's exercise is scoped to sites; the grant set itself must not be transmitted. |
| `ImportJob` | Reads a third-party profile's history and bookmarks; the result is history in the same sense. |
| `PdfViewState`, `FindInPageState` | Derived from the current page. |

**What "structurally true" means here, in this design.**

1. **One egress chokepoint with a closed purpose.** FR-007a's enumeration is
exhaustive: page load, certificate status, submitted search, hand-off. The
design routes every request the *shell* originates through a single crate whose
request constructor requires a `Purpose` drawn from a closed enumeration of
exactly those four plus a separately listed set of non-history purposes (the
FR-014 update check, the FR-039b diagnostic channel, the Apivo API calls
FR-021/FR-024/FR-025/FR-026/FR-028 require). Adding a variant is a one-file
diff, which is what FR-007a means by "an amendment … made in the pull request
that would add the transmission". **[design]** — FR-007a requires the
enumeration and the conformance test; it does not name a chokepoint.
2. **No serialiser reaches a class-L type.** None of the entities above has a
serialisation used by the egress crate. Their only serialisations are the local
store formats. This is the mechanical half of the rule: a future field that
tried to carry a visit count off the machine would have to add a serialiser to a
type that has none, in a crate that cannot depend on a socket.
3. **Engine-originated traffic is not reachable by construction, and is
enforced by capture.** The system web runtime opens its own sockets. FR-007a
makes those Evreos's transmissions, and FR-007a's paragraph on the system web
runtime requires any runtime feature that sends visited addresses on its own to
be turned off — "a reputation or address-filtering service is the case to
expect". That is not FR-007a's final paragraph; the final one is the
conformance-capture clause, which is a separate obligation. The instrument is
**SC-014**'s published capture, run on the exact commit a release artefact is
built from — not documentation and not review.
4. **Deletion cascades.** FR-004 requires a deletion to erase the record "from
the store and from every index derived from it, not merely from the list
presented to the member", and requires it not to reappear. The model therefore
admits no append-only history log and no derived index that is rebuilt from
anything other than the live store. Every derived structure names its source
store and its deletion behaviour in this document.
5. **Private windows leave no residue.** FR-007 requires a private window to
leave no browsing trace after it closes. In this model that means: its tabs are
never written to `Session`; its navigations produce no `HistoryEntry`; its
`SitePermission` and `SiteBlockingException` decisions are held for the window's
lifetime only; and its engine-side data store is a distinct non-persistent one.

**What FR-007a does *not* bind.** Local computation. The FR-003 suggestion
index, the FR-004 history view and the FR-012 import are that computation and
are permitted. A derived value becomes governed the moment it is transmitted or
retained off the machine.

---

## Invariant B — a wallet amount is rendered, never produced

**Forced by**: FR-026, FR-026a, FR-027, Principle V, and the spec's Edge Case
"the ledger and the client disagree".

FR-026 requires the wallet to present every amount the service reports, in the
state the service reports it, **exactly as reported**, and forbids presenting
any amount the service did not report or computing, estimating, aggregating or
omitting any amount whatever its source. FR-026a forbids the client
re-implementing, approximating or caching-as-truth double entry, evidence,
approver-gated payouts and exactly-once, and requires a held value to be
presented as stale with the time it was last received.

The model makes this structural in four ways.

1. **`Amount` has no arithmetic.** It is constructed only by the API
deserialiser. It implements no addition, no summation, no multiplication, no
rounding and no currency conversion. A total the wallet displays is a field the
service sent (`StateTotal`, `PayableAmount`), never a fold over `WalletEntry`. A
compile-fail test asserting that `a + b` and `iter.sum()` do not typecheck over
`Amount` is the artefact that makes Principle V's central prohibition checkable
rather than reviewed. **[design]** — FR-026 requires the prohibition; the type
discipline is how this design enforces it.
2. **A cached value is a different type.** `Stale<T> { value, received_at }` has
no path back to a bare `Amount`. Every render site therefore has to decide, in
code the compiler checks, whether it is drawing a live figure or a stale one —
and FR-026a requires the stale one to be labelled with the time it was received
and never presented as a current balance.
3. **Refresh replaces; it never reconciles.** `LedgerSnapshot` is replaced
wholesale on a successful read. There is no merge, no diff and no per-entry
reconciliation, because FR-026a says a client that resolves a disagreement with
the ledger has computed a balance.
4. **Omission is representable as a failure, not as an empty screen.** All four
states — pending, confirmed, declined, reversed — are present in
`LedgerSnapshot` whether or not each holds entries, so that a rendering which
drops declined and reversed is a missing branch rather than a plausible layout.
FR-026 names exactly that failure: "a wallet that shows pending and confirmed
but drops declined and reversed states a larger entitlement than the ledger
holds, and the member believes the wallet."

**Consequence the plan must carry.** If the Apivo API reports only individual
entries and no per-state total, no compliant wallet can display a total at all,
because computing one is what FR-026 forbids. That is a spec amendment or an API
change, not an implementation choice. See *Gaps*.

---

## 1. Browsing

### 1.1 Profile

**Forced by**: FR-004 (three stores survive close and reopen), FR-007 (private
windows), FR-023 (nothing credential-bearing here), SC-001 (profile, cache and
downloads are member data and excluded from the installed footprint).

| Field | Type | Notes |
| --- | --- | --- |
| `profile_id` | opaque local id | Local only. Never transmitted; **FR-036a** forbids deriving it from device characteristics, and no requirement asks for it off-machine. |
| `root_path` | path | Per operating-system user. |
| `language` | `Language` ∈ {`de`, `el`, `en`} | Primary subtag only (**FR-035**). |
| `place` | `Place` | A separate value; never fused into `language` (**FR-035**, **Principle VII**). |
| `theme_preference` | `System \| Light \| Dark` | Default `System` (**FR-010**). |
| `ui_scale` | percentage, 100–200 | **FR-005**; the chrome-layout half. |
| `default_search_provider` | `SearchProviderSetting` | **FR-003a**. |
| `hand_off_browser` | `HandOffBrowser` | **Key Entities**; FR-015a, FR-037. |
| `diagnostics_enabled` | bool, default `false` | **FR-039**; off until the member turns it on. |
| `crash_reporting_enabled` | bool, default `false` | **FR-039c**; separately consented. |
| `home_surface_hidden` | bool, default `false` | **FR-016**, **FR-016a**. |

**Stores held under the profile** (each its own section below): `HistoryStore`,
`BookmarkStore`, `DownloadStore`, `SessionStore`, `SitePermissionStore`,
`SiteBlockingExceptionStore`, `GrantStore`, `DismissalStore`, `SurfaceCache`.

**Validation**

- No store under the profile may hold an account credential, a token derived
from one, or any value from which either can be reconstructed (**FR-023**). The
observer test FR-023 states — sign in, search the profile directory and the
logs, find no match — is a test over this directory.
- The diagnostic enrolment state is **not** held here. See §5.1 and the gap it
raises.

**Residence**: L.

### 1.2 Window

**Forced by**: FR-001, FR-007, FR-016a.

| Field | Type | Notes |
| --- | --- | --- |
| `window_id` | local id | |
| `kind` | `Normal \| Private` | **FR-007**. |
| `tabs` | ordered list of `Tab` | Order is the member's, and is restorable (**FR-001**). |
| `active_tab` | `Tab` reference | Exactly one when the window has tabs. |
| `data_store` | `DataStoreSelector` | `Persistent` for `Normal`, a distinct `NonPersistent` store for `Private`. **[design]**: FR-007 requires the outcome ("no browsing trace"); the mechanism is the engine seam's, and `research.md` §1.5 (addressable rendering-surface handle plus a data-store selector) records that the merged `Engine` trait carries no selector for it — the trait in `crates/evreos-engine/src/lib.rs` declares `name`, `load` and `current` and nothing else. |

**Validation**

- A `Private` window's tabs MUST NOT appear in `SessionStore` (**FR-007**).
- A `Private` window's navigations MUST NOT produce a `HistoryEntry`
(**FR-007**).
- Closing the last `Private` window destroys its data store, its transient
permissions and its transient blocking exceptions (**FR-007**).

**State**

```
Opening ──▶ Open ──▶ Closing ──▶ Closed
                       │
                       └─ Private only: data store destroyed before Closed
```

**Residence**: L.

### 1.3 Tab

**Forced by**: FR-001 (open, close, reorder, restore), FR-002 (suspension with a
stated policy, reversible without losing visible page state), FR-015 (navigation
failure is distinguishable), FR-018a (occasion), SC-006 (tab switch within 16
ms).

| Field | Type | Notes |
| --- | --- | --- |
| `tab_id` | local id | Stable across suspend and restore, so a dismissal, a grant or an occasion can be keyed to it. |
| `window_id` | reference | |
| `position` | index | **FR-001** reorder. |
| `displayed_address` | address string | The address that actually loaded, not the one requested — showing the request while displaying the response is how an address bar lies (the seam records this in `Page::address`). |
| `title` | string | Arrives on its own engine event, not with the load outcome. `research.md` §1.2 establishes this on tier 1: wry registers `add_DocumentTitleChanged` and `add_NavigationCompleted` as separate handlers with no ordering between them (wry 0.56.1, `src/webview2/mod.rs:688`, `:724`), so one returned `Page{address,title}` conflates two events and will routinely carry a stale or empty title. The tier-2 ordering is **not** established there and is not claimed here; the model's shape does not depend on it. |
| `navigation_epoch` | monotonic counter | Increments on **every** change to the displayed address, including one the page performs without fetching a new document. **FR-018a** forces this: "A navigation is any change to the address the member is on, including one the page performs without fetching a new document." |
| `lifecycle` | `Loading \| Live \| Suspended \| RestoredNotLoaded \| Failed(LoadError)` | See *State*. |
| `load_outcome` | `NavigationOutcome` | §1.4. |
| `find_state` | `FindInPageState` (transient) | **FR-005**. |
| `page_zoom` | percentage | **FR-005**; distinct from `Profile.ui_scale` and from the engine's rasterisation scale. |

**Relationships**: belongs to one `Window`; points at zero or one `AppSurface`
where the tab hosts an app rather than a page (§3.4); is the subject of zero or
one `OfferActivation` at a time (§4.6).

**Validation**

- `lifecycle = Failed` and a rendered page are mutually exclusive: a failed load
MUST NOT present as a successful empty page (**FR-015**).
- A tab that has been `Loading` for 30 s without resolving is a defect the
gate catches, not a state the model normalises — **SC-009** admits zero loading
indicators that do not resolve within 30 s. The model therefore carries a
`loading_started_at` so the shell can time out; the timeout policy is the
shell's, not the engine's, and no `LoadError` variant represents it.
- `RestoredNotLoaded` is a first-class state, not a `Loading` that has not
begun: **FR-001** requires the session restored, and restoring ten live pages
spends SC-002's whole start-up budget and enters SC-004's ten-tab ceiling at its
worst point. **[design]** — FR-001 requires restoration of tabs; it does not
require them restored suspended.

**State**

```
                    ┌──────────────── activate ─────────────────┐
                    ▼                                            │
RestoredNotLoaded ──▶ Loading ──▶ Live ──▶ Suspended ────────────┘
                          │         ▲          │
                          │         └── resume ┘   (FR-002: reversible without
                          │                         losing visible page state)
                          └──▶ Failed(LoadError)   (FR-015)
```

- `Live → Suspended` follows the stated policy **FR-002** requires. The policy
is written per tier, because the mechanism differs and on tier 2 at the macOS 13
floor no verified lever was located: `research.md` §2.5 records
`WKPreferences.inactiveSchedulingPolicy` as macOS 14.0+ and unexposed by wry,
and wry's own `with_background_throttling` as documented Unsupported on Windows
and Supported only since macOS 14.0 (`src/lib.rs:1488-1493`) — so neither tier
gets that lever at its floor, and the tier-2 mechanism stays unestablished and
is carried as measurement N5. A policy stated from an API that does not exist at
the floor would be a claim about behaviour nobody measured; the plan carries the
measurement.
- Every transition out of `Live` increments nothing; only an address change
increments `navigation_epoch`.

**Residence**: L.

### 1.4 NavigationOutcome

**Forced by**: FR-015, SC-009, and the merged `Engine` seam
(`crates/evreos-engine/src/lib.rs`; `feat/engine-seam` is merged to `main`).

The seam's `LoadError` is a closed four-variant enum matching FR-015's four
causes. This model keeps it unchanged and wraps it.

| Field | Type | Notes |
| --- | --- | --- |
| `navigation_id` | id | Correlates a request with its outcome, so an outcome for a navigation the member abandoned is distinguishable from the current one. **[design]** — `research.md` §1.2 establishes that the merged synchronous `load` (`fn load(&mut self, request: &Request) -> Result<Page, LoadError>`) carries no request-to-outcome correlation, so an outcome for a navigation the member abandoned cannot be told from the current one; the reshaping is the plan's, not this document's. |
| `requested` | address | What the shell asked for. |
| `outcome` | `Succeeded { address, title } \| Failed(LoadError)` | |
| `LoadError` | `Unresolvable \| Certificate { detail } \| Intercepted \| AuthenticationRequired` | The four causes **FR-015** names, exercised on every supported platform by **SC-009**. |

**Validation**

- `Intercepted` is never synthesised by a platform backend from an error code.
`research.md` §1.3 establishes, from an exhaustive reading of
`COREWEBVIEW2_WEB_ERROR_STATUS`'s nineteen values (webview2-com-sys 0.38.2,
`src/bindings.rs:886-923`) and of `NSURLError` (objc2-foundation 0.3.2,
`NSURLError.rs:113-165`), that no error code on either shipping tier denotes
interception, and that a captive portal answers, so the navigation succeeds.
Classifying it is a shell-level inference, and any probe-based inference is an
outbound request **FR-007a** governs and **Principle VI** constrains — a founder
decision, not a backend detail. The variant stays in the enum because **FR-015**
names four causes and **SC-009** requires four exercised; the headless engine
scripts it so the fourth case stays testable. **[gap]**
- No timeout variant is added. A stalled load is an absence of an event, not a
cause, and modelling it as an error would move a timeout policy FR-015 and
SC-009 place with the shell into the engine.

**Residence**: T (the outcome), L (its effect on `Tab` and `HistoryEntry`).

### 1.5 Session

**Forced by**: FR-001 (session restored after close and reopen), FR-007 (private
windows leave no trace), SC-002 (start-up budgets), SC-004 (ten-tab memory), Key
Entities ("the set of open tabs and their state, restored across restarts").

| Field | Type | Notes |
| --- | --- | --- |
| `saved_at` | timestamp | |
| `windows` | list of `SessionWindow` | `Normal` windows only. |
| `SessionWindow.tabs` | ordered list of `SessionTab` | |
| `SessionTab` | `{ address, title, position, active }` | Enough to restore identity, order and address. |

**Validation**

- A `Private` window MUST NOT appear in this store, in any form, at any point
in its lifetime (**FR-007**). This is a write-path exclusion, not a filter on
read: a session file that contained a private tab and hid it on load would have
left the trace FR-007 forbids.
- Restoration reproduces tab identity, order and address; page load happens on
first activation (`RestoredNotLoaded` in §1.3). **[design]**
- The write is atomic (temp file plus rename) so a crash cannot truncate the
session into partial loss. **[gap]** — no requirement states this; FR-001's
"restored" is the outcome the design is protecting.

**Residence**: L.

### 1.6 HistoryEntry

**Forced by**: FR-004 (review, search, delete single entries and a chosen time
range; deletion erases from the store and every derived index and does not
reappear), FR-003 (suggestions), FR-007 (private windows), FR-007a (the whole of
Invariant A), FR-012 (import).

| Field | Type | Notes |
| --- | --- | --- |
| `entry_id` | local id | |
| `address` | address | |
| `title` | string | |
| `visited_at` | timestamp | Ordering is part of the record FR-007a defines. |
| `source` | `Navigated \| Imported { browser }` | **FR-012**. |

**Relationships**: feeds `SuggestionIndex` (§1.7); referenced by nothing that
leaves the machine.

**Validation**

- Never transmitted, never retained off the machine, in whole or in derived
form — no count, no frequency, no dwell time, no cohort, no embedding, no hash
(**FR-007a**, naming each of these as a plausible addition and forbidding it).
- Never synchronised (**FR-007a** names "no synchronisation of history or
bookmarks"; **Non-Goals** names synchronisation across devices).
- Deletion of an entry, or of a time range, removes it from this store and from
`SuggestionIndex` in the same operation, and it MUST NOT reappear (**FR-004**).
The model therefore holds no second copy — no undo log, no append-only journal,
no search index with an independent lifetime.
- A `Private` window produces none (**FR-007**).

**Residence**: L.

### 1.7 SuggestionIndex

**Forced by**: FR-003 (one field combining search, history and bookmarks),
FR-007a ("suggestions … produced only from data already on the machine: the
member's history, bookmarks and open tabs. The field therefore transmits nothing
as the member types"), SC-006 (address-field keystroke within 16 ms, no trial
over 16 ms).

| Field | Type | Notes |
| --- | --- | --- |
| `sources` | `{ HistoryStore, BookmarkStore, open tabs }` | Exactly these three. **FR-007a** closes the list. |
| `entries` | derived rows | Rebuilt from the live stores; holds no record its sources have deleted. |

**Validation**

- No network component of any kind. There is no suggestion service to be
consented to (**FR-007a**), and the FR-003 field transmits nothing before the
member submits (**FR-003a**).
- Deletion in a source store propagates here in the same operation (**FR-004**).
- Lookups against this index run off the UI thread, because SC-006 admits no
trial over 16 ms and the index grows with the profile. **[design]**
- The index's size and shape is a stated measurement condition on SC-006's
address-field entry. **[gap]** — the budget file records no profile condition on
any entry, and a figure measured on a fresh profile and one measured on a
year-old profile are different quantities under the same entry, which SC-013's
third party cannot reproduce.

**Residence**: L.

### 1.8 Bookmark and BookmarkFolder

**Forced by**: FR-004 (create, rename, organise into folders, delete; survives
restart), FR-003 (a suggestion source), FR-007a, FR-012.

| Field | Type | Notes |
| --- | --- | --- |
| `bookmark_id` | local id | |
| `parent_folder` | folder reference | Root folder is implicit. |
| `title` | string | Member-editable (**FR-004** rename). |
| `address` | address | |
| `created_at` | timestamp | |
| `source` | `Created \| Imported { browser }` | **FR-012**. |

`BookmarkFolder`: `{ folder_id, parent_folder, name, position }`.

**Validation**

- Folder graph is a tree: no cycles, one root, every bookmark reachable.
- Never transmitted or synchronised (**FR-007a**).
- Deletion is a deletion, including from `SuggestionIndex` (**FR-004**).

**Residence**: L.

### 1.9 Download

**Forced by**: FR-004 (in progress and completed, each with its destination on
disk; cancel one in progress; remove one from the list; survives restart),
FR-007, FR-007a, SC-001 (downloads are member data, excluded from the installed
footprint).

| Field | Type | Notes |
| --- | --- | --- |
| `download_id` | local id | |
| `source_address` | address | Browsing history under **FR-007a**. |
| `destination_path` | path | **FR-004** requires the destination shown. |
| `bytes_total` | integer or unknown | |
| `bytes_received` | integer | |
| `state` | `InProgress \| Completed \| Cancelled \| Failed` | |
| `started_at`, `finished_at` | timestamps | |

**Validation**

- Removing a download from the list removes the record; it does not delete the
file the member saved. **FR-004** says "remove one from the list", and the file
on disk is the member's, not Evreos's.
- Never transmitted (**FR-007a**).

**State**

```
InProgress ──▶ Completed
     │
     ├──▶ Cancelled   (member action, FR-004)
     └──▶ Failed
```

**Gap**: FR-007 requires a private window to leave no browsing trace, and FR-004
requires the download list to survive restart. The spec does not settle whether
a download started in a private window leaves a record after that window closes.
This design takes the reading that the **record** does not persist while the
**file the member chose to save** does, on the ground that a file the member
directed Evreos to write is not a browsing trace. That reading needs a founder
confirmation; it is not derivable from the text.

**Residence**: L.

### 1.10 SitePermission

**Forced by**: FR-006 (prompt per site for camera, microphone, location and
notification access, and allow those decisions to be revisited), FR-037 (say so
and offer a hand-off where a capability proves unavailable), FR-041 (state
limitations before download), FR-007a.

| Field | Type | Notes |
| --- | --- | --- |
| `site` | site key | See *Gaps* — the spec says "per site" and does not fix the key. |
| `capability` | `Camera \| Microphone \| Location \| Notification` | Closed, from **FR-006**. |
| `decision` | `Ask \| Granted \| Denied \| UnavailableOnThisPlatform` | The fourth is not a member decision; see below. |
| `decided_at` | timestamp | |
| `window_scope` | `Persistent \| PrivateWindow(window_id)` | A private window's decisions die with it (**FR-007**). |

**Validation**

- `Ask` is the default for every (site, capability) pair; a permission is never
pre-granted.
- Every decision is revisitable (**FR-006**), which means this store is
member-editable and a revocation takes effect without reinstalling anything.
- `UnavailableOnThisPlatform` is distinct from `Denied` because they are
different facts and the member is owed different words. `research.md` §2.4 reads
WebKit's public `WKUIDelegate.h` and records that media-capture permission is
declared `WK_API_AVAILABLE(macos(12.0))` — reachable at the macOS 13 floor, so
camera and microphone prompts exist — while geolocation permission is declared
`WK_API_AVAILABLE(macos(27.0))` and no notification-permission delegate is
declared at all. That entry is marked *established for API availability;
unestablished for what a page observes* (measurement N9), and this document
carries it no further: what a page actually observes at that floor is
**unmeasured**, and nothing here states a presence or an absence. **A finding to
route, not a conclusion of this document.** Where the architecture cannot
deliver a requirement, this project records it in ADR-0001's capability floor —
and that floor records nothing of the kind. Its seven bullets are content
protection, the absence of a cross-platform content-blocking primitive,
third-party Chrome extensions, passkeys, engine security fixes on macOS and
Linux, per-tab process isolation, and WebView2 being Chromium; no permission API
appears. If §2.4's header reading holds, it belongs in that record as an
amendment. That amendment is out of scope here and is carried in *Gaps*. One
citation must also be dropped rather than repaired: **FR-041**'s "MUST NOT
assert either presence or absence" is scoped to the site-credential autofill
test FR-015a requires and does not reach FR-006's four capabilities, so it is
not the bar here. **[gap]** — no requirement forbids the distribution page
speaking about permission availability before it is measured; this design
proposes it stay silent regardless. Where the capability is genuinely
unavailable, **FR-037** requires the browser to say so and offer a hand-off
rather than fail silently, and the model needs the state to say it.
- The store is never transmitted (**FR-007a**): the set of sites it names is a
partial record of where the member has been.

**Residence**: L.

### 1.11 BlockingRuleSet and its compiled artefacts

**Forced by**: FR-008 (active on first launch without configuration; a visible
per-site control; collapsing the space a blocked element leaves), FR-018a
(collapsing is not injection, and content blocking is named as meeting the
exemption test), SC-001 (the corpus and the compiled artefact land inside the
download and installed-footprint entries), SC-005 (the blocking-list refresh is
one of the three enumerated wakes), ADR-0001 capability floor (WebKit's compiled
rule lists cap at 150,000 rules each, so multi-list splitting is required from
day one), Q-E12.

| Field | Type | Notes |
| --- | --- | --- |
| `list_id` | id | e.g. EasyList, EasyPrivacy, EasyList Germany. |
| `source_revision` | pinned revision | Reproducible under **SC-013**. |
| `fetched_at` | timestamp | |
| `rule_count_source` | integer | Source filter rules. |
| `partitions` | list of `CompiledPartition` | Tier 2 only; tier 1 consults a native matcher in-process. |
| `conversion_failures` | map of failure kind → count | The tier-1/tier-2 capability delta expressed as a number rather than an impression. **[design]** |

`CompiledPartition`: `{ partition_id, emitted_rule_count, exception_rules,
block_rules, artefact_digest }`.

**Validation**

- `emitted_rule_count` ≤ 150,000 **per compiled list**, counted on the emitted
rule objects and not on source filter rules. The conversion is not one-to-one,
so a source count under the ceiling does not imply a list that compiles, and the
failure is total — the whole list fails, so it cannot be discovered on a
member's machine. ADR-0001's capability floor states the 150,000 figure;
`research.md` §3.3 refines the unit it is counted in and establishes that
refinement against WebKit's own parser (`ContentExtensionParser.cpp:333-334`,
where `maxRuleCount = 150000` is checked against the length of the top-level
JSON array, returning `JSONTooManyRules`) and against adblock-rust's converter
(`src/content_blocking.rs:253-261, 582-592`, where
`CbRuleEquivalent::SplitDocument` emits two content-blocking rules for one ABP
rule). Gating it in CI on the emitted artefact is §3.3's proposal, which this
document adopts; no such gate exists yet. **[design]**
- **Every partition is exception-closed**: the full exception set is duplicated
into every partition, and only block rules are partitioned. `research.md` §3.4
establishes this against WebKit's own evaluator
(`ContentExtensionsBackend.cpp:152-168, 172-200, 280-310`):
`ContentExtensionsBackend::actionsForResourceLoad` maps over the installed lists
and calls `actionsFromContentRuleList` once per list; `ignore-previous-rules` is
resolved *inside* that per-list function, truncating that list's own action
vector; and the cross-list combination is a logical OR in which any list's block
action sets `blockedLoad`. So an exception compiled into one partition cannot
cancel a block in another. A size-based split therefore silently disables
exception rules — and the sites that breaks are the bank and government sites
the spec's Edge Cases name as abandonment triggers. **[design]** Proving the
property by test on the emitted JSON is §3.4's proposal and this document's
design decision; the test does not exist yet, and until it does the rule rests
on the source reading above. The residual risk, named rather than hidden: if
that reading is ever falsified at a later WebKit version, the duplicated
exception set is bytes paid for nothing and the partitioning rule is revisited
against the same file at the pinned version.
- Blocking is active on first launch with no configuration (**FR-008**), which
forecloses deferring the corpus to a background fetch to keep the SC-001
download entry clean. The corpus ships, and its bytes are stated under
**FR-043**.
- The refresh, if it is a timer at all, is an enumerated wake in the budget file
carrying its period, its processor-time bound and its justifying requirement
(**SC-005**), coalesced with the platform scheduler, not waking the machine from
sleep, and completing within 50 ms of processor time.
- Blocking never becomes a transmission: no request is made to ask whether a
resource should be blocked. Matching is local, against a locally held corpus
(**FR-007a**).

**Residence**: B for the shipped corpus; L for the compiled artefact and the
fetched revision.

### 1.12 SiteBlockingException

**Forced by**: FR-008 (a visible per-site control), Story 1 acceptance scenario
5, Edge Cases ("the per-site control must be discoverable at the moment of
failure").

| Field | Type | Notes |
| --- | --- | --- |
| `site` | site key | |
| `blocking_enabled` | bool | `false` means blocking off for that site alone. |
| `set_at` | timestamp | |
| `window_scope` | `Persistent \| PrivateWindow(window_id)` | |

**Validation**

- Persists across restarts. The words "the setting persists" are **Story 1
acceptance scenario 5**'s, not FR-008's: FR-008 requires blocking active on
first launch without configuration, a visible per-site control, and the
collapsing of blocked slots, and says nothing about persistence.
- Reachable from the failing page's own chrome affordance and not only from
settings, because this cohort abandons rather than hunts (**Edge Cases**). That
is a surface obligation, recorded here because it constrains where the entity is
read.
- Never transmitted (**FR-007a**).

**Residence**: L.

### 1.13 ImportJob

**Forced by**: FR-012 (import bookmarks and history from Chrome, Firefox and
Edge), Assumptions (import sources; site credentials out of scope), FR-007a
(this is local computation and is permitted), Q-E5.

| Field | Type | Notes |
| --- | --- | --- |
| `source_browser` | `Chrome \| Firefox \| Edge` | Closed, from **Assumptions**. |
| `scope` | `{ bookmarks, history }` | Site credentials are excluded — **FR-015a** forbids Evreos holding them, and what may not be held cannot be imported (**Q-E5**). |
| `state` | `Pending \| Reading \| Written \| Failed(reason)` | |
| `counts` | `{ bookmarks_imported, history_imported }` | |

**Validation**: imported records are ordinary `Bookmark` and `HistoryEntry` rows
carrying `source = Imported`, and are class L from the moment they exist.

**Residence**: L.

### 1.14 HandOffBrowser

**Forced by**: Key Entities, FR-015a, FR-037, FR-007a (hand-off is the fourth
enumerated transmission and its destination is "a program on the same machine,
not a server").

| Field | Type | Notes |
| --- | --- | --- |
| `selection` | `SystemDefault \| Nominated { program }` | Where Evreos is itself the default, the member nominates one once in settings, defaulting to the platform's own. |
| `nominated_at` | timestamp | |

**Validation**

- Evreos MUST NOT nominate itself (**Key Entities**).
- A hand-off passes the address of the current site, on the member's action for
that occasion, and nothing else (**FR-007a**).

**Residence**: L.

### 1.15 SearchProviderSetting

**Forced by**: FR-003a, FR-007a (submitted search is the third enumerated
transmission), FR-042 (brand configuration may change the recipient), Q-E2.

| Field | Type | Notes |
| --- | --- | --- |
| `provider` | provider identity | Default DuckDuckGo (**Q-E2**); changeable by the member from first run, without penalty. |
| `endpoint` | from `BrandConfiguration` | **FR-042** holds endpoints in one place; **FR-003a** allows brand configuration to change which service receives the query. |

**Validation**

- The request carries only the terms the member submitted. It MUST NOT carry an
address the member navigated to, page content, the member's history or
bookmarks, or any identifier Evreos assigns or persists across searches
(**FR-003a**).
- Nothing is sent before the member submits (**FR-003a**).
- Changing the provider changes which service receives the query and MUST NOT
change what the query carries (**FR-003a**).
- No paid placement or revenue-sharing arrangement exists for v1; should one
ever, it is disclosed in the product rather than only in a policy document
(**FR-003a**).

**Residence**: L (the setting), B (the endpoint's default, via brand config).

### 1.16 SiteCredential — an entity deliberately not modelled

**Forced by**: FR-015a, FR-023, Q-E4, Q-E5, Non-Goals (no built-in password
manager).

Evreos stores **no** site credentials in v1. There is no entity, no store and no
field. The only related state is:

| Field | Type | Notes |
| --- | --- | --- |
| `AutofillAvailability` | `Present \| Absent \| Untested` per tier | The result of the test **FR-015a** requires — platform version tested, reference machine, presence or absence — committed to this repository, owned by the founder, and a release blocker per tier until it is. |
| `PasswordFieldDetection` | transient | Local to the device; inspects only whether a password-type input is present; transmits and retains no page content (**FR-015a**). |

**FR-041** forbids the distribution page asserting either presence or absence
until that result exists.

**Residence**: B (the committed test result), T (detection).

---

## 2. Shell configuration and measurement

### 2.1 LanguageCatalogue

**Forced by**: FR-035 (German, Greek and English, keyed by the BCP-47 primary
language subtag alone — `de`, `el`, `en` — with no region subtag in the key and
place never fused into the language value), FR-016a (the neutral menu entry is a
static label drawn from these catalogues), FR-041 (the distribution page carries
the same language obligation, verified on the published page), Principle VII.

| Field | Type | Notes |
| --- | --- | --- |
| `language` | `de \| el \| en` | The whole of the key. |
| `messages` | map of message key → message | Interpolation is by named argument. |
| `brand_arguments` | named arguments | Brand names enter through arguments, never through message text, because **FR-042** forbids a brand name outside the brand configuration. |

**Validation**

- No region subtag appears in a catalogue filename, in a catalogue key, or in
any message key. `de-DE` re-fuses the two values FR-035 separates, and FR-035
names that exact failure.
- No message key, and no stored preference, interface state or request field,
fuses language and place (**FR-035**, **Principle VII**).
- Every catalogue resolves at first run with no account, no network and no
Apivo state, because **FR-016a**'s neutral menu entry is drawn from it and must
be present from first run.
- A rendering of each of `de`, `el` and `en` shows no untranslated string —
required of the distribution page by **FR-041** and applied to the shell as the
same check.

**Residence**: B.

### 2.2 BrandConfiguration

**Forced by**: FR-042 (no brand name, colour, endpoint or support address
hardcoded outside a single brand configuration; a fixture brand builds in CI on
every change), Principle VIII, Q-E13, FR-007a and FR-003a (what brand
configuration may and may not change about a transmission).

| Field | Type | Notes |
| --- | --- | --- |
| `brand_name` | string | |
| `colours` | palette | |
| `endpoints` | map of purpose → endpoint | Every service address the shell may reach. |
| `support_address` | address | |
| `search_provider_endpoint` | endpoint | **FR-003a**. |

**Validation**

- No brand name, colour, endpoint or support address exists anywhere else in the
build (**FR-042**). The fixture brand building in CI on every change is what
proves the seam rather than asserting it (**FR-042**, **Principle VIII**).
- Brand configuration may change **which server receives an enumerated
transmission**, and MUST NOT add a transmission FR-007a's list does not carry,
widen what an enumerated one carries, or remove the member action an entry
requires (**FR-007a**).
- Changing the search provider changes the recipient and not the payload
(**FR-003a**).
- No partner-branded distribution ships in v1 and none is promised (**Q-E13**);
the seam exists regardless.

**Residence**: B.

### 2.3 BudgetEntry

**Forced by**: FR-043 (one budget file; every pull request states its byte and
millisecond cost against it), Principle II, and the Success Criteria preamble,
which defines the entry, the three gates, and the closed list of entries.

A **budget entry** is one number, for one criterion, on one platform, under one
stated measurement condition.

| Field | Type | Notes |
| --- | --- | --- |
| `criterion` | `SC-001 \| SC-002 \| SC-004 \| SC-005 \| SC-006` | SC-003 states no figure and carries no entry. |
| `name` | string | e.g. `download size`, `warm start`, `ten-tab memory`, `tab switch`. |
| `platform` | `windows \| macos` | A figure stated per platform is one entry per platform. |
| `figure` + `unit` | number + `MB \| ms \| percent-of-core` | **[gap]** — the file as it stands is `figure_mb` throughout, and SC-002/SC-006 are milliseconds while SC-005 is a percentage of one core plus two processor-time bounds. |
| `condition` | string | The stated measurement condition; part of the entry's identity. |
| `status` | `ratified \| provisional` | Describes the figure alone, never whether a gate exists. |
| `founder_decision` | reference | Required when `status = ratified`; the budget-file gate fails on a ratified entry naming none. |
| `baseline` | number | The regression gate compares against this. |
| `tolerance` | percentage | Justified by measured run-to-run variation on the pinned runner; ≤ 5% of the **baseline**; undeclared means zero. |
| `cross_check_margin` | number | **SC-004 only**; declared and justified exactly as a tolerance is; undeclared means zero. |
| `spike_exemption` | `{ pull_request, figure }` or absent | Exempts that one entry's **absolute** gate and nothing else. Never lifts the regression or budget-file gate. |
| `baseline_reset` | `{ date, measured_cost, requirement_served, founder_decision }` | Upward only by recorded founder decision, in its own commit, never above the entry's stated figure. |

**Validation** (each of these is a budget-file gate failure, from the preamble)

- An entry a criterion states is missing.
- A recorded baseline above that entry's stated figure.
- A status absent, or a ratified entry naming no founder decision.
- A tolerance or cross-check margin above its limit.
- An upward baseline reset naming no recorded founder decision.
- Either tier's pinned runner identity absent.
- SC-005's wake enumeration absent, or a wake in it lacking a period, a
processor-time bound or a justifying requirement.

**Closed list**: nine entries per platform, eighteen in all — SC-001 download
size and installed footprint; SC-002 warm start and cold start; SC-004 ten-tab
memory; SC-005 60-minute window figure and wake-free 1-second sample; SC-006 tab
switch and address-field keystroke. Adding an entry is a spec amendment.

**Current state, recorded as a gap**, read against `budgets.toml` and
`scripts/check-budgets.py` as they stand on `main` (`feat/budget-gate` is
merged): the file carries only SC-001's four entries, so fourteen of the
eighteen are absent — and `check_budget_file` iterates only over the entries
declared, never comparing them against the preamble's closed list, so a missing
entry is not caught. It has no `unit`, no `cross_check_margin`, no
`founder_decision`, no `spike_exemption`, no wake enumeration, and no display
refresh in the runner blocks that SC-006's 60 Hz condition needs; the script
checks for none of those either. Both runner identities are empty and the
budget-file gate does block on that — but the CI workflow's blocking invocation
passes `--allow-unpinned-runners`, which moves it to advisory until the two
machines Q-E9a names are procured, and passes `--allow-unmeasured` on the same
terms for SC-001's two installed-footprint entries, which have no installer to
measure yet. The script now treats any other unmeasured entry as a budget-file
failure. Both deferrals are stated in the workflow with what retires each. All
`baseline_mb` values are `0.0`, and the regression comparison is skipped while a
baseline is not positive, so the regression half of those entries is inert until
the first real measurement writes a baseline — the commit that first measures
must also set it.

**Residence**: repository state (versioned, not member data).

### 2.4 PinnedRunner

**Forced by**: the Success Criteria preamble, Q-E9a, Assumptions (reference
hardware), SC-013.

| Field | Type | Notes |
| --- | --- | --- |
| `tier` | 1 or 2 | |
| `platform` | `windows \| macos` | |
| `model` | string | The oldest configuration that tier's floor admits, at 8 GB. |
| `os_version` | string | |
| `memory` | string | |
| `identity` | durable machine identifier | Absent identity fails the budget-file gate. |
| `display_refresh` | Hz | **SC-006** measures on a display driven at 60 Hz. **[gap]** — not in the file. |
| `engine_runtime_version` | string, per run | Recorded as a measurement condition on every hardware-dependent entry; the runtime is evergreen on tier 1 and OS-bound on tier 2, so it moves figures with no change to Evreos. **[design]** |

**Residence**: repository state.

### 2.5 WakeEnumerationEntry

**Forced by**: SC-005 ("every scheduled wake on the idle path MUST be enumerated
in the budget file with its period, its processor-time bound and its justifying
requirement"; "no periodic timer outside the enumeration may exist on the idle
path").

| Field | Type | Notes |
| --- | --- | --- |
| `name` | string | |
| `period` | duration | |
| `processor_time_bound` | ms | ≤ 50 ms per wake. |
| `justifying_requirement` | requirement id | |

**Validation**

- Each wake is coalesced with the platform's own scheduler and MUST NOT wake the
machine from sleep (**SC-005**).
- The enumerated wakes together consume ≤ 500 ms of processor time in any
60-minute window — at most ten 50 ms wakes in an hour.
- At the time of writing the enumeration is: the update check (**FR-014**), the
blocking-list refresh (**FR-008**), and — only where the member enabled
diagnostics — the retention evaluation (**FR-039a**). None is *required* to be a
scheduled wake; FR-039a evaluates whenever the browser runs, so an
implementation with no timer for it enumerates none.
- The prohibition on unenumerated timers is verified by design review and by
instrumentation of scheduled work rather than by observation, since no finite
window can falsify a timer with a longer period (**SC-005**). This design's
instrument is a single timer facility whose arming requires an identifier drawn
from a set generated at build time from this enumeration, so an unenumerated
wake fails to compile. **[design]**

**Residence**: repository state.

### 2.6 GateRunRecord and DiscardRecord

**Forced by**: SC-006 (the discard budget is two invocation-level discards per
head commit, counted cumulatively across every run of the gate on that commit;
re-running does not reset the count; only a new head commit carries a new
budget), SC-013 (publication and reproducibility), SC-004 (cross-check delta),
FR-038.

`GateRunRecord`: `{ commit_sha, entry, figure, baseline, verdict,
runner_identity, engine_runtime_version, conditions, raw_samples }`.

`DiscardRecord`: `{ commit_sha, interaction, cause, observed_by, at }`.

**Validation**

- Discards are counted by `commit_sha`, cumulatively across every invocation on
that commit. The third discard on a commit fails the gate. A gate that knew only
its own invocation would grant two discards per run, which is unlimited retries
with extra steps — the behaviour SC-006 says "passes any hard maximum with
probability one".
- A discard is invocation-level only, for a recorded, externally observable
cause the harness detects. Individual trials are never discarded.
- The ledger is durable state indexed by head commit and is published under
SC-013 with cause and commit. It cannot live in the repository at a path a
commit would change, because committing it changes the head SHA and so resets
the budget it constrains. **[design]**
- Raw per-sample and per-trial data is published, not only summaries: 5,760
samples per process for an 8-hour soak, every one of the ≥1000 trials per
interaction (**SC-013**).

**Residence**: published run record.

---

## 3. The app platform

### 3.1 App

**Forced by**: FR-016 (the home surface presents every installed first-party app
the member has not removed), FR-016a (dismissible, keyed to the app identity
declared in the signed manifest; identity MUST NOT change across an update; an
app republished under a new identity is treated as dismissed for everyone who
dismissed its predecessor), FR-017, FR-019, Key Entities.

| Field | Type | Notes |
| --- | --- | --- |
| `app_id` | app identity | Declared in the signed manifest (**FR-017**); stable across updates (**FR-016a**). |
| `display_name` | catalogue key or manifest field | |
| `installed` | bool | Removing from the home surface does not uninstall (**FR-016**). |
| `manifest` | `AppManifest` | §3.2. |
| `surface` | `AppSurface` | §3.4. |
| `supersedes` | `app_id` or none | The predecessor identity whose dismissals it inherits (**FR-016a**). |

**Relationships**: one `App` has one current `AppManifest` and one cached
`AppSurface`; zero or more `Grant`s; zero or one `DismissalRecord`.

**Validation**

- `app_id` MUST NOT change across an update (**FR-016a**).
- An app whose `supersedes` names a dismissed identity is itself dismissed for
that member, without the member acting again (**FR-016a**).
- **[gap]** The spec does not say **where** succession is declared. Declaring it
in the app's own manifest makes it self-attested by exactly the party FR-016a
constrains — "without that, an operator clears every dismissal by renaming" — so
this design proposes it be carried in a root-signed document rather than in the
app's manifest, together with the honest statement that no client mechanism
binds the holder of the root key. That is a design decision to record, not a
requirement to cite.

**Residence**: LR (the installed app's delivered parts), B (the shipped roster,
if one is adopted — see *Gaps*).

### 3.2 AppManifest

**Forced by**: FR-017 (each app declares its capabilities in a signed, versioned
manifest and MUST NOT be able to widen them from inside), FR-018 (every
declarable capability is classified in the published catalogue), FR-019a (the
surface signature binds the manifest digest), FR-036a (no capability may require
a device or member derivation), Principle IX.

| Field | Type | Notes |
| --- | --- | --- |
| `app_id` | app identity | **FR-016a** keys dismissal to it. |
| `manifest_version` | version | **FR-017** requires the manifest versioned. |
| `declared_capabilities` | set of capability names | **FR-017**. |
| `signature` | signature over the manifest | **FR-017**. |
| `digest` | SHA-256 of the manifest bytes | Bound into the surface signature by **FR-019a**. |

**Validation**

- An app cannot widen its capabilities from inside (**FR-017**). The effective
set is computed by the shell (§3.6), never read from anything the app controls
at runtime.
- A declared capability the shipped catalogue does not classify is never granted
(**FR-018**), so an implementer cannot escape the grant by naming a new
capability.
- A manifest MUST NOT declare a capability whose effect is to place device,
display, font, network or timing characteristics in the hands of a party that
could derive an identifier or correlator — including at one remove, by
forwarding the inputs (**FR-036a**).

**Residence**: LR (fetched and cached), verified against B (the pinned root).

### 3.3 Capability and CapabilityCatalogue

**Forced by**: FR-018 (a capability is page-adjacent when its subject is a web
page the member visits or has visited, or that page's context, whether the app
alters it, reads it, or only observes it; every declarable capability MUST be
classified in the catalogue published with the manifest format; an unclassified
capability MUST NOT be granted), FR-036a, Principle IX.

| Field | Type | Notes |
| --- | --- | --- |
| `name` | capability name | |
| `page_adjacent` | bool | The classification **FR-018** requires. |
| `description` | catalogue key | Shown to the member when the grant is asked for. |

**Illustrative page-adjacent set** (FR-018's own list, explicitly illustrative
rather than exhaustive): reading or altering page content; reading the address
or title of the current or any open tab; observing navigation or session state;
reading or writing a page's storage or cookies; running code in a page.

**Validation**

- The catalogue is a **build constant**. Fetching it would hand the delivery
  host
the power to name new capabilities, which is the escape FR-018's last sentence
forecloses. **[design]** — FR-018 requires the catalogue published with the
manifest format and requires unclassified capabilities never granted; shipping
it in the release is how this design makes that hold.
- "Touches page content" is narrower than Principle IX's "anything
page-adjacent" and MUST NOT be substituted for it (**FR-018**). Observing
without altering is page-adjacent regardless: reading the current tab's URL
touches no page content and requires the grant.
- No catalogue entry may name a capability that would require a derivation
FR-036a prohibits.

**Residence**: B.

### 3.4 AppSurface

**Forced by**: FR-019 (updatable without a browser release), FR-019a (signed;
verified before rendering **or** before writing to the FR-020 cache; three named
properties), FR-019b (no app surface and no cached copy ships in an installer or
a browser update; the cache is populated only from surfaces delivered after
installation), FR-020 (cached so that a stated offline state is presented rather
than a blank surface), Principle IX.

| Field | Type | Notes |
| --- | --- | --- |
| `app_id` | app identity | Must equal the app the shell is about to render (**FR-019a**). |
| `surface_version` | monotonic version | **FR-019a**'s no-downgrade rule compares it. **[design]** — a monotonic integer rather than a display version, so ordering is total and unambiguous. |
| `bytes` | surface content | Never on the release path (**FR-019b**). |
| `bytes_digest` | SHA-256 | |
| `manifest_digest` | SHA-256 | Must equal the digest of the manifest whose capabilities it would run under (**FR-019a**). |
| `signature` | one signature over the whole binding | Covers surface bytes, app identity, manifest digest and surface version **together** (**FR-019a**). |
| `verified_at` | timestamp | |
| `state` | see below | |

**Validation** — FR-019a's three properties, each stated as the check that fails

1. **Pinned trust root.** The root is pinned in the shipped shell and is never
fetched, replaced or updated from the host that serves the surface or any host
under the same control. A change of root reaches the member only in a browser
release. (§3.7.)
2. **One signature over the whole binding.** Refuse a surface whose signed app
identity is not the app about to be rendered, or whose signed manifest digest is
not the manifest whose capabilities it would run under.
3. **No downgrade.** The delivered version MUST be ≥ the cached copy's version;
   a
lower version is refused, the cached copy retained, and the refusal stated.

Plus:

- Verification precedes **both** rendering and the cache write (**FR-019a**), so
unverified bytes never reach the cache.
- An unverifiable surface is refused, the cached copy retained, and the refusal
**stated** — not shown as a blank surface, which is what FR-020 already requires
of the offline case (**FR-019a**).
- No surface and no cached copy ships in an installer or an update
(**FR-019b**). A pre-cached surface would carry a valid signature and so would
satisfy FR-019a, which is why FR-019b exists separately; enforcement is
therefore by provenance and artefact contents, not by signature. The design's
four checks: a verified-surface type whose only producer takes bytes handed over
by the delivery client; a release-artefact scan that fails the release job on
any surface bundle, app manifest or surface-cache path in the installer or the
installed tree; a post-install acceptance test that finds the cache absent or
empty before any network activity and every app presenting FR-020's offline
state; and the SC-014-style capture asserting that the first render of a surface
is preceded by that surface's delivery fetch. **[design]**

**State**

```
Absent ──fetch──▶ Delivered ──verify──▶ Verified ──▶ Cached ──▶ Rendered
                       │                                 ▲
                       └──refused (signature, identity,   │
                          manifest digest, or downgrade) ─┘  cached copy retained,
                                                             refusal stated
```

Offline with a cached copy → `Cached` renders with the stated offline state
(**FR-020**). Offline with no cached copy → the stated offline state, never a
blank surface (**FR-020**).

**Residence**: LR. The cache lives under the profile and is member data for
SC-001's purposes; the compiled or materialised product data that first run
creates is not (**SC-001**).

### 3.5 SurfaceVersionFloor

**Forced by**: FR-019a's no-downgrade property.

| Field | Type | Notes |
| --- | --- | --- |
| `app_id` | app identity | |
| `highest_version_seen` | monotonic version | |

**[design / gap]**: FR-019a compares the delivered version against "the version
of the cached copy it would replace". On a fresh install, or after the cache is
cleared, there is no cached copy and so no floor — and whoever controls delivery
at that moment can replay a correctly signed older surface with a known defect.
Holding the floor in a store separate from the FR-020 cache closes that window
and is **stronger than the spec requires**; it is recorded here as a design
decision, with the honest residual that the floor is a local file and an
attacker with profile write access can lower it.

**Residence**: L.

### 3.6 Grant and the effective capability set

**Forced by**: FR-018 (a per-app grant from the member, asked for when the
capability is first used), FR-017, FR-018a (a per-app grant authorises an app to
respond to a qualifying action; it does not authorise injection in its absence),
FR-036a, Story 3 acceptance scenario 5, Principle IX.

| Field | Type | Notes |
| --- | --- | --- |
| `app_id` | app identity | |
| `capability` | capability name | |
| `decision` | `Ask \| Granted \| Refused` | |
| `decided_at` | timestamp | |

**Effective capability set** for an app, computed in the shell:

```
effective = shipped_capability_catalogue          (B — FR-018)
          ∩ manifest.declared_capabilities         (LR, verified — FR-017)
          ∩ { page_adjacent ⇒ Grant = Granted }    (L — FR-018)
          [ ∩ shipped_registry_ceiling ]           (B — design, see below)
```

**Validation**

- A capability the shipped catalogue does not classify is never granted
(**FR-018**).
- An app can never widen its capabilities from inside (**FR-017**); the app
  never
computes this set.
- A grant is asked for at first use, not at install (**FR-018**, Story 3).
- A grant authorises page-adjacent access; it never authorises injection
(**FR-018a**), and it never authorises the derivation FR-036a prohibits
(**FR-036a**).
- The app must be able to ask the shell what it actually holds, so an app
published for a newer shell degrades honestly on an older one rather than
failing obscurely. **[design]**

**[design] Registry ceiling.** Bounding each app to a capability ceiling shipped
in the release limits the blast radius of a compromised publishing key to what
the app already had. It is a design proposal; **FR-017** and **FR-018** require
the manifest and the catalogue, and neither requires a ceiling.

**Residence**: L.

### 3.7 TrustRoot and PublishingDelegation

**Forced by**: FR-019a (pinned trust root; not fetched, replaced or updated from
the serving host or any host under the same control; a change of root reaches
the member only in a browser release), FR-014 (update verification).

`TrustRoot`: `{ public_key }` — compiled into the shell. **B.**

`PublishingDelegation` **[design]**: `{ version, valid_from, valid_until,
entries: [{ app_id, publishing_key, supersedes, capability_ceiling }],
signature_by_root }`.

**Validation**

- The root is never fetched or replaced at runtime (**FR-019a**).
- **[design]** A delegation is accepted only under the pinned root, only at a
version ≥ the highest seen, and only inside its validity window. The monotonic
floor and the window are the countermeasures to replay and to withholding; the
split between an offline root and an online publishing key exists so routine
publishing does not put the root online. FR-019a requires the pin and says
nothing about intermediate keys, so the whole two-level construction is a design
decision. Where the root key lives, who holds it, and the recorded procedure for
signing a delegation is a founder decision not derivable from the spec; see
*Gaps*.
- Residual, stated rather than engineered around: root compromise is recoverable
only by a browser release on a staged **FR-014** channel.

**Residence**: B (root), LR (delegation).

### 3.8 DismissalRecord and HomeSurfaceState

**Forced by**: FR-016 (presented and removed are the two states; hiding the home
surface removes it from the browsing experience and never from the menu entry;
opening it while hidden presents it for that occasion only, and clearing the
hidden state is a separate deliberate choice), FR-016a (dismissible; each choice
persists across restarts and updates; an app update or a browser release MUST
NOT reverse a dismissal; every dismissal is reversible from the same menu
entry), Principle IV.

| Field | Type | Notes |
| --- | --- | --- |
| `subject` | `App(app_id) \| HomeSurface \| Wallet \| ClaimSurface` | The closed set **FR-016a** names as dismissible. |
| `dismissed` | bool | |
| `dismissed_at` | timestamp | |

`HomeSurfaceState`: `{ hidden: bool, presented_for_this_occasion: transient }`.

**Validation**

- Persists across restarts **and updates**; neither an app update nor a browser
release reverses one (**FR-016a**).
- Reversible by the member from the same menu entry (**FR-016a**).
- Keyed to the app identity in the signed manifest, and inherited by a successor
identity (**FR-016a**, §3.1).
- `presented_for_this_occasion` is **transient by construction**. FR-016 is
explicit: opening a hidden home surface from the menu presents it for that
occasion only, the hidden state persists, and clearing it is a separate
deliberate choice — "so that reaching a hidden surface never restores it". A
design that wrote this flag to disk would defeat the requirement that created
it.
- Dismissal removes a surface from the browsing experience, never from the
member's reach: **FR-028** requires a withdrawal followable to a terminal state,
which a wallet with no way back would make unsatisfiable (**FR-016a**).

**Residence**: L (`DismissalRecord`, `hidden`), T
(`presented_for_this_occasion`).

### 3.9 MenuEntry (the neutral entry point)

**Forced by**: FR-016a's neutrality clause, FR-035, Principle IV.

| Field | Type | Notes |
| --- | --- | --- |
| `label` | catalogue key | A **static** label drawn from the FR-035 catalogues. |
| `style` | the menu's own typeface and colour | Not the brand's. |

**Validation** — FR-016a writes the test, and it is a diffable assertion over
two profile fixtures rather than a review item:

- MUST NOT carry a brand colour, a badge, a counter, an amount, a promotional
string, or any state derived from the wallet, the claim surface or any other
money surface.
- MUST read identically on a fresh profile and on a signed-in member's machine.
In this model that is: identical accessible name, identical resolved style
tokens, identical node shape.
- It is not itself an Apivo surface and is exempt from the opt-in rule; without
the exemption, opt-in and discoverability would be circular.
- On a fresh profile that one menu entry is the whole of Apivo's presence
(**FR-016a**).

**Residence**: B (label and style), rendered from L (nothing).

---

## 4. Identity and money

### 4.1 Member and Account

**Forced by**: FR-021 (one account serves every app; sign in once on any Apivo
surface and be signed in on all of them; no app presents a sign-in or an account
of its own; signing out on one surface ends the session on all), FR-022 (signing
in is required for money surfaces and MUST NOT be required for browsing), Key
Entities, Principle IV.

| Field | Type | Notes |
| --- | --- | --- |
| `member_identity` | as the service reports it | **R** — the client never mints it. |
| `session_state` | `SignedOut \| SignedIn` | One value for the whole shell (**FR-021**). |
| `credential_handle` | reference into the OS store | §4.2. |

**Validation**

- Browsing works fully signed out; nothing about Story 1 requires an account
(**FR-022**, **Principle IV**, Story 2 acceptance scenario 5).
- No app holds an account of its own, and no app presents a sign-in
(**FR-021**). The observer test: sign in once, open each installed first-party
app and find the same identity with no second prompt; sign out on one surface
and find the others signed out.
- **FR-036a** forbids deriving any member or device correlator from device,
display, font, network or timing characteristics — so nothing here is seeded
from a machine value, and no such value backs a rollout bucket, a crash group or
an update identity.

**Residence**: R (identity), L (session state).

### 4.2 AccountCredential

**Forced by**: FR-023 (held in the operating system's secure credential store on
every supported platform, **and nowhere else**), Key Entities.

There is no Evreos-owned storage for this value. The model holds only a handle.

| Field | Type | Notes |
| --- | --- | --- |
| `store_reference` | platform store reference | The credential itself is never read into an Evreos-owned store, file, database, preference store, cache or log. |

**Validation**

- No Evreos profile file, database, preference store, cache or log may hold the
credential, a token derived from it, or any value from which either can be
reconstructed (**FR-023**).
- Where the store is unavailable, or the member declines access to it, the
  member
stays signed out. The credential MUST NOT fall back to storage Evreos writes
itself (**FR-023**).
- The observer test is a search of the profile directory and the logs finding no
match, and deletion of the platform entry leaving the member signed out
(**FR-023**).

**Residence**: platform store — outside every class above, deliberately.

### 4.3 Amount, StateTotal and Stale

**Forced by**: FR-026, FR-026a, Principle V. See *Invariant B*.

| Type | Shape | Rules |
| --- | --- | --- |
| `Amount` | `{ value, currency }` | Constructed only by the API deserialiser. No addition, no summation, no multiplication, no rounding, no currency conversion, no comparison that implies arithmetic. |
| `StateTotal` | `{ state, Amount }` | A field the service sent, never a fold over entries. |
| `PayableAmount` | `Amount` | Present only where the service reports one (**FR-026**). |
| `Stale<T>` | `{ value: T, received_at }` | The only representation of a value held on the device (**FR-026a**). No conversion back to a bare value. |
| `DisplayString` | service-rendered text for a (language, place) pair | **[gap]** — see *Gaps*: formatting an amount in the client puts rounding and symbol placement in the client, while **FR-035** requires language and place to stay separate values. Whether the API offers a rendered string is unverified. |

### 4.4 WalletEntry and LedgerSnapshot

**Forced by**: FR-026, FR-026a, FR-027, Key Entities ("a ledger-derived amount
in a stated state — pending, confirmed, declined or reversed — never computed by
the client"), Principle V, Edge Cases.

`WalletEntry`:

| Field | Type | Notes |
| --- | --- | --- |
| `entry_id` | service id | |
| `state` | `Pending \| Confirmed \| Declined \| Reversed` | The closed set **FR-026** and **Principle V** require displayed. |
| `amount` | `Amount` | Exactly as reported. |
| `pending_reason` | reason | Only where `state = Pending`; **FR-027** requires a plain-language explanation. |
| `occurred_at` | timestamp from the service | |
| `merchant_reference` | service reference | |

`LedgerSnapshot`: `{ entries, state_totals: [StateTotal; 4], payable:
Option<PayableAmount>, received_at }`.

**Validation**

- Every amount the service reports is presented, in the state the service
reports for it, exactly as reported (**FR-026**).
- No amount the service did not report is presented; nothing is computed,
estimated, aggregated or omitted, whatever its source (**FR-026**).
- All four states are present in the snapshot whether or not each holds entries,
so dropping declined and reversed is a missing branch rather than a plausible
layout (**FR-026**).
- Filtering or paginating **entries** is permitted; removing a **state** is not
(**FR-026**).
- A held value is `Stale<LedgerSnapshot>`, presented as stale with the time it
was last received, never as a current balance (**FR-026a**, Edge Cases).
- On reconnection the service's value replaces the cached one outright: no
reconcile, no merge, no diff (**FR-026a**).
- The client MUST NOT post, pair or balance ledger entries; MUST NOT generate,
hold or infer the evidence for an entry; MUST NOT approve, pre-approve or
predict the outcome of a payout; MUST NOT deduplicate money actions or treat a
retry of its own as having settled one (**FR-026a**).

**Residence**: R (the truth), LR (`Stale<LedgerSnapshot>`).

### 4.5 WithdrawalRequest

**Forced by**: FR-028 (request a withdrawal and follow its status to a terminal
state), FR-026a (no client deduplication of money actions; no predicted
outcome), FR-016a (dismissal never removes reach, because FR-028 requires the
status followable), Key Entities.

| Field | Type | Notes |
| --- | --- | --- |
| `request_id` | service id | The client does not mint it. |
| `requested_at` | timestamp | |
| `status` | as reported by the service | |
| `is_terminal` | as reported by the service | **[gap]** — the client must not decide that a status is terminal; the spec does not say who publishes the terminal set. |
| `outcome_unknown` | bool | Set when a submission's response was lost. |

**State**

```
Requested ──▶ (service-reported statuses) ──▶ Terminal
     │
     └──▶ ResponseLost ──re-read status──▶ (service-reported statuses)
```

**Validation**

- On a lost response the client neither retries blind nor infers an outcome: it
re-reads status and shows an explicit unknown state (**FR-026a**).
- The wallet stays reachable from the menu entry after dismissal precisely so
  the
status can be followed (**FR-016a**, **FR-028**).
- **[design]** A server-issued withdrawal token, obtained before submission,
keeps exactly-once wholly behind the API where **Principle V** puts it, and
avoids a client-generated idempotency key that sits close to the client owning
exactly-once. Whether the API offers one is unverified; see *Gaps*.

**Residence**: R, rendered from LR.

### 4.6 Offer, MerchantCatalogueEntry and OfferActivation

**Forced by**: FR-024 (browse the merchant catalogue, with language and place as
independent parameters), FR-018b (no advertising in a page; a cashback offer
surface is rendered in the browser's own chrome, never in the page), FR-018a
(the occasion test), FR-030, Key Entities ("a catalogue entry, resolved by
language and place as separate parameters").

`MerchantCatalogueEntry`: `{ offer_id, merchant, terms_text, language, place,
... }` — **LR**.

`OfferActivation` — **T**, and transient is the point:

| Field | Type | Notes |
| --- | --- | --- |
| `offer_id` | reference | The specific offer named to the member in the control (**FR-018a**: "addressed to the specific thing it authorises"). |
| `tab_id` | reference | |
| `navigation_epoch` | the tab's epoch at the moment of the action | |
| `taken_at` | timestamp | |

**Validation**

- Language and place are two parameters, never one (**FR-024**, **FR-035**).
- The offer surface is rendered in the browser's own chrome, never in the page
(**FR-018b**). The Permanent Prohibition on advert injection admits no consent
exception, so a member tapping "show offers here" does not make an injected
panel permissible.
- An activation is valid **only** while `tab.navigation_epoch` equals the epoch
recorded at activation. Any address change — including one the page performs
without fetching a new document — invalidates it (**FR-018a**). A reload and a
restored tab are each a new occasion.
- Interaction with page content is never a qualifying action: a click, keypress,
scroll or pointer movement anywhere in the page authorises nothing, at any
position and on any element (**FR-018a**). An overlay armed by the member's
first interaction with a merchant's page is prohibited however it is described.
- A per-app grant under FR-018 does not supply an occasion (**FR-018a**).
- **Consequence for the model**: because FR-018b forbids in-page offer surfaces
outright, v1 needs no in-page injection mechanism at all, and no capability in
the catalogue writes into, reads from or executes script in a web page the
member visits. Offer detection therefore runs in the shell against the current
address, matched locally against a downloaded merchant list — which is also what
keeps **FR-007a** intact, since asking a service whether an offer applies to the
current address is not one of the four enumerated transmissions. **[design]** —
and it carries a size cost against **SC-001**, a memory cost against **SC-004**,
and, if refreshed on a timer, an enumerated wake under **SC-005**.

**Residence**: LR (catalogue), T (activation).

### 4.7 ClickOut

**Forced by**: FR-025 (opening an offer routes through a click-out URL issued by
the service for that occasion; the user is told that tracking is taking place;
the client MUST NOT construct, template or modify an affiliate link or any of
its parameters), FR-030 (attribution never attached without an explicit member
action for that occasion, and never claimed for a purchase the member's click
did not lead to), FR-040 (following a click-out is one of the four
member-initiated acts), Key Entities, Principle V, the Permanent Prohibition on
silent affiliate attribution.

| Field | Type | Notes |
| --- | --- | --- |
| `url` | `ClickOutUrl` | A newtype constructible **only** from the API response field. Navigated byte for byte. |
| `offer_id` | reference | |
| `issued_at` | timestamp | |
| `occasion` | the `OfferActivation` it discharges | |
| `disclosure_shown` | bool | **FR-025** requires the member told plainly that tracking is taking place. |

**Validation**

- The client never constructs, templates or modifies the link or any parameter
of it (**FR-025**). The check that makes this mechanical rather than asserted is
a test asserting **byte equality** between the response field and the navigated
address — which catches the plausible accident, a URL round-tripped through a
parser that normalises it. **[design]**
- Attribution is attached only on an explicit member action for that occasion,
and never on a weaker connection — not a visit to the merchant, not a search,
not a session already in progress, not a click made elsewhere (**FR-030**).
- Carries the FR-040 client-type marker, because following a click-out is one of
the four enumerated member-initiated acts.

**Residence**: R (issued), T (used).

### 4.8 MemberInitiatedAct (the FR-040 marker)

**Forced by**: FR-040 (the origin marker is a client-type field carried on
requests that a deliberate member act on an Apivo surface initiates, **and on no
others**; the acts are a closed enumeration), SC-011, SC-012, Q-E14, FR-036a.

| Value | Act |
| --- | --- |
| `SignIn` | signing in |
| `OpenWallet` | opening the wallet |
| `RedeemClaimCode` | redeeming a claim code |
| `FollowClickOut` | following a click-out to a merchant |

**Validation**

- A request the client makes without such an act — a wallet or catalogue refresh
at launch, a background token renewal, an update check, a retry — MUST NOT carry
the marker and MUST NOT count towards retention, or the criterion measures
browser launches rather than members returning (**FR-040**).
- The marker MUST NOT be a device fingerprint (**FR-040**, **FR-036a**).
- Adding an act to the enumeration is a spec amendment (**FR-040**).
- **[design]** The model separates request builders: a member-initiated builder
that carries the marker and a background builder structurally unable to carry
it, so "on no others" is a property of the type rather than of the caller's
care.

**Residence**: request attribute.

### 4.9 ClaimCode and Campaign

**Forced by**: FR-029 (member-facing redemption present and disabled until the
existing service is confirmed), FR-032 (scanning or entering a code opens the
claim flow directly after installation), FR-033 (attribution for a partner
referral comes from a code the member deliberately scans or types and MUST NOT
be inferred from the installation), Key Entities, Q-E11a, SC-010, Principle VI's
prohibition on install-referrer tricks.

`ClaimCode`: `{ code, entered_by: Scanned | Typed, entered_at }` — **T** until
submitted, then **R**.

`Campaign`: `{ campaign_id, ... }` — **R**. Held by the Apivo service. The
client never produces a campaign record or a redemption (**FR-029**, **Principle
V**).

**State**

```
Presented ──scan/type──▶ Entered ──submit──▶ Redeemed
                              │
                              ├──▶ AlreadyRedeemed
                              ├──▶ Expired
                              └──▶ BelongsToAnotherMember
```

**Validation**

- Each of `AlreadyRedeemed`, `Expired` and `BelongsToAnotherMember` has a
distinct plain-language outcome; none may present as a generic error (**Edge
Cases**).
- Attribution comes from the code the member deliberately presented, never from
the installation (**FR-033**). Consequently: one installer artefact is served to
everyone, with no per-partner or per-campaign build, and no post-install fetch
of a referrer token keyed to the download session — either would be the
install-referrer trick **Principle VI** prohibits at one remove. **[design]**
- The download URL's parameter set is language and place only, kept as two
separate values (**FR-041**).
- Redemption is present in the interface and **disabled** until the service is
confirmed (**FR-029**, **Q-E11a**). The disabled state is a build constant, not
a fetched flag: FR-029a's observer test is stated over "a build with no backing
service", so the state must be a property of the build for the test to mean
anything, and a fetched flag would make the disabled state network-dependent.
**[design]**
- **SC-010** cannot be measured until redemption is enabled.

**Residence**: T → R.

### 4.10 DisabledFlowState

**Forced by**: FR-029, FR-029a (partner-facing campaign administration present
and disabled until its backing service exists; the disabled control states in
plain language that the flow is not yet available, is not presented as a control
that failed, and makes no request to any service when the member activates it),
FR-034, SC-008.

| Field | Type | Notes |
| --- | --- | --- |
| `flow` | `ClaimRedemption \| CampaignAdministration` | Two flows, two distinct dependencies (**FR-029**). |
| `enabled` | build constant | |
| `explanation` | catalogue key | Plain language, from the FR-035 catalogues. |

**Validation**

- Present and reachable in the interface; not hidden (**FR-029**, **FR-029a**).
- Activating it produces **no outbound request** and **no error state**
(**FR-029**, **FR-029a**). In this model the activation handler contains no
network call path at all, so the observer test cannot fail by accident.
- The explanation is reachable by keyboard and announced to assistive technology
(**FR-011**, **FR-034**, **SC-008**): a control removed from the tab order could
satisfy FR-029's letter — present, reason stated — while a keyboard or
screen-reader member cannot reach the reason at all. **[design]**

**Residence**: B (`enabled`, `explanation`), L (nothing).

---

## 5. Diagnostics

Everything in this section exists only where the member has turned the relevant
signal on (**FR-039**, **FR-039c**), and none of it may carry browsing history,
URLs, page content or search terms (**FR-039**, **FR-039c**, **FR-007a**).

### 5.1 DiagnosticEnrolment

**Forced by**: FR-039a (at most one enrolment per install and at most two
reports per enrolment; both keyed to enrolment, never to install; nothing about
the install date is transmitted), FR-039b (a client MUST NOT retransmit an
unacknowledged report — an unacknowledged enrolment is abandoned and emits
nothing further), FR-036a, Assumptions (cohort week is ISO-8601, Monday to
Sunday, UTC).

| Field | Type | Notes |
| --- | --- | --- |
| `enrolment_date` | calendar date, UTC | The first day diagnostics were enabled after installation. |
| `enrolment_week` | ISO-8601 week, UTC | The **only** value ever transmitted. |
| `state` | `NotEnrolled \| Enrolled \| RetentionSent \| Withdrawn \| Abandoned` | |

**Validation — the field set is the requirement**

- The type contains **no generated value of any kind**: no UUID, no nonce, no
salt, no counter, no hash. What turns local state into a per-install identifier
is a field with more entropy than the calendar, and a value held "only locally"
becomes an identifier the moment a log line, a crash frame or a future field
puts it on the wire. **FR-036a** binds on the derivation rather than on the
lifetime of what it produces. "No generated value exists" is checkable; "we will
not send the generated value" is a promise. A test asserts the serialised field
set so adding one is visible in review. **[design]** — FR-039a and FR-039b
require the absence of an identifier; the field-set test is how this design
makes that structural.
- UTC because Assumptions fix cohort week as ISO-8601 Monday–Sunday UTC; a
local-time enrolment date would put the same install in different weeks by
timezone and make timezone weakly inferable at week boundaries.
- At most one enrolment per install, ever (**FR-039a**). Re-enabling after a
withdrawal emits nothing further.

**State**

```
NotEnrolled ──diagnostics enabled──▶ Enrolled ──ack lost──▶ Abandoned
                                        │
                                        ├─ day 24–30 window, browser runs ──▶ RetentionSent
                                        └─ disabled before window closes,
                                           no retention report emitted ─────▶ Withdrawn
```

**[design / gap]** This state lives outside the browsing profile, in the
per-user application-data directory, and is not cleared by "clear browsing data"
or a profile reset; uninstall removes it. Profile-scoped state would let one
install enrol repeatedly and inflate the enrolment denominator, which FR-039a's
one-enrolment-per-install cap exists to prevent. But a file surviving a
privacy-motivated profile wipe is something **FR-039**'s pre-consent disclosure
must state, since that disclosure must name every transmission and its occasion
in plain language. The location is a design decision; the disclosure obligation
is FR-039's.

**Residence**: L.

### 5.2 DiagnosticReport

**Forced by**: FR-039a (three report kinds; each carries only the enrolment
week), FR-039b (no identifier, at the network layer too; relay; encryption to a
key pinned in the build and rotated only by a release), FR-039, FR-039f.

| Field | Type | Notes |
| --- | --- | --- |
| `kind` | `Enrolment \| Retention \| Withdrawal` | Closed (**FR-039a**). |
| `enrolment_week` | ISO week | The whole payload. |

**Validation**

- Carries no identifier of any kind (**FR-039b**).
- Carries no browsing history, URL, page content or search term (**FR-039**,
**FR-007a**).
- Reaches the service through a relay that is structurally unable to read what
  it
forwards: the client encrypts the payload to the receiving service's public key,
**pinned in the build and rotated only by a release**, and the relay sees only
the ciphertext, its length and the destination (**FR-039b**).
- The relay is operated by a legal entity distinct from the one operating the
receiving service, named with its jurisdiction in the pre-consent disclosure,
under a written contract whose existence and effective date are stated there.
**Where no operator is named or no such contract is in force, the signal MUST
NOT be offered and no report may be transmitted** (**FR-039b**) — so the release
gate on FR-039 is a contract, and the model must support a build in which the
whole feature is unofferable and no report path exists.
- Not retransmitted if unacknowledged; the enrolment abandons (**FR-039b**).
- Received, processed and retained only on EU infrastructure (**FR-039f**).
- **[design]** Every report is padded to one fixed ciphertext length identical
across all four report kinds, and at most one report is sent per connection.
FR-039b concedes the relay sees length and destination; it does not require
length to be *informative*, and with four kinds of naturally different size the
relay — which does see the source address — would otherwise learn, per address,
whether that member withdrew or crashed. The fixed size must exceed the largest
crash report, which is a cost stated under **FR-043** and an argument for
bounding the captured frame count.

**Residence**: T (constructed, sent, never stored).

### 5.3 CrashReport

**Forced by**: FR-039c (opt-in, off by default, separately consented; carries
only a symbolised stack trace, the browser and operating-system version, and a
crash-reason code drawn from a closed enumeration committed to this repository;
each frame carries only module name, symbol name, source file and line from
Evreos's own debug information; frame arguments, register contents and strings
read from the heap or the stack MUST NOT be captured or transmitted; full
process memory MUST NOT be captured or transmitted), FR-039e (one report per key
for the life of the install), FR-039b, FR-039f.

| Field | Type | Notes |
| --- | --- | --- |
| `frames` | list of `{ module, symbol, file, line }` | **Only** these four fields, from Evreos's own debug information. |
| `browser_version` | string | |
| `os_version` | coarsened version | **[design / gap]** — see below. |
| `reason_code` | from the committed closed enumeration | A code outside the enumeration is **discarded on receipt rather than counted** (**FR-039c**). |

**Validation**

- No frame arguments, no register contents, no heap or stack strings, no full or
partial process memory (**FR-039c**). A minidump of any kind is excluded: it
includes thread stack memory, and FR-039c's own reasoning for banning full
memory — that forbidding URLs inside a memory image is not implementable —
applies to a partial image too. A return-address list is provably free of them
by construction. **[design]** — FR-039c bans the contents; the capture shape is
this design's answer.
- A free-form reason string is forbidden, because a field that accepts arbitrary
text accepts a URL (**FR-039c**).
- Symbolisation happens on the device, from a symbol table shipped in the
installer. This is settled by the spec rather than chosen: FR-039c enumerates
the three things a frame may carry, and a raw address or module-relative offset
is not among them. A client-queried symbol server would additionally be a
per-crash request keyed to the crashing code path, accounted for by no entry in
**FR-007a**'s closed list. The table's bytes are stated under **FR-043** against
**SC-001**.
- Before consent the member is shown, in plain language, exactly what a crash
report contains and the once-per-install cap (**FR-039c**).
- **Engine-process failures**: on tier 1 web content does not run in Evreos's
process, so a page crash produces no Evreos stack. The reason-code enumeration
therefore carries two disjoint families — Evreos-process crash reasons, and
engine-process failure kinds mapped one-to-one from the runtime's own closed
enumeration — and an engine-process report carries a reason code and an **empty
frame list**, which FR-039d's counter key tolerates because the key is the
symbol list itself. Any crash dump the runtime writes of its own is deleted
unread, never parsed: a renderer dump contains page memory, hence URLs and page
content, which **FR-039c** bans capturing and **FR-007a** bans transmitting.
**[design]**
- **[design / gap]** `os_version` is coarsened to the marketing or build level
rather than the full patch quadruple, and the granularity is committed beside
the reason-code enumeration so widening it is as visible as adding a code.
FR-039d permits "operating-system version" without fixing a granularity. Both
privacy and utility point the same way: a full version string is close to
identifying in a cohort of a few thousand, and it fragments the counter so
finely that no key reaches FR-039e's 50.

**Residence**: L briefly (captured at crash time, symbolised on next launch),
then T (sent, never stored).

### 5.4 CrashReportCapKey

**Forced by**: FR-039e (a client MUST emit at most one crash report per
(symbolised stack, release, operating-system version, reason code) key **for the
life of the install**, enforced on the device and stated in the pre-consent
disclosure).

| Field | Type | Notes |
| --- | --- | --- |
| `key` | `(symbolised_stack, release, os_version, reason_code)` | |
| `emitted_at` | date | |

**Validation**

- Append-only; never cleared. Because the key carries the release, its effect
resets naturally at each release without the set ever being cleared — which is
the property that makes "50 reports under one key are 50 distinct installs" hold
(**FR-039e**).
- A per-day cap would not carry the claim: one machine in a crash loop reaches
  50
over 50 days, which is exactly the single member on a single code path the
threshold exists to suppress (**FR-039e**).
- Contains no generated value, on the same rule as §5.1.
- The cap binds the client and is not a guarantee against a fabricated stream,
because **FR-039b** admits no client credential (**FR-039e**, **FR-039a**).

**Residence**: L.

### 5.5 Counter

**Forced by**: FR-039d (counters, not reports, are the retained artefact; two
counter keys and no others; discard deadlines), FR-039f, Principle VI's
*aggregate* condition.

| Field | Type | Notes |
| --- | --- | --- |
| `key` | `(report_type, enrolment_week)` **or** `(symbolised_stack, release, os_version, reason_code)` | **These two keys are the only ones permitted** (**FR-039d**). |
| `count` | integer | |
| `first_increment_at` | date | Day granularity only. |
| `discard_deadline` | date | See below. |

**Validation**

- A report is added to its counter **on receipt** and discarded by the end of
  the
following calendar day — the finest deadline a day-granularity receipt timestamp
can audit. No report of either kind is retained individually (**FR-039d**).
- No counter is keyed on anything else — machine or processor model, screen
geometry, installed fonts, timezone, language, or any combination of them — and
discarding the reports on schedule does not license one, because a count of one
under a key few installs share is a per-install record whatever it is called
(**FR-039d**). Adding or widening a key is a spec amendment.
- Discard deadlines: a `(report type, enrolment week)` counter 30 days after
  that
enrolment week ends, which is the last day a report bearing that week can arrive
under FR-039a; a crash counter 90 days after its first increment (**FR-039d**).
- The receiving service retains no source address, no transport metadata and no
receipt timestamp finer than the day (**FR-039b**).
- Received, processed and retained only on EU infrastructure, and every figure
derived from them likewise (**FR-039f**).
- **[design]** There is no report store at all: receipt, counter increment and
acknowledgement are one transaction against a counter table, and the report
exists only as a request body in memory. FR-039d's discard deadline is then met
by construction, and the proof is the absence of any table a report could sit in
— auditable by a third party reading the published schema. A retention deadline
enforced by a deletion job is a deadline that fails silently when the job fails.
This also constrains operations: the gateway carries **no per-request
instrumentation** of any kind, since a per-request record carries a receipt
timestamp finer than the day, which FR-039b bans independently of where it is
hosted.

**Residence**: EU service state (**FR-039f**).

### 5.6 DisclosureUnit

**Forced by**: FR-039e (the publication threshold is 50 reports and applies to a
disclosure unit rather than to each counter in isolation; the units are a closed
pair), FR-039a, SC-011.

| Unit | Gated on | Notes |
| --- | --- | --- |
| **Crash stack** | its own report count | One `(symbolised stack, release, OS version, reason code)` counter. |
| **Enrolment week** | that week's enrolment count **less** its withdrawal count | The population the published rate is actually computed over. Gating on enrolments alone would clear the threshold on a week of 60 enrolments and 56 withdrawals and then publish a rate over four people. |

**Validation**

- Below its threshold a unit is **held**: nothing drawn from it may be
  published,
exported or used in any derived figure, in any form — not a count, not a rate,
not a range, not a confidence interval, since an interval around a small count
discloses the same thing more politely. It is withheld and its absence stated
(**FR-039e**).
- At or above threshold the unit is published, and a count inside it that is
itself below 50 — in a healthy week the withdrawal count, which FR-039a requires
beside the rate — is published as a band: "fewer than 10", or the decade it
falls in (**FR-039e**).
- The rate is published rounded to the nearest whole percentage point, and no
further figure from the same unit may recover a banded count exactly
(**FR-039e**).
- A qualifying week is **always** publishable: the gate is the cohort, the band
is the protection, and no implementer may cite FR-039e to withhold a rate whose
cohort clears the threshold (**FR-039e**).
- Where the denominator is not positive, the counts are reported and the rate is
stated as **not computable** (**FR-039a**).
- Signed-out retention is labelled **unverified** and **self-selected** wherever
reported, used only for direction inside the project, and never quoted outside
the project as a measurement (**FR-039a**, **SC-011**).
- **[design]** The disclosure rule is executed by a committed script that takes
the counter export and emits the published figures — implementing the units, the
gate, the banding, the rounding, the not-computable case and the
withheld-and-stated case, and refusing to emit anything drawn from a held unit.
FR-039e has at least six branches and the one easiest to get wrong is the one
that publishes; a checklist followed at publication time is the same kind of
rule Principle II declines to rely on.

**Residence**: published figures; EU service state upstream of them.

---

## 6. Traceability

| Entity | Requirements that force its shape |
| --- | --- |
| Profile | FR-004, FR-007, FR-010, FR-023, FR-035, SC-001 |
| Window | FR-001, FR-007, FR-016a |
| Tab | FR-001, FR-002, FR-005, FR-015, FR-018a, SC-006 |
| NavigationOutcome | FR-015, SC-009 |
| Session | FR-001, FR-007, SC-002, SC-004 |
| HistoryEntry | FR-003, FR-004, FR-007, FR-007a, FR-012 |
| SuggestionIndex | FR-003, FR-007a, SC-006 |
| Bookmark / BookmarkFolder | FR-004, FR-007a, FR-012 |
| Download | FR-004, FR-007, FR-007a, SC-001 |
| SitePermission | FR-006, FR-007, FR-007a, FR-037, FR-041 |
| BlockingRuleSet | FR-008, FR-018a, SC-001, SC-005, Q-E12 |
| SiteBlockingException | FR-008, FR-007a |
| ImportJob | FR-012, FR-007a, Q-E5 |
| HandOffBrowser | FR-007a, FR-015a, FR-037 |
| SearchProviderSetting | FR-003a, FR-007a, FR-042, Q-E2 |
| SiteCredential (absent) | FR-015a, FR-023, FR-041, Q-E4, Q-E5 |
| LanguageCatalogue | FR-016a, FR-035, FR-041 |
| BrandConfiguration | FR-003a, FR-007a, FR-042, Q-E13 |
| BudgetEntry | FR-043, SC-001–SC-006, SC preamble |
| PinnedRunner | SC preamble, SC-013, Q-E9a |
| WakeEnumerationEntry | SC-005, FR-008, FR-014, FR-039a |
| GateRunRecord / DiscardRecord | SC-006, SC-013, FR-038 |
| App | FR-016, FR-016a, FR-017, FR-019 |
| AppManifest | FR-017, FR-018, FR-019a, FR-036a |
| Capability / CapabilityCatalogue | FR-017, FR-018, FR-036a |
| AppSurface | FR-019, FR-019a, FR-019b, FR-020 |
| SurfaceVersionFloor | FR-019a |
| Grant | FR-017, FR-018, FR-018a, FR-036a |
| TrustRoot / PublishingDelegation | FR-014, FR-019a |
| DismissalRecord / HomeSurfaceState | FR-016, FR-016a |
| MenuEntry | FR-016a, FR-035 |
| Member / Account | FR-021, FR-022, FR-036a |
| AccountCredential | FR-023 |
| Amount / Stale | FR-026, FR-026a |
| WalletEntry / LedgerSnapshot | FR-026, FR-026a, FR-027, FR-031 |
| WithdrawalRequest | FR-026a, FR-028, FR-016a |
| Offer / OfferActivation | FR-018a, FR-018b, FR-024, FR-030 |
| ClickOut | FR-025, FR-030, FR-040 |
| MemberInitiatedAct | FR-040, SC-011, SC-012 |
| ClaimCode / Campaign | FR-029, FR-032, FR-033, SC-010, Q-E11a |
| DisabledFlowState | FR-029, FR-029a, FR-034 |
| DiagnosticEnrolment | FR-039, FR-039a, FR-039b, FR-036a |
| DiagnosticReport | FR-039, FR-039a, FR-039b, FR-039f |
| CrashReport | FR-039b, FR-039c, FR-039e, FR-039f |
| CrashReportCapKey | FR-039e |
| Counter | FR-039d, FR-039f |
| DisclosureUnit | FR-039a, FR-039e, SC-011 |

---

## 7. Gaps and open questions

Each item below is something this model needs and the specification does not
supply. None of them is written into the entity definitions as though it were
required. They divide into founder decisions, external dependencies, and
measurements that no amount of design settles.

### 7.1 Founder decisions

1. **The site key.** FR-006 prompts "per site" and FR-008 exempts "for that site
alone", and neither fixes whether the key is the origin, the registrable domain,
or the host. The three give different member-visible behaviour on subdomains,
and a per-site control that behaves differently from the member's expectation on
a bank's login subdomain is the failure the Edge Cases name.
2. **Private-window download records.** FR-007 and FR-004 pull in opposite
directions and the spec does not resolve it. This model's reading — record does
not persist, saved file does — needs confirmation.
3. **Where succession is declared.** FR-016a requires a successor identity to
inherit dismissals but does not say where succession is asserted. Declaring it
in the app's own manifest makes it self-attested by exactly the party the rule
constrains.
4. **Whether a shipped app registry / capability ceiling exists.** Not required
by FR-017 or FR-018; proposed here to bound publishing-key compromise, at the
cost that widening an app's ceiling becomes a browser release.
5. **The root signing key** — where it lives, who holds it, and the recorded
procedure for signing. Not derivable from the spec, and the two-level key design
is unimplementable without it. Belongs in an ADR.
6. **Whether the merchant catalogue is a delivered signed surface or
shell-native.** Delivered keeps catalogue churn off the release cycle as
Principle IX intends, at the cost of capabilities for reading catalogue data and
requesting a click-out; shell-native avoids both and puts catalogue changes back
on releases. Costed under FR-043 either way.
7. **The FR-039c frame-contents reading.** Is "MUST carry only the module name,
the symbol name, and the source file and line" a ceiling or a floor? The answer
decides whether line tables ship in the installer, which is a measurable
download-size cost against a 20 MB budget. Take the reading with the measurement
in hand.
8. **The diagnostic state file's location and clearing semantics**, and the
plain-language disclosure that follows from it (FR-039).
9. **Which reading of SC-014's "every URL-bearing payload" governs.** Read
literally, a conforming build fails on its own FR-014 update check, which
carries a URL but carries no history and which FR-007a therefore does not reach.
Either SC-014 is restated in terms of history-bearing payloads, or a committed
closed list of permitted non-history destinations is added that the capture's
classifier reads. It changes what the criterion means, so it is not an
implementer's call.
10. **Whether an outbound probe may be used to distinguish an intercepting
    network** (FR-015's `Intercepted`). No platform error code denotes it on
    either shipping tier, and a probe is an outbound request FR-007a's closed
    list does not carry. If no combination of platform signals distinguishes it,
    the choice is between a founder decision under Principle VI and an amendment
    to FR-015.
11. **Whether the client formats amounts or renders a service-supplied display
    string** (FR-026 read against FR-035). Formatting in the client puts rounding
    and symbol placement in the client; a service-rendered string for a
    (language, place) pair keeps both out of it.

### 7.2 External dependencies (the Apivo API contract)

12. **Does the service report per-state totals and a payable amount, or only
    entries?** If only entries, FR-026's ban on client aggregation makes the
    wallet unbuildable as specified, and either the API gains total fields or
    FR-026 is amended. This is the single largest external dependency in the
    money model.
13. **Can a wallet hold amounts in more than one currency?** If so, no single
    total exists that the client could show even were it permitted to compute
    one, and per-currency service-supplied totals become mandatory.
14. **Is the FR-027 pending-reason set a closed enumeration with stable codes,
    and is its text keyed by primary language subtag with place separate?** A
    closed code set lets the explanation ship in the FR-035 catalogues and stay
    available in the offline and stale states; a free-form server string does
    not.
15. **Does the service issue a withdrawal token before submission, and does it
    publish the terminal status set?** Without the first, the plan must record
    which reading of FR-026a it takes and why. Without the second, the client
    would be deciding terminality, which FR-028 does not authorise.
16. **Q-E11a** — the existing service is not yet confirmed to hold campaign
    records and accept a redemption. FR-029 ships disabled; SC-010 is not
    measurable until it is confirmed.
17. **Q-E14** — the client-type field and the EU-hosted retention computation
    are
    changes to a service outside this repository; accepted, not assumed.
18. **The FR-039b relay operator and contract.** No operator is named. Until one
    is, with a written contract in force, the diagnostic signal MUST NOT be
    offered and no report may be transmitted — so the model must support a build
    in which the entire feature is dark. Whether a US-incorporated operator
    running EU-only ingress satisfies FR-039b's naming rule and FR-039f's hosting
    rule is a legal question for counsel, not an engineering reading.
19. **The lawful basis for the withdrawal report**, transmitted after the member
    turns diagnostics off. Disclosure under FR-039 is not the same as a basis.

### 7.3 Measurements (spikes — no answer is predicted here)

20. **Q-E10** — whether affiliate attribution survives tracking prevention on
    tier 2. ADR-0001 risk 1 requires this tested before the wallet is designed
    around cookies, so it is an ordering constraint on §4, not only a risk.
21. **Q-E11 / Q-E11b** — content-protected playback on each tier. Nothing in
    this
    model claims or excludes a capability; FR-037's hand-off is built regardless.
22. **Q-E12** — whether FR-008 parity is reachable at the macOS 13 floor, and at
    what cost. §1.11's partition rules hold whatever the answer; the answer
    decides the floor.
23. **The cold-start spike** SC-002's four entries wait on, and **the macOS
    ten-tab memory spike** SC-004's tier-2 entry waits on.
24. **FR-002's suspension mechanism on each tier at its floor.** FR-002 requires
    a *stated* policy; a policy stated from an API that does not exist at the
    floor would be a claim about behaviour nobody measured.
25. **FR-006 on tier 2 at the macOS 13 floor** — what a page actually observes
    when it requests geolocation or notification permission (`research.md` §2.4;
    measurement N9). No requirement forbids the distribution page speaking about
    this before it is measured: FR-041's "MUST NOT assert either presence or
    absence" is scoped to the site-credential autofill test FR-015a requires and
    does not reach FR-006's capabilities, so the silence §1.10 proposes is a
    **[gap]** rather than a citation. Separately, if §2.4's header reading holds,
    it is a tier-2 limitation on a requirement and belongs in ADR-0001's
    capability floor, which today carries nothing about permission APIs; putting
    it there is an ADR amendment, not a change to this document. `SitePermission`
    carries `UnavailableOnThisPlatform` as a distinct value rather than folding
    it into `Denied` so the model can state the fact once it is established.
26. **The FR-015a autofill test on each tier**, whose result is committed to
    this
    repository and is a release blocker for that tier.
27. **Whether the FR-039e floor of 50 is ever reached at pilot scale** for
    crash counters keyed on stack, release, OS version and reason code. FR-039d
    already answers what to do if not — the counters are discarded on schedule,
    and the answer is neither a retained exemplar nor a wider key — but the plan
    should expect that outcome for the first cohorts rather than treat it as a
    defect. OS-version coarsening (§5.3) is the one lever available.

### 7.4 Repository state that must change before measurement lands

28. **budgets.toml carries four of eighteen entries**, has no unit field, no
    `cross_check_margin`, no `founder_decision`, no `spike_exemption`, no wake
    enumeration and no display refresh in the runner blocks; both runner
    identities are empty; every baseline is zero, so the regression half of the
    SC-001 entries is inert until a first measurement writes one. The schema
    lands before the measurements do, and the commit that first measures also
    sets the baseline.
29. **`scripts/check-budgets.py` compares one Linux binary size against both the
    Windows and the macOS download-size entries**, keying measurements on
    `(criterion, name)` with no platform. Linux is the deferred platform and
    neither entry's stated condition is met by it.