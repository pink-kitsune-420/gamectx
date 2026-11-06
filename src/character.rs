//! Provider-independent character tools and task-focused full-request selection.
//! Snapshots, capabilities and token counts come from the host; model size is no ceiling.
use crate::{
    checked as c,
    dialogue::InteractionTools,
    info::Notes,
    inventory::Inventory,
    location::Locations,
    obj,
    request::{bounded, BudgetedRequest, Count, RequestBudget},
    world::WorldState,
    Result, Value,
};
use std::sync::Arc;
#[derive(Clone, Debug)]
pub struct CharacterView {
    pub revision: u32,
    world: Option<(Arc<WorldState>, Vec<String>)>,
    location: Option<(Arc<Locations>, String)>,
    inventory: Option<(Arc<Inventory>, Vec<String>)>,
    sources: Value,
}
fn names(values: &[&str], max: usize) -> Result<Vec<String>> {
    if values.len() > max {
        return Err(c::bad());
    }
    let mut out = Vec::new();
    for &v in values {
        c::scope(v)?;
        if out.iter().any(|old| old == v) {
            return Err(c::bad());
        }
        out.push(v.into());
    }
    Ok(out)
}
impl CharacterView {
    pub fn new(revision: u32) -> Self {
        Self { revision, world: None, location: None, inventory: None, sources: Vec::<Value>::new().into() }
    }
    pub fn with_world(mut self, world: impl Into<Arc<WorldState>>, keys: &[&str]) -> Result<Self> {
        let world = world.into();
        if world.revision() != self.revision {
            return Err(c::bad());
        }
        self.world = Some((world, names(keys, 64)?));
        Ok(self)
    }
    /// Scene details must already be redacted for this actor. The model cannot choose
    /// another location; the current ID is supplied here by the game.
    pub fn with_location(mut self, locations: impl Into<Arc<Locations>>, current: &str) -> Result<Self> {
        c::scope(current)?;
        let locations = locations.into();
        if locations.revision() != self.revision {
            return Err(c::bad());
        }
        self.location = Some((locations, current.into()));
        Ok(self)
    }
    pub fn with_inventory(mut self, inventory: impl Into<Arc<Inventory>>, owners: &[&str]) -> Result<Self> {
        self.inventory = Some((inventory.into(), names(owners, 32)?));
        Ok(self)
    }
    pub fn with_sources(mut self, sources: &Value) -> Result<Self> {
        crate::info::sources(sources)?;
        self.sources = sources.clone();
        Ok(self)
    }
}
#[derive(Clone, Debug)]
pub struct CharacterTools {
    notes: Notes,
    interactions: InteractionTools,
    view: Option<CharacterView>,
    enabled: Vec<String>,
    world_id: Option<String>,
    routine: Option<Box<crate::routine::Runtime>>,
}
impl CharacterTools {
    pub fn new(scope: &str, actor: &str, participants: &Value) -> Result<Self> {
        Ok(Self {
            notes: Notes::new(scope)?,
            interactions: InteractionTools::new(scope, actor, participants)?,
            view: None,
            enabled: Vec::new(),
            world_id: None,
            routine: None,
        })
    }
    /// Bind one fresh actor view; changing a world ID requires clearing the character.
    /// Inventory may have its own revision stream; the host binds its freshness to this frame.
    pub fn bind(&mut self, view: CharacterView) -> Result<()> {
        if let Some((world, _)) = &view.world {
            let id = world.id();
            if self.world_id.as_ref().is_some_and(|old| old != id) {
                return Err(c::bad());
            }
            self.world_id = Some(id.into());
        }
        self.view = Some(view);
        self.enabled.clear();
        Ok(())
    }
    fn schema(&self, name: &str) -> Result<Value> {
        let view = self.view.as_ref().ok_or_else(c::bad)?;
        match name {
            "routine_status" | "activity_propose" => {
                let runtime = self.routine.as_ref().ok_or_else(c::bad)?;
                if runtime.revision() != Some(view.revision) {
                    return Err(c::bad());
                }
                runtime.schema(name)
            }
            "location_lookup" => Ok(view.location.as_ref().ok_or_else(c::bad)?.0.tool()),
            "look_around" => {
                let mut tool = view.location.as_ref().ok_or_else(c::bad)?.0.tool();
                if let Value::Object(root) = &mut tool {
                    if let Some((_, Value::Object(fields))) = root.iter_mut().find(|(k, _)| k == "function") {
                        for (k, v) in fields {
                            if k == "name" {
                                *v = Value::str("look_around");
                            } else if k == "description" {
                                *v=Value::str("Read the latest host-observed surroundings of this character. Only visible objects, people and observable weather may be included; this grants no remote vision.");
                            }
                        }
                    }
                }
                Ok(tool)
            }
            "world_status" => Ok(view.world.as_ref().ok_or_else(c::bad)?.0.tool()),
            "inventory_lookup" => Ok(view.inventory.as_ref().ok_or_else(c::bad)?.0.lookup_tool()),
            "info_lookup" | "info_remember" | "info_forget" => self
                .notes
                .tools()
                .as_array()
                .expect("tools")
                .iter()
                .find(|t| t.get("function").and_then(|v| v.get("name")).and_then(Value::as_str) == Some(name))
                .cloned()
                .ok_or_else(c::bad),
            _ => Ok(self.interactions.tools(&[name])?.as_array().expect("tools")[0].clone()),
        }
    }
    /// Register only the schemas selected for this turn. Calls to other tools fail.
    pub fn enable(&mut self, tools: &[&str]) -> Result<Value> {
        let selected = names(tools, 32)?;
        let schemas = selected.iter().map(|n| self.schema(n)).collect::<Result<Vec<_>>>()?;
        self.enabled = selected;
        Ok(schemas.into())
    }
    pub fn enabled(&self) -> &[String] {
        &self.enabled
    }
    pub fn interactions_mut(&mut self) -> &mut InteractionTools {
        &mut self.interactions
    }
    pub fn call(
        &mut self,
        name: &str,
        input: &str,
        revision: u32,
        max_bytes: usize,
        available: impl FnMut(&Value) -> bool,
    ) -> Result<Value> {
        let view = self.view.as_ref().ok_or_else(c::bad)?;
        if view.revision != revision || !self.enabled.iter().any(|n| n == name) || !(64..=8192).contains(&max_bytes) {
            return Err(c::bad());
        }
        let result = match name {
            "routine_status" | "activity_propose" => {
                self.routine.as_ref().ok_or_else(c::bad)?.call(name, input, revision, max_bytes, available)?
            }
            "world_status" => {
                let (world, keys) = view.world.as_ref().ok_or_else(c::bad)?;
                world.call(input, revision, max_bytes, |k| keys.iter().any(|x| x == k))?
            }
            "location_lookup" | "look_around" => {
                let (locations, current) = view.location.as_ref().ok_or_else(c::bad)?;
                locations.call(input, current, revision, max_bytes, |_| true)?
            }
            "inventory_lookup" => {
                let (inventory, owners) = view.inventory.as_ref().ok_or_else(c::bad)?;
                inventory.lookup_call(input, &owners.clone().into())?
            }
            "info_lookup" => self.notes.call(name, input, &view.sources)?,
            "info_remember" | "info_forget" => {
                let mut draft = self.notes.clone();
                let result = draft.call(name, input, &view.sources)?;
                bounded(&result, max_bytes)?;
                self.notes = draft;
                result
            }
            _ => self.interactions.call(name, input, &view.sources, revision, max_bytes, available)?,
        };
        bounded(&result, max_bytes)?;
        Ok(result)
    }
    /// Attach a host-configured optional runtime. Scope/actor are not model inputs.
    pub fn with_runtime(&mut self, runtime: crate::routine::Runtime) -> Result<()> {
        let (scope, actor, participants) = self.interactions.identity();
        if !runtime.identity_matches(scope, actor, participants) {
            return Err(c::bad());
        }
        self.disable();
        self.routine = Some(Box::new(runtime));
        Ok(())
    }
    pub fn configure_routine(
        &mut self,
        calendar: &str,
        config: crate::routine::Config,
        definition: Option<Arc<crate::routine::Routine>>,
    ) -> Result<()> {
        let (scope, actor, participants) = self.interactions.identity();
        let runtime =
            crate::routine::Runtime::new(scope, actor, &Value::strings(participants), calendar, config, definition)?;
        self.with_runtime(runtime)
    }
    /// Reserve one provider call without changing a counted/exposed request.
    pub fn reserve_routine_call(&mut self, revision: u32) -> Result<()> {
        self.routine.as_mut().ok_or_else(c::bad)?.reserve_model_call(revision)
    }
    pub fn routine(&self) -> Option<&crate::routine::Runtime> {
        self.routine.as_deref()
    }
    /// Host updates invalidate exposed tools. Rebind/reassemble after any change.
    pub fn routine_mut(&mut self) -> Result<&mut crate::routine::Runtime> {
        self.disable();
        self.routine.as_deref_mut().ok_or_else(c::bad)
    }
    pub fn clear(&mut self) {
        self.notes.clear();
        self.interactions.clear();
        self.view = None;
        self.enabled.clear();
        self.world_id = None;
        if let Some(runtime) = &mut self.routine {
            runtime.clear();
        }
    }
}
/// Cognitive limits are independent of context capacity. No provider/model-size table
/// is assumed: a host can use any model ID and explicit measured capabilities.
#[derive(Clone, Copy, Debug)]
pub struct AttentionLimits {
    pub tools: usize,
    pub memories: usize,
    pub exchanges: usize,
}
impl AttentionLimits {
    pub fn focused() -> Self {
        Self { tools: 1, memories: 2, exchanges: 1 }
    }
    pub fn new(tools: usize, memories: usize, exchanges: usize) -> Result<Self> {
        if tools > 32 || memories > 64 || exchanges > 16 {
            return Err(c::bad());
        }
        Ok(Self { tools, memories, exchanges })
    }
}
#[derive(Clone, Debug)]
pub struct ToolChoice {
    pub name: String,
    pub task: String,
    pub priority: i32,
    pub required: bool,
}
#[derive(Clone, Debug)]
pub struct MemoryChoice {
    pub value: Value,
    pub priority: i32,
    pub required: bool,
}
#[derive(Clone, Debug)]
pub struct TurnPlan {
    pub task: String,
    pub input: String,
    pub facts: Value,
    pub tools: Vec<ToolChoice>,
    pub memories: Vec<MemoryChoice>,
    pub dialogue: Vec<Value>,
    pub attention: AttentionLimits,
}
fn picks(rows: &[(i32, bool)], cap: usize) -> Result<Vec<usize>> {
    let mut selected = rows.iter().enumerate().filter(|(_, r)| r.1).map(|(i, _)| i).collect::<Vec<_>>();
    if selected.len() > cap {
        return Err(c::bad());
    }
    let mut optional =
        rows.iter().enumerate().filter(|(_, r)| !r.1).map(|(i, r)| (std::cmp::Reverse(r.0), i)).collect::<Vec<_>>();
    optional.sort();
    selected.extend(optional.into_iter().take(cap - selected.len()).map(|(_, i)| i));
    selected.sort();
    Ok(selected)
}
impl CharacterTools {
    /// Count the complete provider-rendered request, with native tools, templates,
    /// history and output/safety reserves. Protected facts, required tools/memories and
    /// the last whole exchange never disappear. More relevant optional data fits as the
    /// host raises capacity; attention limits can still constrain a small model.
    pub fn assemble(
        &mut self,
        plan: &TurnPlan,
        budget: &RequestBudget,
        mut render: impl FnMut(&Value) -> Result<Value>,
        count: impl FnMut(&Value) -> Result<Count>,
    ) -> Result<BudgetedRequest> {
        self.disable();
        let (sections, tools) = self.sections(plan)?;
        let result = budget.assemble(sections, |p| render(&frame(p)?), count)?;
        self.select_enabled(&result.packet.value, &tools);
        Ok(result)
    }
    pub(crate) fn sections(&self, plan: &TurnPlan) -> Result<(Vec<crate::budget::Section>, Vec<String>)> {
        use crate::budget::Section;
        c::scope(&plan.task)?;
        if plan.input.is_empty()
            || plan.input.len() > 4096
            || plan.tools.len() > 64
            || plan.memories.len() > 128
            || plan.dialogue.len() > 128
        {
            return Err(c::bad());
        }
        AttentionLimits::new(plan.attention.tools, plan.attention.memories, plan.attention.exchanges)?;
        let mut tools = Vec::new();
        for t in &plan.tools {
            c::scope(&t.name)?;
            c::scope(&t.task)?;
            if !(-1_000_000..=1_000_000).contains(&t.priority) {
                return Err(c::bad());
            }
            if t.task != plan.task {
                if t.required {
                    return Err(c::bad());
                }
                continue;
            }
            if tools.iter().any(|old: &&ToolChoice| old.name == t.name) {
                return Err(c::bad());
            }
            tools.push(t);
        }
        let mut sections = vec![Section {
            name: "base".into(),
            priority: 1_000_000,
            stable: false,
            required: true,
            value: obj! {"task"=>plan.task.clone(),"input"=>plan.input.clone(),"facts"=>plan.facts.clone()},
        }];
        for i in picks(&tools.iter().map(|t| (t.priority, t.required)).collect::<Vec<_>>(), plan.attention.tools)? {
            let t = tools[i];
            sections.push(Section {
                name: format!("tool_{i}"),
                priority: i64::from(t.priority),
                stable: false,
                required: t.required,
                value: self.schema(&t.name)?,
            });
        }
        for m in &plan.memories {
            if !(-1_000_000..=1_000_000).contains(&m.priority) {
                return Err(c::bad());
            }
            bounded(&m.value, 32768)?;
        }
        for i in
            picks(&plan.memories.iter().map(|m| (m.priority, m.required)).collect::<Vec<_>>(), plan.attention.memories)?
        {
            let m = &plan.memories[i];
            sections.push(Section {
                name: format!("memory_{i}"),
                priority: i64::from(m.priority),
                stable: false,
                required: m.required,
                value: m.value.clone(),
            });
        }
        let n = plan.dialogue.len().min(plan.attention.exchanges);
        for (i, e) in plan.dialogue.iter().enumerate() {
            c::fields(e, &["user", "assistant"])?;
            for k in ["user", "assistant"] {
                if c::text(c::get(e, k)?)?.len() > 4096 {
                    return Err(c::bad());
                }
            }
            if i >= plan.dialogue.len() - n {
                sections.push(Section {
                    name: format!("exchange_{i}"),
                    priority: 100 + i as i64,
                    stable: false,
                    required: i == plan.dialogue.len() - 1,
                    value: e.clone(),
                });
            }
        }
        Ok((sections, tools.iter().map(|t| t.name.clone()).collect()))
    }
    pub(crate) fn select_enabled(&mut self, packet: &Value, tools: &[String]) {
        self.enabled = tools
            .iter()
            .enumerate()
            .filter(|(i, _)| packet.get(&format!("tool_{i}")).is_some())
            .map(|(_, t)| t.clone())
            .collect();
    }
    pub(crate) fn disable(&mut self) {
        self.enabled.clear();
    }
    pub fn revision(&self) -> Option<u32> {
        self.view.as_ref().map(|v| v.revision)
    }
}
impl CharacterView {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, crate::request::MAX_REQUEST_BYTES)?;
        c::fields(v, &["revision", "world", "world_keys", "locations", "current", "inventory", "owners", "sources"])?;
        let mut view = Self::new(c::uint(c::get(v, "revision")?, 0, u32::MAX as u64)? as u32);
        let strings = |key: &str, max: usize| -> Result<Vec<String>> {
            let rows = c::list(c::get(v, key)?)?;
            let refs = rows.iter().map(c::text).collect::<Result<Vec<_>>>()?;
            names(&refs, max)
        };
        let keys = strings("world_keys", 64)?;
        let owners = strings("owners", 32)?;
        if c::get(v, "world")? != &Value::Null {
            view = view.with_world(
                WorldState::from_value(c::get(v, "world")?)?,
                &keys.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
        }
        if c::get(v, "locations")? != &Value::Null {
            view =
                view.with_location(Locations::from_value(c::get(v, "locations")?)?, c::text(c::get(v, "current")?)?)?;
        } else if c::get(v, "current")? != &Value::Null {
            return Err(c::bad());
        }
        if c::get(v, "inventory")? != &Value::Null {
            view = view.with_inventory(
                Inventory::from_value(c::get(v, "inventory")?)?,
                &owners.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
        }
        view.with_sources(c::get(v, "sources")?)
    }
}

