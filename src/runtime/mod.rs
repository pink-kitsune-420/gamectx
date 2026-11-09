//! Bounded decision runtime: reaction brains, legal-choice decisions, scheduling, procedures,
//! outcome labels, replay, one-use request tickets and microdollar spending.
pub mod being;
pub mod brain;
pub mod decision;
pub mod outcome;
pub mod playbook;
pub mod replay;
pub mod requests;
pub mod schedule;
pub mod spending;

pub use being::BeingRuntime;
