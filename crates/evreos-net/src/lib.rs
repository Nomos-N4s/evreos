//! The sole egress chokepoint.
//!
//! FR-007a is a closed enumeration of what may carry browsing history off the
//! machine, and SC-014 proves conformance by capturing every outbound request
//! and classifying it. Both depend on there being one place traffic originates:
//! a transmission added anywhere else is a transmission the enumeration never
//! judged and the capture's analysis never listed. This crate is that place.
//! Every transmission Evreos makes on its own behalf enters through
//! [`request`], which takes a [`Purpose`] naming which permitted transmission
//! this is and an [`Endpoint`] naming where it may go — so a transmission
//! neither set of purposes names has no type to be, and fails to compile
//! rather than failing review.
//!
//! Three boundaries of this crate, stated so nothing is assumed of it.
//!
//! **No networking happens here today.** This crate has no dependencies at
//! all, network-capable or otherwise; [`request`] returns a [`PlannedRequest`]
//! that a later transport implementation will consume, and that transport
//! lands with the story that needs it. What lands now is the type structure
//! under which the transport, when it comes, can only be handed traffic the
//! enumeration already judged. Until then,
//! `scripts/checks/check_egress_chokepoint.py` fails the build when any crate
//! but this one depends on a network-capable crate, so the diff that would add
//! a second egress path is the diff that fails.
//!
//! **FR-040's client-type marker is not a field of this crate.** FR-040
//! requires an origin marker on requests that a deliberate member act on an
//! Apivo surface initiates — and on no others: a launch-time refresh or a
//! token renewal carrying it would make the retention figure measure browser
//! launches. Whether a given request is such an act is knowledge the money
//! request builder has and this crate does not, so the marker is carried by
//! that builder, which calls this crate — giving the marker exactly one
//! implementation in the workspace, where a field here would have invited
//! every caller to set it.
//!
//! **The committed closed list of permitted non-history destinations does not
//! exist yet.** SC-014's classifier will read one; specs/001-evreos-v1's
//! research records it as gap G16, resolved only by a specification amendment.
//! This crate may not settle it: [`Endpoint`] constrains where a value comes
//! from (the brand configuration), never which destinations are permitted,
//! and nothing here is that list.

#![forbid(unsafe_code)]

pub mod purpose;

pub use purpose::{
    ClaimCode, ClickOutReference, HistoryBearing, MinorUnits, NonHistory, Purpose, ValueError,
};

/// A destination address resolved from the brand configuration, on its way to
/// becoming an [`Endpoint`].
///
/// This type exists so that minting an endpoint from a literal is visible and
/// checkable. [`Endpoint`] itself exposes no constructor from any string-like
/// value; the one route to one runs through this wrapper, and this wrapper's
/// constructor is named after the only place allowed to call it:
/// `crates/evreos-shell/src/brand.rs`, the brand configuration seam, whose
/// values are the committed brand files under `brands/`. FR-042 permits brand
/// configuration to change which server receives an enumerated transmission
/// and forbids it adding one, which is exactly the shape this type enforces:
/// the brand file chooses the address, the [`Purpose`] enumeration stays
/// closed.
///
/// The boundary, honestly: the compiler proves that [`Endpoint`] cannot be
/// built from a literal — no `new(&str)`, no `From`/`TryFrom` of a string, no
/// public field exists on it. The compiler does NOT prove who calls
/// [`BrandResolved::declared_in_brand_configuration`]; a caller outside
/// brand.rs handing it a literal is a misuse this crate cannot see. That rests
/// on review, for which the constructor's name is the greppable marker, and on
/// the check suite, which reads source the way
/// `scripts/checks/check_purpose_enum.py` does and can grow a clause binding
/// the call site when the shell lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrandResolved {
    address: String,
}

impl BrandResolved {
    /// Wrap a destination address read from the brand configuration.
    ///
    /// Callable, by rule, only from `crates/evreos-shell/src/brand.rs` with a
    /// value that module resolved from a committed brand file — never with a
    /// literal, and never with a value computed from anything the member did.
    /// The name is deliberate: every call site is one `grep
    /// declared_in_brand_configuration` away from review.
    pub fn declared_in_brand_configuration(address: String) -> Self {
        Self { address }
    }
}

/// Where a transmission may go: an address the caller resolved from the brand
/// configuration, and nothing else.
///
/// No public constructor takes a literal or any string-like value, no
/// string conversion trait is implemented, and the field is private — so the
/// only way to hold an `Endpoint` is to have gone through [`BrandResolved`],
/// whose documentation states which half of that guarantee the compiler
/// carries and which half rests on review. An endpoint is a *destination for
/// an enumerated transmission*; the page-load and certificate-status entries
/// of FR-007a name destinations the member's own navigation chooses, which
/// the system web runtime contacts while rendering and which never pass
/// through brand configuration — those transmissions are the engine's, bound
/// by FR-007a directly, and are represented in [`Purpose`] so the enumeration
/// is complete, not because their traffic originates here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    address: String,
}

impl Endpoint {
    /// The one route to an `Endpoint`: consume a [`BrandResolved`] value.
    pub fn resolve(source: BrandResolved) -> Self {
        Self {
            address: source.address,
        }
    }

    /// The resolved destination, for the transport that will consume it.
    pub fn address(&self) -> &str {
        &self.address
    }
}

/// A transmission this crate has judged and a later transport will perform.
///
/// The value [`request`] returns: purpose and endpoint, bound together, so the
/// transport that eventually sends it cannot be handed one without the other.
/// It carries no body today; each purpose's payload shape is either part of
/// the [`Purpose`] variant itself (the money purposes, whose typed fields are
/// the point) or lands with that purpose's transport story, bounded by what
/// its FR-007a entry or its non-history definition permits it to carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRequest {
    purpose: Purpose,
    endpoint: Endpoint,
}

impl PlannedRequest {
    /// Which permitted transmission this is.
    pub fn purpose(&self) -> &Purpose {
        &self.purpose
    }

    /// Where it is bound for.
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}

/// The one request entry point.
///
/// Takes a [`Purpose`] and an [`Endpoint`] — both, always. A caller that
/// cannot name which enumerated transmission it is making has no `Purpose` to
/// pass; one that did not resolve its destination from the brand
/// configuration has no `Endpoint`; either way the call does not compile.
/// This crate's public surface offers no other path that plans, builds or
/// represents an outbound transmission, and keeping it that way is a review
/// obligation this doc comment states rather than hides.
pub fn request(purpose: Purpose, endpoint: Endpoint) -> PlannedRequest {
    PlannedRequest { purpose, endpoint }
}
