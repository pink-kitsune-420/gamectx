//! One save for every stateful being manager (pass 5, design `O`).
//!
//! A bundle holds at most one manager per local kind (`chemistry`, `beliefs`, `space`, …) and
//! runs requests on them in place. `export` writes every present manager's own save (see
//! [`crate::data::persist`]) into one document:
//!
//! ```json
//! {"format":"gamectx-bundle","sections":[["beliefs",{…save…}],["space",{…save…}]],"version":1}
//! ```
//!
//! Sections are ascending by kind. `import` treats the document as untrusted: it checks the
//! envelope, then restores every section in full (types, ranges, canonical form, every
//! invariant the ops enforce, migrations of older section versions) into fresh managers, and
//! only when all of them pass does it replace the bundle. A refused import changes nothing.
//! `inspect` runs the same checks and reports what an import would load, without loading it.
//! Each manager keeps its current limits; the limits written in a save are only reported.
//!
//! Ops: `run {kind, request}`, `export {}`, `import {save}`, `inspect {save}`. Limits:
//! `sections` (how many managers one bundle may hold).

use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, Durable, MAX_SAVE_BYTES};
use crate::knowledge::codex::keys;
use crate::{obj, Result, Value};
use std::collections::BTreeMap;

/// The bundle document's `format`.
pub const FORMAT: &str = "gamectx-bundle";
/// The current bundle version.
pub const VERSION: u64 = 1;

/// Host limits: `sections` caps how many managers a bundle holds (and a save may carry).
pub const LIMITS: &[Limit] = &[Limit { name: "sections", default: 64, ceiling: 64 }];

/// One manager inside a bundle, whatever its kind.
pub trait Section {
    /// The manager's local kind.
    fn kind(&self) -> &'static str;
    /// Runs one workflow request on the manager.
    fn run(&mut self, request: &Value) -> Result<Value>;
    /// The manager's save under its own limits.
    fn save(&self) -> Value;
    /// A fresh manager of this kind restored from `save`, keeping this manager's limits, with
    /// the save's version and saved limits.
    fn restore(&self, save: &Value) -> Result<(Box<dyn Section>, u64, Value)>;
}

impl<T: Durable + 'static> Section for T {
    fn kind(&self) -> &'static str {
        T::KIND
    }

    fn run(&mut self, request: &Value) -> Result<Value> {
        Durable::run(self, request)
    }

    fn save(&self) -> Value {
        persist::export(self, self.limits())
    }

    fn restore(&self, save: &Value) -> Result<(Box<dyn Section>, u64, Value)> {
        let (m, from, saved) = persist::restore::<T>(save, self.limits())?;
        Ok((Box::new(m), from, saved))
    }
}

/// A blank manager of `kind`, or `None` for a kind that has no durable state.
pub fn blank(kind: &str) -> Option<Box<dyn Section>> {
    use crate::character as ch;
    use crate::knowledge as kn;
    fn new<T: Durable + 'static>() -> Option<Box<dyn Section>> {
        Some(Box::new(T::default()))
    }
    match kind {
        "aspiration" => new::<ch::aspiration::Aspirations>(),
        "attention" => new::<ch::attention::AttentionModel>(),
        "beliefs" => new::<ch::beliefs::BeliefStore>(),
        "chatter" => new::<ch::chatter::Chatter>(),
        "chemistry" => new::<ch::chemistry::Chemistry>(),
        "cognition" => new::<ch::cognition::Cognition>(),
        "cooperation" => new::<ch::cooperation::Cooperation>(),
        "creed" => new::<ch::creed::CreedManager>(),
        "cueing" => new::<ch::cueing::Cueing>(),
        "culture" => new::<kn::culture::Culture>(),
        "defenses" => new::<ch::defenses::Defenses>(),
        "detail" => new::<ch::detail::Detail>(),
        "diet" => new::<ch::diet::DietManager>(),
        "drift" => new::<ch::drift::Drift>(),
        "economy" => new::<ch::economy::Economy>(),
        "embodiment" => new::<ch::embodiment::Embodiment>(),
        "expectation" => new::<ch::expectation::ExpectationBook>(),
        "explain" => Some(Box::new(ch::explain::ExplainTracer::new())),
        "fabric" => new::<kn::fabric::Fabric>(),
        "genre" => new::<kn::genre::Genres>(),
        "habit" => new::<ch::habit::Habits>(),
        "lesson" => new::<ch::lesson::Lessons>(),
        "minds" => new::<ch::minds::Minds>(),
        "packs" => new::<ch::packs::Packs>(),
        "pragmatics" => new::<ch::pragmatics::Pragmatics>(),
        "projects" => new::<ch::projects::Projects>(),
        "retention" => new::<ch::retention::Retention>(),
        "roles" => new::<ch::roles::Roles>(),
        "space" => new::<ch::space::Space>(),
        "speech_act" => new::<ch::speech_act::SpeechActs>(),
        "stability" => Some(Box::new(ch::stability::StabilityMonitor::new())),
        "surroundings" => new::<kn::surroundings::Surroundings>(),
        "survival" => new::<ch::survival::Survival>(),
        "turn-frame" => new::<ch::frame::Frame>(),
        _ => None,
    }
}

