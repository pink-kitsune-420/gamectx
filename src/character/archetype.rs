//! Explicit, composable loadouts. Constructing one never loads a model, registers a
//! tool or executes a routine. Size is advisory; hosts pin real deployment identities.
use crate::{
    character::life::ContextCaps, checked as c, deploy::deployment::ModelSize, obj, packet::request::bounded, Result,
    Value,
};
mod runtime;
pub(crate) const MODULES: &[&str] =
    &["routine", "brain", "life", "stock", "lore", "evidence", "recall", "promises", "continuity", "initiative"];
#[derive(Clone, Copy, Debug)]
pub enum Archetype {
    Ambient,
    Service,
    Quest,
    Companion,
}
impl Archetype {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "ambient" => Ok(Self::Ambient),
            "service" => Ok(Self::Service),
            "quest" => Ok(Self::Quest),
            "companion" => Ok(Self::Companion),
            _ => Err(c::bad()),
        }
    }
    fn settings(self) -> (&'static str, &'static str, &'static [&'static str], usize) {
        match self {
            Self::Ambient => ("ambient", "0b", &["routine", "brain", "initiative"], 0),
            Self::Service => ("service", "3b", &["routine", "life", "stock", "lore"], 1),
            Self::Quest => ("quest", "3b", &["routine", "life", "lore", "evidence", "recall", "promises"], 1),
            Self::Companion => (
                "companion",
                "8b",
                &["routine", "life", "lore", "evidence", "recall", "promises", "continuity", "initiative"],
                2,
            ),
        }
    }
}
#[derive(Clone, Debug)]
pub struct BlueprintBuilder {
    value: Value,
}
impl BlueprintBuilder {
    pub fn new(preset: Archetype) -> Self {
        let (name, size, _, calls) = preset.settings();
        Self {
            value: obj! {"version"=>1,"preset"=>name,"size"=>size,"modules"=>obj!{},"policies"=>obj!{"balanced"=>false,"deception"=>false,"mistaken"=>false},"tools"=>Vec::<Value>::new(),"model_calls"=>calls},
        }
    }
    pub fn module(mut self, name: &str, enabled: bool) -> Result<Self> {
        if !MODULES.contains(&name) {
            return Err(c::bad());
        }
        set_field(self.value_field("modules"), name, enabled.into());
        Ok(self)
    }
    fn value_field(&mut self, k: &str) -> &mut Value {
        let Value::Object(fields) = &mut self.value else { unreachable!() };
        fields.iter_mut().find(|(n, _)| n == k).map(|(_, v)| v).expect("builder field")
    }
    /// `None` is a deterministic 0B loadout, not a new ModelSize variant.
    pub fn size(mut self, size: Option<ModelSize>) -> Self {
        *self.value_field("size") =
            size.map_or_else(|| Value::str("0b"), |s| s.guideline().get("size_class").expect("size").clone());
        self
    }
    pub fn policies(mut self, balanced: bool, deception: bool, mistaken: bool) -> Self {
        *self.value_field("policies") = obj! {"balanced"=>balanced,"deception"=>deception,"mistaken"=>mistaken};
        self
    }
    pub fn tools(mut self, tools: &[&str]) -> Self {
        *self.value_field("tools") = Value::strings(tools);
        self
    }
    pub fn model_calls(mut self, calls: usize) -> Self {
        *self.value_field("model_calls") = calls.into();
        self
    }
    pub fn build(self) -> Result<Blueprint> {
        Blueprint::from_value(&self.value)
    }
}
pub(crate) fn set_field(v: &mut Value, k: &str, x: Value) {
    if let Value::Object(fields) = v {
        if let Some((_, value)) = fields.iter_mut().find(|(n, _)| n == k) {
            *value = x;
        } else {
            fields.push((k.into(), x));
        }
    }
}
#[derive(Clone, Debug)]
pub struct Blueprint {
    value: Value,
    caps: ContextCaps,
}
impl Blueprint {
    pub fn preset(preset: Archetype) -> Result<Self> {
        BlueprintBuilder::new(preset).build()
    }
    pub fn from_value(a: &Value) -> Result<Self> {
        bounded(a, 8192)?;
        c::fields(a, &["version", "preset", "size", "modules", "policies", "tools", "model_calls"])?;
        c::uint(c::get(a, "version")?, 1, 1)?;
        let (_, _, enabled, _) = Archetype::parse(crate::character::life::text(a, "preset")?)?.settings();
        let mut modules = Value::Object(MODULES.iter().map(|k| ((*k).into(), enabled.contains(k).into())).collect());
        for (k, v) in c::get(a, "modules")?.as_object().ok_or_else(c::bad)? {
            if !MODULES.contains(&k.as_str()) {
                return Err(c::bad());
            }
            c::boolean(v)?;
            set_field(&mut modules, k, v.clone());
        }
        let m = |k| c::boolean(modules.get(k).expect("module")).expect("boolean");
        let policies = c::get(a, "policies")?;
        c::fields(policies, &["balanced", "deception", "mistaken"])?;
        let balanced = c::boolean(c::get(policies, "balanced")?)?;
        let deception = c::boolean(c::get(policies, "deception")?)?;
        let mistaken = c::boolean(c::get(policies, "mistaken")?)?;
        let tools = crate::character::life::names(c::get(a, "tools")?, 2)?;
        if tools.iter().any(|t| !matches!(t.as_str(), "life_change" | "life_recall")) {
            return Err(c::bad());
        }
        let calls = c::uint(c::get(a, "model_calls")?, 0, 8)?;
        let size = crate::character::life::text(a, "size")?;
        let nominal = if size == "0b" {
            None
        } else {
            let s = ModelSize::parse(size)?;
            if s.guideline().get("size_class").and_then(Value::as_str) != Some(size) {
                return Err(c::bad());
            }
            Some(s)
        };
        let expanded = matches!(nominal, Some(ModelSize::B8 | ModelSize::B30 | ModelSize::B70));
        if !m("life")
            && (["recall", "promises", "continuity", "evidence"].iter().any(|k| m(k))
                || balanced
                || deception
                || mistaken
                || !tools.is_empty())
            || tools.iter().any(|t| t == "life_recall") && !m("recall")
            || tools.iter().any(|t| t == "life_change") && !m("continuity")
            || !expanded && (balanced || deception || tools.iter().any(|t| t == "life_change"))
            || nominal.is_none() && (calls != 0 || !tools.is_empty() || balanced || deception || mistaken)
        {
            return Err(c::bad());
        }
        let caps = if let Some(s) = nominal {
            let mut caps = ContextCaps::from_hints(s.workload_hints(crate::deploy::deployment::Workload::Dialogue))?;
            if !m("recall") {
                caps.episodes = 0;
            }
            if !m("life") {
                caps.facets = 0;
                caps.attitudes = 0;
            }
            caps
        } else {
            ContextCaps::new(0, 0, 0, 0, 0)?
        };
        let life_config = obj! {"enabled"=>m("life"),"balanced"=>balanced,"deception"=>deception,"mistaken"=>mistaken,"tools"=>tools.clone()};
        let routine_config = obj! {"routines"=>m("routine"),"model_calls"=>0};
        let initiative_config = obj! {"enabled"=>m("initiative"),"model_calls"=>0};
        crate::character::life::Config::resolve(&life_config, &obj! {})?;
        crate::character::routine::Config::resolve(&routine_config, &obj! {})?;
        crate::character::life::InitiativePolicy::resolve(&initiative_config, &obj! {})?;
        let value = obj! {"version"=>1,"preset"=>c::get(a,"preset")?.clone(),"size"=>size,"modules"=>modules,"policies"=>policies.clone(),"tools"=>tools,"model_calls"=>calls,"attention"=>if expanded{"expanded"}else{"focused"},"caps"=>obj!{"facts"=>caps.facts,"facets"=>caps.facets,"attitudes"=>caps.attitudes,"episodes"=>caps.episodes,"exchanges"=>caps.exchanges},"life_config"=>life_config,"routine_config"=>routine_config,"initiative_config"=>initiative_config};
        Ok(Self { value, caps })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    pub fn caps(&self) -> ContextCaps {
        self.caps
    }
    pub fn has(&self, module: &str) -> bool {
        self.value.get("modules").and_then(|m| m.get(module)).and_then(Value::as_bool) == Some(true)
    }
    pub fn life_config(&self) -> Result<crate::character::life::Config> {
        crate::character::life::Config::resolve(c::get(&self.value, "life_config")?, &obj! {})
    }
    pub fn routine_config(&self) -> Result<crate::character::routine::Config> {
        crate::character::routine::Config::resolve(c::get(&self.value, "routine_config")?, &obj! {})
    }
    pub fn initiative_policy(&self) -> Result<crate::character::life::InitiativePolicy> {
        crate::character::life::InitiativePolicy::resolve(c::get(&self.value, "initiative_config")?, &obj! {})
    }
    pub(crate) fn value(&self) -> &Value {
        &self.value
    }
}
