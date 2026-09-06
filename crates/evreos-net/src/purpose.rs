//! The closed enumeration of why Evreos may transmit anything at all.
//!
//! FR-007a splits what leaves the machine into two categories, and this module
//! keeps them as two disjoint Rust enums inside one [`Purpose`], so that the
//! split is structural: a value is in exactly one set, provable by an
//! exhaustive match, and `scripts/checks/check_purpose_enum.py` can parse the
//! two enum bodies and hold the history-bearing one in agreement with
//! FR-007a's own enumeration text — in both directions, so an enum edited
//! without the specification amendment fails the build.

use core::fmt;

/// Why a transmission is being made — the closed set of them.
///
/// Exactly two variants, one per set, and this is a convention with teeth:
/// the only request path in the workspace is typed on this enum, so a
/// transmission is history-bearing if and only if it is built from a
/// [`HistoryBearing`] value, and that enum's membership is held to FR-007a's
/// four by `scripts/checks/check_purpose_enum.py`. A third category would be
/// a third variant here, which is a diff to this file and a specification
/// question before it is a code question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
    /// One of the four transmissions FR-007a permits to carry browsing
    /// history — an address the member navigated to, a term typed into the
    /// FR-003 field, page content, or any value derived from any of them.
    HistoryBearing(HistoryBearing),
    /// A transmission that carries none of those, ever. Money traffic is in
    /// this set as its own purposes, never as a crate exempted from the
    /// chokepoint: FR-007a's enumeration governs transmissions that carry
    /// browsing history, and money traffic is a different category, not an
    /// exception.
    NonHistory(NonHistory),
}

/// The four permitted history-bearing transmissions — FR-007a's enumeration,
/// exactly, which that requirement states is exhaustive.
///
/// Adding a variant here is an amendment to the specification, made in the
/// pull request that would add the transmission and checked against the
/// Permanent Prohibition there; it is never an implementation decision.
/// `scripts/checks/check_purpose_enum.py` fails the build when this enum and
/// FR-007a's list disagree in either direction.
///
/// The variants are deliberately unit variants: what each transmission
/// carries is bounded by its FR-007a entry, and the value it carries lands
/// with the transport in the story that needs it. Three of the four are not
/// even this crate's traffic — the page load and the certificate status are
/// made by the system web runtime while rendering, and the hand-off goes to a
/// program on the same machine, not a server — but the enumeration is only a
/// closed set if it is all in one place, so all four are named here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryBearing {
    /// The requests that load and use the site the member opened, sent to
    /// that site and to the hosts that page references.
    PageLoad,
    /// The validity check made while loading a site, to the authority that
    /// site's certificate names, carrying that certificate's identifiers.
    CertificateStatus,
    /// The terms the member submits from the FR-003 field, to the default
    /// search provider, on the boundary FR-003a states: only what was
    /// submitted, nothing before submission, no identifier across searches.
    SubmittedSearch,
    /// The address of the current site, passed under FR-015a or FR-037 on the
    /// member's action for that occasion, to the hand-off browser — a program
    /// on the same machine, not a server.
    HandOff,
}

/// The transmissions that carry no address the member navigated to, no term
/// typed into the FR-003 field, no page content and no value derived from any
/// of them.
///
/// Four are the browser's own infrastructure; six are the Apivo money
/// transmissions, present because User Story 2 has no lawful route off the
/// machine without them. The money variants that carry a value carry it in a
/// dedicated typed field — an integer amount, a code newtype with a validated
/// charset — and none carries a free `String`, so none has a field able to
/// hold an address, a search term or page content. FR-040's client-type
/// marker is deliberately not a field on any of them: the money request
/// builder that calls this crate carries it, and only on the member acts
/// FR-040 enumerates.
///
/// Which *destinations* are permitted for this set is a different question
/// from which purposes exist, and it is not answered here: the committed
/// closed list SC-014's classifier reads is gap G16, a specification
/// amendment this crate may not settle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonHistory {
    /// The FR-014 update check: the browser asking whether a verified update
    /// exists for it.
    UpdateCheck,
    /// The refresh of the blocking list FR-008's tracker and advert blocking
    /// depends on. The list comes down; nothing about what the member visited
    /// goes up — a lookup keyed to the address being visited is exactly what
    /// FR-007a names as a plausible, forbidden addition.
    BlockingListRefresh,
    /// FR-019 surface delivery: fetching a signed app surface so apps update
    /// without a browser release, verified under FR-019a before rendering.
    SurfaceDelivery,
    /// The FR-039 report path: the opt-in diagnostic signal's reports, off
    /// until the member turns it on, carrying only what FR-039a and FR-039c
    /// enumerate — which FR-039 states may never include browsing history,
    /// URLs, page content or search terms.
    DiagnosticReport,
    /// Signing in to the member's Apivo account.
    SignIn,
    /// Reading the wallet: ledger-derived amounts in stated states, computed
    /// by the service and never by the client.
    WalletRead,
    /// Redeeming a claim code the member deliberately presented, binding the
    /// member to a partner campaign.
    ClaimCodeRedemption {
        /// The code, in a charset that cannot spell an address, a search
        /// term or page content.
        code: ClaimCode,
    },
    /// A member-initiated withdrawal request.
    WithdrawalRequest {
        /// The amount, as an integer count of minor units. An integer cannot
        /// hold history.
        amount: MinorUnits,
    },
    /// Reading the merchant catalogue of offers.
    MerchantCatalogueRead,
    /// Following a click-out to a merchant: recording the intent to shop
    /// under the reference the service issued, which later attributes a
    /// purchase.
    ClickOut {
        /// The service-issued reference. Service-issued is the point: the
        /// client relays a token it was given, in a charset that cannot
        /// carry what the member typed or visited.
        reference: ClickOutReference,
    },
}

