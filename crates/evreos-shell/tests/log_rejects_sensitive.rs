//! Proves that logging an address, a credential, or any sensitive wrapper
//! fails to typecheck by construction.
//!
//! Fulfills FR-023, FR-007a, and FR-039c invariants.

#![forbid(unsafe_code)]

#[test]
fn log_rejects_sensitive() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
