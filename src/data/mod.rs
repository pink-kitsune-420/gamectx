//! Data encoding and exact computation: strict JSON, lossless tables, row views, safe
//! formulas, `when` conditions, searchable catalogs and the bounded model-written SQL query engine.
pub mod catalog;
pub mod conditions;
pub mod formula;
pub mod json;
pub mod persist;
pub mod query;
pub mod table;
pub mod view;