/// Why a typed money value was refused.
///
/// A closed set, like every failure enumeration in this workspace: a
/// catch-all would let a caller treat rejection as noise, and the rejection
/// IS the guarantee — it is what makes "no money purpose can carry history"
/// hold for the fields that are text at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueError {
    /// A character outside the value's closed charset. The charsets exclude
    /// `/`, `:`, `.`, whitespace and every other character an address, a
    /// typed phrase or markup needs, which is how a URL, a search term or
    /// page content fails to fit.
    Charset { found: char },
    /// A length outside the value's stated bounds, `1..=` the type's maximum.
    Length { length: usize },
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Charset { found } => {
                write!(f, "character {found:?} is outside the value's charset")
            }
            Self::Length { length } => {
                write!(f, "length {length} is outside the value's bounds")
            }
        }
    }
}

impl core::error::Error for ValueError {}

/// A money amount, as an integer count of minor currency units.
///
/// An integer by design — "amounts as integers" is the rule that keeps a
/// money field from being a text field. Which currency's minor units, and any
/// bound the service places on an amount, are the service's contract and land
/// with the money transport; what is settled here is only the shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MinorUnits(u64);

impl MinorUnits {
    /// An amount of `count` minor units. Infallible: every `u64` is a valid
    /// count, and no `u64` is an address.
    pub const fn new(count: u64) -> Self {
        Self(count)
    }

    /// The count, for the builder that serialises the request.
    pub const fn count(self) -> u64 {
        self.0
    }
}

const CLAIM_CODE_MAX: usize = 64;
const CLICK_OUT_REFERENCE_MAX: usize = 128;

/// A claim code: ASCII letters, digits and `-`, 1 to 64 characters.
///
/// The charset is the guarantee. It has no `/`, `:` or `.`, so it cannot
/// spell an address; no whitespace, so it cannot hold a typed phrase; no `<`,
/// `>` or `&`, so it cannot hold markup. What the charset cannot do is stop a
/// caller encoding something derived from history INTO a conforming token —
/// that is FR-007a's "any value derived", it rests on review of the one money
/// request builder, and SC-014's published capture is the backstop that reads
/// what was actually sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimCode(String);

impl ClaimCode {
    /// Validate and hold a code the member deliberately presented.
    pub fn new(code: &str) -> Result<Self, ValueError> {
        validated(code, CLAIM_CODE_MAX, |ch| {
            ch.is_ascii_alphanumeric() || ch == '-'
        })
        .map(Self)
    }

    /// The code, for the builder that serialises the request.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The one validator both text newtypes go through: a length in `1..=maximum`
/// and every character inside the type's closed charset, or the value is
/// refused with the first reason found.
fn validated(
    value: &str,
    maximum: usize,
    permitted: impl Fn(char) -> bool,
) -> Result<String, ValueError> {
    let length = value.chars().count();
    if length == 0 || length > maximum {
        return Err(ValueError::Length { length });
    }
    match value.chars().find(|&ch| !permitted(ch)) {
        Some(found) => Err(ValueError::Charset { found }),
        None => Ok(value.to_owned()),
    }
}

/// A service-issued click-out reference: ASCII letters, digits, `-` and `_`,
/// 1 to 128 characters.
///
/// The same charset argument as [`ClaimCode`], with the same honest limit,
/// and one more property that matters to FR-030's rule against silent
/// attribution: the client can only relay a reference the service issued, so
/// the value never encodes anything the client knows and the service does
/// not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClickOutReference(String);

impl ClickOutReference {
    /// Validate and hold a reference exactly as the service issued it.
    pub fn new(reference: &str) -> Result<Self, ValueError> {
        validated(reference, CLICK_OUT_REFERENCE_MAX, |ch| {
            ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
        })
        .map(Self)
    }

    /// The reference, for the builder that serialises the request.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
