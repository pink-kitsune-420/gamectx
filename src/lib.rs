//! gamectx compiles game data and tool schemas into small, budgeted context packets for
//! AI-driven games.
//!
//! Start with [`Context`]: register tools and prompts once, then add variables and tables
//! each turn. The modules below are the building blocks it uses.
//!
//! - [`request`], [`dialogue`], [`decision`], [`deployment`], [`turn`]: full-request budgets, protected scenes, bounded choices and local deployment telemetry.
//! - [`selection`], [`tune`]: dependency-complete host evidence and split recorded-configuration comparisons.
//! - [`info`], [`inventory`]: optional reported information tools and current ownership/count lookup.
//!   [`dialogue::InteractionTools`] adds scoped events, promises, relationships, references and offers.
//! - [`kit`]: shared data registration, named recipes, validated callbacks, inspection and host bridges.
//! - [`context`]: the easy API ([`Context`], [`Turn`], [`Table`]).
//! - [`table`]: uniform objects to a header plus rows, optionally grouped by one or more
//!   columns (lossless).
//! - [`schema`]: tool schemas to rows with shared `$defs` (lossless), or a one-line outline (lossy).
//! - [`signatures`]: tool schemas as short text lines, 12–14% fewer tokens than packed JSON
//!   Schema (lossless).
//! - [`view`]: sort, keep the top N, keep chosen columns and round numbers before packing.
//! - [`catalog`]: searchable, paged registries instead of whole-registry dumps.
//! - [`budget`]: stable-first assembly that drops low-priority sections to fit a budget.
//! - [`estimate`]: conservative token estimates calibrated from provider usage reports.
//! - [`memory`]: long-term records, scoped facts and cited hypotheses with expiry/privacy, with a
//!   legacy `remember` tool for the model.
//! - [`facts`]: explicit host/user assertions, pins, versioned dependencies and exact typed claim checks.
//! - [`recall`]: optional immutable lexical indexes with host aliases and word normalization.
//! - [`playbook`]: reusable host-validated procedures with current versioned prerequisites.
//! - [`outcome`]: execution/exposure-gated, single-consumption training labels.
//! - [`temporal`]: native event windows, rates and ordered sequence counts.
//! - [`requests`]: sealed one-use asynchronous result tickets with freshness checks.
//! - [`spending`]: integer reservations, settlement and conservative crash recovery.
//! - [`query`]: a small SQL `SELECT` the model writes and the game runs exactly, for
//!   filters and totals models get wrong when they read the data themselves.
//!   Table descriptions keep rows in the game; correction sessions allow one retry.
//! - [`brain`]: small game-defined reaction networks with deterministic local inference,
//!   guarded/sliced training, native inference, versioned bindings, diagnostics and optional stability/caching.
//!   [`brain::BrainBundle`] reuses one sealed perception frame across decision candidates.
//! - [`replay`]: bounded observation/state traces with diagnostic and recorded-latency summaries.
//! - [`schedule`]: bounded event-driven actor scheduling with periodic refresh.
//! - [`sync`]: acknowledged context snapshots/deltas with strict reconstruction and resynchronization.
//! - [`manifest`]: a JSON description of a packet, used by the `gamectx` command.
//! - [`json`]: strict JSON values with exact number text, plus the [`obj!`] macro.
//!
//! The model only ever proposes. Packed context describes what exists; the game must
//! still validate every action the model asks for.
//!
//! ```
//! use gamectx::budget::{assemble_bytes, Section};
//! use gamectx::{json, table};
//!
//! let inventory = json::parse(r#"[{"id":"salt_pouch","count":12},{"id":"ash_bread","count":4}]"#, 1 << 20)?;
//! let packed = table::pack(inventory.as_array().unwrap(), &[])?;
//! let packet = assemble_bytes(
//!     vec![Section { name: "inventory".into(), priority: 5, stable: false, required: true, value: packed }],
//!     4096,
//! )?;
//! assert_eq!(packet.text, r#"{"inventory":{"columns":["id","count"],"rows":[["salt_pouch",12],["ash_bread",4]]}}"#);
//! # Ok::<(), gamectx::Error>(())
//! ```

pub mod archetype;
pub mod brain;
pub mod budget;
pub mod cast;
pub mod catalog;
pub mod character;
mod checked;
pub mod context;
pub mod decision;
pub mod deployment;
pub mod dialogue;
pub mod estimate;
pub mod facts;
pub mod formula;
pub mod info;
pub mod inventory;
pub mod json;
pub mod kit;
pub mod life;
pub mod living;
pub mod local;
pub mod location;
pub mod lore;
pub mod manifest;
pub mod memory;
pub mod observation;
pub mod outcome;
pub mod party;
pub mod playbook;
pub mod query;
pub mod recall;
pub mod replay;
pub mod request;
pub mod requests;
pub mod routine;
pub mod schedule;
pub mod schema;
pub mod selection;
pub mod signatures;
pub mod spending;
pub mod studio;
pub mod sync;
pub mod table;
pub mod temporal;
pub mod tune;
pub mod turn;
pub mod view;
pub mod visual;
pub mod world;

pub use context::{Context, Table, Turn};
pub use json::{Error, Result, Value};

/// 64-bit FNV-1a. Used to notice changed data, never for security.
pub fn fingerprint_u64(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// [`fingerprint_u64`] as 16 hex digits.
pub fn fingerprint(text: &str) -> String {
    format!("{:016x}", fingerprint_u64(text))
}

#[cfg(test)]
mod tests {
    #[test]
    fn fingerprint_matches_fnv1a_reference() {
        assert_eq!(super::fingerprint(""), "cbf29ce484222325");
        assert_eq!(super::fingerprint("a"), "af63dc4c8601ec8c");
    }
}
