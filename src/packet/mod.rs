//! Packet assembly: the high-level [`Context`](context::Context) builder, budgeted sections,
//! manifests, tool schemas/signatures, full-request accounting, token estimates and sync.
pub mod budget;
pub mod context;
pub mod estimate;
pub mod manifest;
pub mod request;
pub mod schema;
pub mod signatures;
pub mod sync;
