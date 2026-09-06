//! Chrome fronts: implementations of [`crate::host::ChromeFront`]. T015
//! needs exactly one, the minimal AccessKit tree; a later chrome-renderer
//! candidate is another module here behind the same trait.

pub mod accesskit_min;
