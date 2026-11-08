//! gamectx compiles game data, tool schemas and character state into small, budgeted context
//! packets for AI-driven games. The model only ever proposes; the game validates and executes.
//!
//! Start with [`Context`]: register tools and prompts once, then add variables and tables
//! each turn. The domain modules below are the building blocks it uses.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`data`] | strict [`json`](data::json), lossless [`table`](data::table)s, [`view`](data::view)s, safe [`formula`](data::formula)s, paged [`catalog`](data::catalog)s and the bounded model-written SQL [`query`](data::query) engine |
//! | [`packet`] | the [`Context`] builder, stable-first [`budget`](packet::budget) assembly, [`manifest`](packet::manifest)s, tool [`schema`](packet::schema)/[`signatures`](packet::signatures), full-[`request`](packet::request) accounting, token [`estimate`](packet::estimate)s and acknowledged [`sync`](packet::sync) |
//! | [`knowledge`] | [`memory`](knowledge::memory), [`recall`](knowledge::recall) indexes, typed [`facts`](knowledge::facts), authored [`lore`](knowledge::lore), attributed [`info`](knowledge::info) notes, [`inventory`](knowledge::inventory), [`observation`](knowledge::observation)s, evidence [`selection`](knowledge::selection), [`location`](knowledge::location), [`world`](knowledge::world) and [`temporal`](knowledge::temporal) windows |
//! | [`character`] | scoped character [`tools`](character::tools), persistent [`life`](character::life), [`dialogue`](character::dialogue), [`routine`](character::routine)s, [`party`](character::party) scenes, [`archetype`](character::archetype) loadouts, [`cast`](character::cast) packs, the [`living`](character::living) world and opt-in [`roleplay`](character::roleplay) policies |
//! | [`runtime`] | reaction [`brain`](runtime::brain)s, legal-choice [`decision`](runtime::decision)s, [`schedule`](runtime::schedule)s, [`playbook`](runtime::playbook) procedures, [`outcome`](runtime::outcome) labels, [`replay`](runtime::replay), one-use [`requests`](runtime::requests) and [`spending`](runtime::spending) |
//! | [`deploy`] | identity-pinned [`deployment`](deploy::deployment) profiles, scene-driven [`model_policy`](deploy::model_policy), recorded-configuration [`tune`](deploy::tune) and complete-[`turn`](deploy::turn) telemetry |
//! | [`media`] | [`visual`](media::visual) specs and asset registry, [`voice`](media::voice) delivery and [`providers`](media::providers) backend routing |
//! | [`kit`] | shared registration, named recipes, validated callbacks, inspection and host bridges |
//!
//! ```
//! use gamectx::data::{json, table};
//! use gamectx::packet::budget::{assemble_bytes, Section};
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

pub mod character;
mod checked;
pub mod data;
pub mod deploy;
pub mod kit;
pub mod knowledge;
pub mod media;
pub mod packet;
pub mod runtime;

pub use data::json::{Error, Result, Value};
pub use packet::context::{Context, Table, Turn};

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
