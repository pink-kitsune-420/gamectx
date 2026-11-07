use super::*;
use std::sync::Arc;
impl Blueprint {
    fn require(&self, module: &str) -> Result<()> {
        if self.has(module) {
            Ok(())
        } else {
            Err(c::bad())
        }
    }
    /// Install only an already host-approved draft. The builder grants no knowledge.
    pub fn life(
        &self,
        scope: &str,
        actor: &str,
        participants: &Value,
        definition: Arc<crate::life::Definition>,
    ) -> Result<crate::life::Life> {
        self.require("life")?;
        crate::life::Life::accept(scope, actor, participants, definition, self.life_config()?)
    }
    pub fn routine(
        &self,
        scope: &str,
        actor: &str,
        participants: &Value,
        calendar: &str,
        routine: Arc<crate::routine::Routine>,
    ) -> Result<crate::routine::Runtime> {
        self.require("routine")?;
        crate::routine::Runtime::new(scope, actor, participants, calendar, self.routine_config()?, Some(routine))
    }
    pub fn initiative(&self, scope: &str, actor: &str, participants: &Value) -> Result<crate::life::Initiative> {
        self.require("initiative")?;
        crate::life::Initiative::new(scope, actor, participants, self.initiative_policy()?)
    }
    pub fn coordinator(&self, scope: &str, actor: &str, participants: &Value) -> Result<crate::life::Coordinator> {
        self.require("continuity")?;
        crate::life::Coordinator::new(scope, actor, participants)
    }
    pub fn run_brain(&self, brain: &crate::brain::Brain, inputs: &Value, allowed: &[&str]) -> Result<Value> {
        self.require("brain")?;
        brain.run(inputs, allowed)
    }
    /// Host-only stock rows, using the existing ownership/tag filters. Offer/action
    /// execution and transaction confirmation still belong to the inventory host.
    pub fn stock(&self, inventory: &crate::inventory::Inventory, owners: &Value, tags: &Value) -> Result<Value> {
        self.require("stock")?;
        Ok(inventory.rows(owners, tags)?.into())
    }
    pub fn evidence(&self, evidence: &crate::selection::Evidence, max_bytes: usize) -> Result<Value> {
        self.require("evidence")?;
        evidence.select(max_bytes)
    }
    pub fn promises(&self, interactions: &crate::dialogue::InteractionTools) -> Result<Value> {
        self.require("promises")?;
        Ok(interactions.accepted_promises().into())
    }
    /// Conservative call reservation for this loadout. A route names a host-pinned
    /// profile; constructing one never runs the profile. Ambient routes always cost 0.
    pub fn route(&self, name: &str, cost_micros: u64) -> Result<crate::party::ModelRoute> {
        crate::party::ModelRoute::from_value(
            &obj! {"name"=>name,"size"=>get(&self.value,"size")?.clone(),"calls"=>get(&self.value,"model_calls")?.clone(),"cost_micros"=>cost_micros},
        )
    }
    /// Rebuild actual context caps for the selected route. Richer context never enables
    /// extra modules, policies or tools. A route cannot exceed this loadout's call quota.
    pub fn for_route(&self, route: &crate::party::ModelRoute) -> Result<Self> {
        let r = route.to_value();
        if c::uint(get(&r, "calls")?, 0, 8)? > c::uint(get(&self.value, "model_calls")?, 0, 8)? {
            return Err(c::bad());
        }
        let size = crate::life::text(&r, "size")?;
        let mut selected = self.clone();
        let expanded = matches!(size, "8b" | "30b" | "70b");
        selected.caps = if size == "0b" {
            ContextCaps::new(0, 0, 0, 0, 0)?
        } else {
            ContextCaps::from_hints(ModelSize::parse(size)?.workload_hints(crate::deployment::Workload::Dialogue))?
        };
        if !selected.has("recall") {
            selected.caps.episodes = 0;
        }
        if !selected.has("life") {
            selected.caps.facets = 0;
            selected.caps.attitudes = 0;
        }
        let caps = selected.caps;
        set_field(&mut selected.value, "size", get(&r, "size")?.clone());
        set_field(&mut selected.value, "attention", Value::str(if expanded { "expanded" } else { "focused" }));
        set_field(&mut selected.value, "model_calls", get(&r, "calls")?.clone());
        set_field(
            &mut selected.value,
            "caps",
            obj! {"facts"=>caps.facts,"facets"=>caps.facets,"attitudes"=>caps.attitudes,"episodes"=>caps.episodes,"exchanges"=>caps.exchanges},
        );
        if !expanded {
            let tools = crate::life::names(get(&selected.value, "tools")?, 2)?
                .into_iter()
                .filter(|s| s != "life_change")
                .collect::<Vec<_>>();
            set_field(&mut selected.value, "tools", tools.into());
        }
        Ok(selected)
    }
}
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    c::get(v, k)
}
