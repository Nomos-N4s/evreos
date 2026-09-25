# Decision 0007: Site key definition for permissions and blocking exceptions

- **Status**: Decided
- **Date**: 2026-09-26
- **Recorded**: 2026-09-26
- **Deciders**: Founder
- **Cite as**: `decisions/0007`

## Question

FR-006 requires that the browser prompt "per site" for camera, microphone,
location, and notification access, and allow those decisions to be revisited.
FR-008 requires tracker and advert blocking to be active on first launch
without configuration, and to offer a visible per-site control allowing
blocking to be disabled "for that site alone".

Neither requirement defines the granularity of a "site". Specifically, the
specification leaves open whether a site is identified by:

1. **Origin** (`scheme://host:port`),
2. **Host** / Fully Qualified Domain Name (FQDN), or
3. **Registrable Domain** (effective top-level domain plus one label, eTLD+1).

These three interpretations diverge sharply on subdomains — particularly during
authentication flows on banking and government services (e.g. `bank.invalid`
versus `login.bank.invalid`). The Edge Cases section of the specification
names bank breakage as a critical failure trigger:

> "Blocking breaks a bank or government site. The per-site control must be
> discoverable at the moment of failure, because this cohort will otherwise
> abandon the browser rather than hunt through settings."

If an exception granted on a landing page does not apply to the login
subdomain, or if each intermediate authentication hop (`auth.bank.invalid`,
`idp.bank.invalid`) is treated as a separate site requiring independent
exception toggling, the login flow breaks and the member abandons the browser.
The plan (`plan.md` §Clarifications) and data model (`data-model.md` §7.1)
reserve this architectural choice to a recorded founder decision.

## Decision

**The site key is fixed as the Registrable Domain (eTLD+1) for domain names,
falling back to the canonical host for IP addresses (IPv4 and IPv6) and
single-label hostnames (e.g. `localhost`).**

A single canonical type, `SiteKey`, implemented in
`crates/evreos-shell/src/site_key.rs`, represents this identity across both the
site permission store (FR-006, T049) and the site blocking-exception store
(FR-008, T069).

Specifically:

1. **Registrable Domain (eTLD+1) Resolution**:
   - For domain names with hierarchical labels, `SiteKey` normalizes the
     address or host string by stripping scheme, port, userinfo, path, query,
     fragment, and trailing dots, converting ASCII characters to lowercase, and
     resolving the registrable domain (`eTLD+1`).
   - All subdomains of a registrable domain map to the same `SiteKey`. For
     example, `https://login.bank.invalid/auth`, `https://www.bank.invalid/`,
     and `https://bank.invalid:8443/` all yield `SiteKey("bank.invalid")`.
   - Multi-part public suffixes (such as `.co.uk`, `.org.uk`, `.gov.uk`,
     `.com.au`, `.ac.uk`) are recognized so that `login.bank.co.uk` resolves
     to `SiteKey("bank.co.uk")`.
2. **IP Addresses and Single-Label Hostnames**:
   - IPv4 address literals (e.g. `127.0.0.1`, `192.168.1.1`) and IPv6 address
     literals (e.g. `[::1]`, `[2001:db8::1]`) resolve to their normalized IP
     host string.
   - Single-label hostnames without dots (e.g. `localhost`, `intranet`)
     resolve to their lowercase host name.
3. **Rejection of Origin**:
   - Origin (`scheme://host:port`) is rejected because it isolates subdomains,
     schemes, and ports. When tracker blocking breaks identity verification on
     `login.bank.invalid`, a member who disabled blocking on `bank.invalid`
     would find the login subdomain still blocked. Because members expect "this
     site" to encompass the organization's login flow, origin isolation directly
     triggers the abandonment failure mode named in Edge Cases.
   - Origin also differentiates between `http` and `https` or standard and
     non-standard ports (`:443` vs `:8443`), which does not match human mental
     models of a site.
4. **Rejection of Exact Host**:
   - Exact host matching is rejected for the same reason: `login.bank.invalid`
     and `bank.invalid` are distinct hostnames. Treating them as separate
     sites would require members to discover and re-apply blocking exceptions
     on redirect hops during sensitive login sequences.
5. **Unified Key Across Permissions and Blocking Exceptions**:
   - Both `SitePermission` (FR-006) and `SiteBlockingException` (FR-008) are
     keyed by `SiteKey`. Members granting a capability (such as microphone or
     camera for video banking) or exempting a site from ad/tracker blocking
     establish that preference for the registrable domain as a whole.

## Evidence

- `specs/001-evreos-v1/spec.md`:
  - **FR-006**: Prompt per site for camera, microphone, location, and
    notification access, revisitable.
  - **FR-008**: Tracker and advert blocking active on first launch with a
    visible per-site control.
  - **FR-007a**: Closed network transmissions; site permissions and blocking
    exceptions are local-only and are never transmitted.
  - **Edge Cases**: Breakage of bank and government sites by blocking triggers
    browser abandonment unless resolved discoverably and comprehensively.
- `specs/001-evreos-v1/plan.md` §Clarifications:
  "NEEDS CLARIFICATION: the site key. FR-006 prompts 'per site' and FR-008
  exempts 'for that site alone'; neither fixes whether the key is the origin, the
  registrable domain or the host, and the three give different behaviour on a
  bank's login subdomain, which the Edge Cases name as an abandonment trigger.
  Settled by: a founder decision recorded with the per-site control's design."
- `specs/001-evreos-v1/data-model.md`:
  - §1.10 (`SitePermission`): keyed by site key.
  - §1.12 (`SiteBlockingException`): keyed by site key.
  - §7.1 item 1: identifies the site key ambiguity as a founder decision.
- `specs/001-evreos-v1/tasks.md`: Task T048.

## Serves

- FR-006 (Per-site permissions for camera, microphone, location, notifications)
- FR-008 (Per-site tracker and advert blocking exceptions)
- Edge Cases (Prevention of browser abandonment on bank/government subdomains)
- Tasks T048, T049, T069

## Consequences

- `crates/evreos-shell/src/site_key.rs` implements `SiteKey` as a validated,
  normalized newtype providing parsing from URLs and host strings.
- Navigating between `bank.invalid`, `login.bank.invalid`, and
  `auth.bank.invalid` shares the same blocking exception and permissions.
- `crates/evreos-shell/tests/site_key.rs` asserts the registrable domain
  resolution over bank login subdomains, IPv4, IPv6, localhost, and URL
  parsing edge cases.

## Corrections

None yet.
