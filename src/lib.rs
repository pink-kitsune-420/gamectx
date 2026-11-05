//! gamectx compiles game data and tool schemas into small, budgeted context packets for
//! AI-driven games.
//!
//! - [`table`]: uniform objects to a header plus rows, (lossless).
//! - [`schema`]: tool schemas to rows with shared `$defs` (lossless), or a one-line outline (lossy).
//! - [`view`]: sort, keep the top N, keep chosen columns and round numbers before packing.
//! - [`catalog`]: searchable, paged registries instead of whole-registry dumps.
//! - [`budget`]: stable-first assembly that drops low-priority sections to fit a budget.
//! - [`estimate`]: conservative token estimates calibrated from provider usage reports.
//! - [`manifest`]: a JSON description of a packet, used by the `gamectx` command.
//!
//! The model only ever proposes. Packed context describes what exists; the game must
//! still validate every action the model asks for.
//!
//! ```
//! use gamectx::budget::{assemble_bytes, Section};
//! use gamectx::{json, table};
//!
//! let inventory = json::parse(r#"[{"id":"salt_pouch","count":12},{"id":"ash_bread","count":4}]"#, 1 << 20)?;
//! let packed = table::pack(inventory.as_array().unwrap(), None)?;
//! let packet = assemble_bytes(
//!     vec![Section { name: "inventory".into(), priority: 5, stable: false, required: true, value: packed }],
//!     4096,
//! )?;
//! assert_eq!(packet.text, r#"{"inventory":{"columns":["id","count"],"rows":[["salt_pouch",12],["ash_bread",4]]}}"#);
//! # Ok::<(), gamectx::Error>(())
//! ```

pub mod budget;
pub mod catalog;
pub mod estimate;
pub mod json;
pub mod manifest;
pub mod schema;
pub mod table;
pub mod view;

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