/// Every durable kind, ascending.
pub const KINDS: [&str; 34] = [
    "aspiration",
    "attention",
    "beliefs",
    "chatter",
    "chemistry",
    "cognition",
    "cooperation",
    "creed",
    "cueing",
    "culture",
    "defenses",
    "detail",
    "diet",
    "drift",
    "economy",
    "embodiment",
    "expectation",
    "explain",
    "fabric",
    "genre",
    "habit",
    "lesson",
    "minds",
    "packs",
    "pragmatics",
    "projects",
    "retention",
    "roles",
    "space",
    "speech_act",
    "stability",
    "surroundings",
    "survival",
    "turn-frame",
];

/// Present managers by kind.
pub type Sections = BTreeMap<&'static str, Box<dyn Section>>;

/// Managers by kind, run and saved together.
pub struct StateBundle {
    /// Host-set limits (`{ops, limits?}` or [`StateBundle::with_limits`]).
    pub limits: Limits,
    /// Present managers by kind.
    pub sections: Sections,
}

impl Default for StateBundle {
    fn default() -> Self {
        StateBundle { limits: Limits::defaults(LIMITS), sections: BTreeMap::new() }
    }
}

impl StateBundle {
    pub fn new() -> Self {
        Self::default()
    }

    /// A bundle with host limits applied (`{sections?}`).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut b = Self::new();
        b.limits.apply(v)?;
        Ok(b)
    }

    /// Runs `{ops, limits?}`.
    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "run" => self.op_run(args, l),
            "export" => self.op_export(args),
            "import" => self.op_import(args, l, true),
            "inspect" => self.op_import(args, l, false),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn op_run(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["kind", "request"], &[])?;
        let kind = c::text(c::get(args, "kind")?)?;
        let fresh = blank(kind).ok_or_else(c::bad)?;
        let key = fresh.kind();
        if !self.sections.contains_key(key) {
            persist::need(self.sections.len() < l.get("sections"))?;
            self.sections.insert(key, fresh);
        }
        let section = self.sections.get_mut(key).ok_or_else(c::bad)?;
        section.run(c::get(args, "request")?)
    }

    /// The bundle save: every present manager's save, ascending by kind.
    pub fn export(&self) -> Value {
        let rows = self.sections.iter().map(|(k, s)| Value::Array(vec![Value::from(*k), s.save()]));
        obj! { "format" => FORMAT, "sections" => Value::Array(rows.collect()), "version" => VERSION }
    }

    fn op_export(&self, args: &Value) -> Result<Value> {
        keys(args, &[], &[])?;
        Ok(self.export())
    }

    /// Checks a bundle save in full and restores every section into fresh managers.
    fn restore(&self, save: &Value, l: &Limits) -> Result<(Sections, Value)> {
        persist::need(save.is_valid(MAX_SAVE_BYTES))?;
        persist::exact_fields(save, &["format", "sections", "version"])?;
        persist::need(c::text(c::get(save, "format")?)? == FORMAT)?;
        let from = c::uint(c::get(save, "version")?, 1, VERSION)?;
        let rows = c::list(c::get(save, "sections")?)?;
        persist::need(rows.len() <= l.get("sections"))?;
        let mut out = Sections::new();
        let mut report = Vec::new();
        let mut last: Option<&str> = None;
        for row in rows {
            let [kind, section] = c::list(row)? else { return Err(c::bad()) };
            let kind = c::text(kind)?;
            persist::need(last.is_none_or(|p| p < kind))?;
            last = Some(kind);
            let spare;
            let current: &dyn Section = match self.sections.get(kind) {
                Some(s) => s.as_ref(),
                None => {
                    spare = blank(kind).ok_or_else(c::bad)?;
                    spare.as_ref()
                }
            };
            let (m, version, saved) = current.restore(section)?;
            report.push(obj! { "from_version" => version, "kind" => kind, "limits" => saved });
            out.insert(m.kind(), m);
        }
        Ok((out, obj! { "from_version" => from, "sections" => Value::Array(report), "version" => VERSION }))
    }

    fn op_import(&mut self, args: &Value, l: &Limits, replace: bool) -> Result<Value> {
        keys(args, &["save"], &[])?;
        let (sections, report) = self.restore(c::get(args, "save")?, l)?;
        if replace {
            self.sections = sections;
        }
        Ok(report)
    }
}

/// Runs a fresh bundle on `{ops, limits?}` (the `state-bundle` local kind).
pub fn workflow(req: &Value) -> Result<Value> {
    StateBundle::new().workflow(req)
}
