//! Supplemental system instructions selected explicitly by the host.
use crate::{Error, Result};

/// Optional prompt guidance, chosen per model/workload by the host.
/// Standard adds no text to the existing tool description. ExplicitSelection adds the
/// exact system paragraph measured on six fresh synthetic states: 3B 46/48→48/48,
/// 24B 47/48→46/48, with higher input use. It is not an automatic accuracy guarantee.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Guidance {
    #[default]
    Standard,
    ExplicitSelection,
}
impl Guidance {
    /// Reads `standard` or `explicit-selection`; other names are errors.
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "standard" => Ok(Self::Standard),
            "explicit-selection" => Ok(Self::ExplicitSelection),
            _ => Err(Error::new("query guidance must be standard or explicit-selection")),
        }
    }
    /// Supplemental text to append once to the system prompt, beside the query table
    /// description. Standard is empty; existing tool/session formats are unaffected.
    /// Keep the same profile for the correction and final-answer turns.
    pub fn text(self) -> &'static str {
        match self {
            Self::Standard => "",
            Self::ExplicitSelection => GUIDANCE,
        }
    }
}
const GUIDANCE: &str = "Return all matching rows for a list: do not add LIMIT unless the question requests a top-N or one row. Use only filters requested by the question; field meanings do not add filters. For a top-N total use SELECT SUM(column) FROM table ORDER BY column DESC LIMIT N, substituting listed names. Never put SELECT inside SELECT or IN. Do not add eligibility filters to an all-row total.";