/// Neutral context frame for any provider renderer. Tool schemas remain ordinary
/// function definitions; adapters may translate them to their provider's transport.
pub fn frame(packet: &Value) -> Result<Value> {
    let fields = packet.as_object().ok_or_else(c::bad)?;
    let base = c::get(packet, "base")?;
    let group =
        |prefix: &str| fields.iter().filter(|(k, _)| k.starts_with(prefix)).map(|(_, v)| v.clone()).collect::<Vec<_>>();
    Ok(
        obj! {"task"=>c::get(base,"task")?.clone(),"input"=>c::get(base,"input")?.clone(),"facts"=>c::get(base,"facts")?.clone(),"tools"=>group("tool_"),"memories"=>group("memory_"),"dialogue"=>group("exchange_")},
    )
}
impl TurnPlan {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, crate::request::MAX_REQUEST_BYTES)?;
        c::fields(v, &["task", "input", "facts", "tools", "memories", "dialogue", "attention"])?;
        let priority = |v: &Value| -> Result<i32> {
            let p = c::get(v, "priority")?.to_json().parse::<i32>().map_err(|_| c::bad())?;
            if !(-1_000_000..=1_000_000).contains(&p) {
                return Err(c::bad());
            }
            Ok(p)
        };
        let mut tools = Vec::new();
        let rows = c::list(c::get(v, "tools")?)?;
        if rows.len() > 64 {
            return Err(c::bad());
        }
        for t in rows {
            c::fields(t, &["name", "task", "priority", "required"])?;
            tools.push(ToolChoice {
                name: c::text(c::get(t, "name")?)?.into(),
                task: c::text(c::get(t, "task")?)?.into(),
                priority: priority(t)?,
                required: c::boolean(c::get(t, "required")?)?,
            });
        }
        let mut memories = Vec::new();
        let rows = c::list(c::get(v, "memories")?)?;
        if rows.len() > 128 {
            return Err(c::bad());
        }
        for m in rows {
            c::fields(m, &["value", "priority", "required"])?;
            memories.push(MemoryChoice {
                value: c::get(m, "value")?.clone(),
                priority: priority(m)?,
                required: c::boolean(c::get(m, "required")?)?,
            });
        }
        let attention = c::get(v, "attention")?;
        c::fields(attention, &["tools", "memories", "exchanges"])?;
        Ok(Self {
            task: c::text(c::get(v, "task")?)?.into(),
            input: c::text(c::get(v, "input")?)?.into(),
            facts: c::get(v, "facts")?.clone(),
            tools,
            memories,
            dialogue: c::list(c::get(v, "dialogue")?)?.to_vec(),
            attention: AttentionLimits::new(
                c::uint(c::get(attention, "tools")?, 0, 32)? as usize,
                c::uint(c::get(attention, "memories")?, 0, 64)? as usize,
                c::uint(c::get(attention, "exchanges")?, 0, 16)? as usize,
            )?,
        })
    }
}
/// Offline conformance adapter. Its identity renderer/one-byte counter are synthetic
/// fixtures, never provider tokenizers; use `assemble` or session count acknowledgements.
pub fn plan_fixture(a: &Value) -> Result<Value> {
    c::fields(a, &["scope", "actor", "participants", "view", "turn", "budget"])?;
    let mut character =
        CharacterTools::new(c::text(c::get(a, "scope")?)?, c::text(c::get(a, "actor")?)?, c::get(a, "participants")?)?;
    character.bind(CharacterView::from_value(c::get(a, "view")?)?)?;
    let turn = TurnPlan::from_value(c::get(a, "turn")?)?;
    let budget = RequestBudget::from_value(c::get(a, "budget")?)?;
    let result = character.assemble(
        &turn,
        &budget,
        |frame| Ok(frame.clone()),
        |v| Ok(Count { input: v.json_len(), method: crate::request::CountMethod::Estimate }),
    )?;
    Ok(
        obj! {"request"=>result.request,"selected_tools"=>character.enabled.clone(),"input_bytes"=>result.receipt.input,"omitted"=>result.packet.receipt.dropped.iter().map(|line| line.name.clone()).collect::<Vec<_>>()},
    )
}
