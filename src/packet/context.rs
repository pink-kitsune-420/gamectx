//! The easy way to use gamectx from code: register what stays the same once, then set
//! what changes each turn.
//!
//! ```
//! use gamectx::{obj, Context, Table};
//!
//! let tools = gamectx::data::json::parse(r#"[{"type":"function","function":{"name":"wait",
//!     "description":"Do nothing for a while.","strict":true,"parameters":{"type":"object",
//!     "properties":{"seconds":{"type":"integer","minimum":1,"maximum":30}},
//!     "required":["seconds"],"additionalProperties":false}}}]"#, 1 << 20)?;
//!
//! // Once, at startup: stable sections are packed now and reused every turn.
//! let mut game = Context::new().budget_bytes(4000);
//! game.tools("actions", tools)?;
//! game.prompt("rules", "Ilse guards the keep. The game validates every action.")?;
//!
//! // Every turn:
//! let mut turn = game.turn();
//! turn.var("mood", "wary")?;
//! turn.table("inventory", vec![
//!     obj! {"id" => "salt_pouch", "kind" => "reagent", "count" => 12},
//!     obj! {"id" => "ember_lantern", "kind" => "tool", "count" => 1},
//! ], Table::new().group_by("kind").priority(6))?;
//! let packet = turn.build()?;
//!
//! assert!(packet.text.starts_with(r#"{"actions":"All arguments required"#));
//! let system_prompt = packet.system_prompt("You are the game logic for Warden Ilse.");
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::data::catalog::Catalog;
use crate::data::formula::Formulas;
use crate::data::json::{Error, Result, Value};
use crate::data::view::{self, Op, View};
use crate::knowledge::memory::Memory;
use crate::packet::budget::{assemble_prepared, assemble_prepared_bytes, Packet, PreparedSection, Section};
use crate::packet::estimate::{self, Calibration};
use crate::{data::table, packet::schema, packet::signatures};

/// Registered sections and budgets shared by every turn.
#[derive(Clone, Debug, Default)]
pub struct Context {
    prepared: Vec<PreparedSection>,
    formulas: Formulas,
    budget_bytes: Option<usize>,
    budget_tokens: Option<usize>,
    calibration: Calibration,
    scope: u64,
}

/// Options for a table section. Every setting is optional.
#[derive(Clone, Debug, Default)]
pub struct Table {
    group_by: Vec<String>,
    view: View,
    priority: Option<i64>,
    required: bool,
    /// A setting that could not be applied, reported when the table is added.
    error: Option<Error>,
}

impl Table {
    pub fn new() -> Table {
        Table::default()
    }

    /// Group rows under this column's values (it must hold strings). Call again to nest
    /// groups: `.group_by("kind").group_by("rarity")` puts rarity groups inside kind groups.
    /// Group by the columns your game filters on most.
    pub fn group_by(mut self, column: &str) -> Table {
        self.group_by.push(column.to_owned());
        self
    }

    /// Add a column computed from each row by a formula expression, e.g.
    /// `.derive("danger", "danger(hp, dist)")`. Runs before every other step, so `filter`
    /// and `sort_by` can use it. Rows whose inputs are missing or not numbers get `null`.
    pub fn derive(mut self, column: &str, expression: &str) -> Table {
        self.view.derive.push((column.to_owned(), expression.to_owned()));
        self
    }

    /// Keep only rows where `column` compares to `value` with `op`: `=`, `!=`, `>`, `>=`,
    /// `<`, `<=` or `in` (value is a list). Call again to require more conditions. This is
    /// how to do numeric filtering, which models get wrong when asked to do it themselves.
    pub fn filter(mut self, column: &str, op: &str, value: impl Into<Value>) -> Table {
        match Op::parse(op) {
            Ok(op) => self.view.filters.push((column.to_owned(), op, value.into())),
            Err(e) => self.error = Some(e),
        }
        self
    }

    /// Keep only the rows that best match the words of `text`, such as the player's message
    /// or the current event; if nothing matches, every row is kept. See
    /// [`crate::data::view::relevant_rows`].
    pub fn relevant_to(mut self, text: &str) -> Table {
        self.view.relevant_to = Some(text.to_owned());
        self
    }

    /// Like [`Table::relevant_to`], but sends no rows when nothing matches. Saves the whole
    /// table on unrelated turns, at the cost of questions about the table as a whole
    /// ("who has the highest trust?"), which then see nothing.
    pub fn relevant_only(mut self, text: &str) -> Table {
        self.view.relevant_to = Some(text.to_owned());
        self.view.relevant_only = true;
        self
    }

    /// Sort rows by a column; prefix `-` for descending, e.g. `"-count"`.
    pub fn sort_by(mut self, column: &str) -> Table {
        let (column, descending) = match column.strip_prefix('-') {
            Some(rest) => (rest, true),
            None => (column, false),
        };
        self.view.sort_by = Some((column.to_owned(), descending));
        self
    }

    /// Keep at most this many rows, after sorting.
    pub fn limit(mut self, rows: usize) -> Table {
        self.view.limit = Some(rows);
        self
    }

    /// Send only these columns, in this order.
    pub fn fields(mut self, columns: &[&str]) -> Table {
        self.view.fields = Some(columns.iter().map(|c| c.to_string()).collect());
        self
    }

    /// Round numbers in a column to this many decimal places (0–12).
    pub fn round(mut self, column: &str, places: u32) -> Table {
        self.view.round.push((column.to_owned(), places));
        self
    }

    /// Higher survives a tight budget longer. Defaults to 5.
    pub fn priority(mut self, priority: i64) -> Table {
        self.priority = Some(priority);
        self
    }

    /// Never drop this table; building fails instead if the budget cannot fit it.
    pub fn required(mut self) -> Table {
        self.required = true;
        self
    }
}

/// Default priority for sections that do not set one.
pub const DEFAULT_PRIORITY: i64 = 5;

fn add(list: &mut Vec<PreparedSection>, others: &[PreparedSection], section: Section) -> Result<()> {
    if list.iter().chain(others).any(|s| s.section().name == section.name) {
        return Err(Error::new(format!("section \"{}\" is already set", section.name)));
    }
    list.push(PreparedSection::new(section));
    Ok(())
}

fn rows(value: Value, what: &str) -> Result<Vec<Value>> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(Error::new(format!("{what} must be an array"))),
    }
}

fn table_section(name: &str, items: Value, options: Table, stable: bool, formulas: &Formulas) -> Result<Section> {
    if let Some(e) = options.error {
        return Err(Error::new(format!("table \"{name}\": {e}")));
    }
    let items = rows(items, &format!("table \"{name}\""))?;
    let viewed =
        view::apply_with(&items, &options.view, formulas).map_err(|e| Error::new(format!("table \"{name}\": {e}")))?;
    let value = table::pack(&viewed, &options.group_by.iter().map(String::as_str).collect::<Vec<_>>())
        .map_err(|e| Error::new(format!("table \"{name}\": {e}")))?;
    Ok(Section {
        name: name.to_owned(),
        priority: options.priority.unwrap_or(DEFAULT_PRIORITY),
        stable,
        required: options.required,
        value,
    })
}

impl Context {
    fn register(&mut self, section: Section) -> Result<()> {
        add(&mut self.prepared, &[], section)?;
        Ok(())
    }

    pub fn new() -> Context {
        Context::default()
    }

    /// Limit every packet to this many bytes of compact JSON.
    pub fn budget_bytes(mut self, bytes: usize) -> Context {
        self.budget_bytes = Some(bytes);
        self
    }

    /// Limit every packet to this many input tokens. Without [`Context::calibration`],
    /// every packet byte counts as one estimated token. External messages, native tools,
    /// template framing and reserved output require [`Turn::build_request`].
    pub fn budget_tokens(mut self, tokens: usize) -> Context {
        self.budget_tokens = Some(tokens);
        self
    }

    /// Estimate tokens with a calibration learned for this provider and model; see
    /// [`crate::packet::estimate`]. `scope` must match the names used when calibrating.
    pub fn calibration(mut self, calibration: Calibration, scope: &[&str]) -> Context {
        self.calibration = calibration;
        self.scope = estimate::scope(scope);
        self
    }

    /// Registers the actions the model may choose from. Written as compact signatures when
    /// every tool allows it, otherwise as packed JSON Schema. Stable and required.
    pub fn tools(&mut self, name: &str, tools: impl Into<Value>) -> Result<&mut Context> {
        let tools = rows(tools.into(), &format!("tools \"{name}\""))?;
        let value = match signatures::pack(&tools) {
            Ok(text) => Value::String(text),
            Err(_) => schema::pack(&tools).map_err(|e| Error::new(format!("tools \"{name}\": {e}")))?,
        };
        let section = Section { name: name.to_owned(), priority: i64::MAX, stable: true, required: true, value };
        self.register(section)?;
        Ok(self)
    }

    /// Registers fixed instructions or rules. Stable and required.
    pub fn prompt(&mut self, name: &str, text: &str) -> Result<&mut Context> {
        let section = Section {
            name: name.to_owned(),
            priority: i64::MAX,
            stable: true,
            required: true,
            value: Value::str(text),
        };
        self.register(section)?;
        Ok(self)
    }

    /// Registers a value that rarely changes, such as world facts. Stable; dropped only
    /// under budget pressure, after lower priorities.
    pub fn constant(&mut self, name: &str, value: impl Into<Value>, priority: i64) -> Result<&mut Context> {
        let section = Section { name: name.to_owned(), priority, stable: true, required: false, value: value.into() };
        self.register(section)?;
        Ok(self)
    }

    /// Registers a table that rarely changes. Stable.
    pub fn constant_table(&mut self, name: &str, items: impl Into<Value>, options: Table) -> Result<&mut Context> {
        let section = table_section(name, items.into(), options, true, &self.formulas)?;
        self.register(section)?;
        Ok(self)
    }

    /// Formulas that tables can `derive` columns from; see [`crate::data::formula`]. Define them
    /// directly, or apply a model's `define_formula` call with
    /// [`Formulas::apply_define`].
    pub fn formulas(&mut self) -> &mut Formulas {
        &mut self.formulas
    }

    /// Starts a turn: registered sections plus whatever the turn adds.
    pub fn turn(&self) -> Turn<'_> {
        Turn { context: self, sections: Vec::new() }
    }
}

/// One request's worth of context. Build it, send `packet.text`, then drop it.
#[derive(Debug)]
pub struct Turn<'a> {
    context: &'a Context,
    sections: Vec<PreparedSection>,
}

impl Turn<'_> {
    /// Sets a value for this turn at the default priority.
    pub fn var(&mut self, name: &str, value: impl Into<Value>) -> Result<&mut Self> {
        self.var_with(name, value, DEFAULT_PRIORITY, false)
    }

    /// Sets a value for this turn with an explicit priority, optionally required.
    pub fn var_with(
        &mut self,
        name: &str,
        value: impl Into<Value>,
        priority: i64,
        required: bool,
    ) -> Result<&mut Self> {
        let section = Section { name: name.to_owned(), priority, stable: false, required, value: value.into() };
        add(&mut self.sections, &self.context.prepared, section)?;
        Ok(self)
    }

    /// Adds current visible registered facts as a bounded table. Registry expiry,
    /// dependency validity and privacy are checked before the turn's packet budget.
    pub fn facts(
        &mut self,
        name: &str,
        registry: &crate::knowledge::facts::FactRegistry,
        now: u64,
        count: usize,
        bytes: usize,
        denied: &[String],
    ) -> Result<&mut Self> {
        self.var(name, registry.packet(now, count, bytes, denied)?)
    }

    /// Adds a table for this turn.
    pub fn table(&mut self, name: &str, items: impl Into<Value>, options: Table) -> Result<&mut Self> {
        let section = table_section(name, items.into(), options, false, &self.context.formulas)?;
        add(&mut self.sections, &self.context.prepared, section)?;
        Ok(self)
    }

    /// Adds the `count` memories most relevant to `text`; see [`Memory::recall`].
    pub fn memory(&mut self, name: &str, memory: &Memory, text: &str, count: usize) -> Result<&mut Self> {
        let recalled = memory.recall(text, count);
        self.var_with(name, Memory::table(&recalled), DEFAULT_PRIORITY, false)
    }

    /// Adds the catalog entries that best match `text`, such as the player's message; see
    /// [`Catalog::relevant`]. An unrelated message adds an empty page.
    pub fn catalog_relevant(&mut self, name: &str, catalog: &Catalog, text: &str, count: usize) -> Result<&mut Self> {
        let value =
            catalog.relevant(text, None, count, None).map_err(|e| Error::new(format!("catalog \"{name}\": {e}")))?;
        self.var_with(name, value.to_value(), DEFAULT_PRIORITY, false)
    }

    /// Adds one page of a catalog search (every word must match); see [`Catalog::search`].
    pub fn catalog_search(&mut self, name: &str, catalog: &Catalog, query: &str, count: usize) -> Result<&mut Self> {
        let page =
            catalog.search(query, None, 0, count, None).map_err(|e| Error::new(format!("catalog \"{name}\": {e}")))?;
        self.var_with(name, page.to_value(), DEFAULT_PRIORITY, false)
    }

    /// Assembles against a full rendered-request contract, including external messages,
    /// native tools and reserved output. Unlike `build`, this uses the supplied request
    /// contract instead of the Context packet budgets; existing build behavior is unchanged.
    pub fn build_request(
        &self,
        budget: &crate::packet::request::RequestBudget,
        renderer: impl FnMut(&Value) -> Result<Value>,
        counter: impl FnMut(&Value) -> Result<crate::packet::request::Count>,
    ) -> Result<crate::packet::request::BudgetedRequest> {
        let sections = self.context.prepared.iter().chain(&self.sections).collect::<Vec<_>>();
        budget.assemble_prepared(&sections, renderer, counter)
    }

    /// Assembles the packet within the context's budgets. Stable sections come first; the
    /// lowest-priority optional sections are dropped if needed, and listed in the receipt.
    pub fn build(&self) -> Result<Packet> {
        let c = self.context;
        let sections: Vec<_> = c.prepared.iter().chain(&self.sections).collect();
        if c.budget_tokens.is_none() {
            return assemble_prepared_bytes(&sections, c.budget_bytes.unwrap_or(usize::MAX));
        }
        assemble_prepared(&sections, &|text: &str| {
            c.budget_bytes.is_none_or(|b| text.len() <= b)
                && c.budget_tokens.is_none_or(|t| c.calibration.estimate(c.scope, text.as_bytes(), 0) <= t)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj;

    fn tools() -> Value {
        crate::data::json::parse(
            r#"[{"name":"wait","description":"Wait.","strict":true,"parameters":{"type":"object","properties":{"s":{"type":"integer"}},"required":["s"],"additionalProperties":false}}]"#,
            4096,
        )
        .unwrap()
    }

    #[test]
    fn registered_sections_lead_every_turn() {
        let mut game = Context::new();
        game.tools("actions", tools()).unwrap().prompt("rules", "Be brief.").unwrap();
        let mut turn = game.turn();
        turn.var("hp", 7).unwrap();
        let text = turn.build().unwrap().text;
        assert_eq!(
            text,
            r#"{"actions":"All arguments required; no others allowed.\nwait(s): Wait.\n  s: integer","rules":"Be brief.","hp":7}"#
        );
        let mut next = game.turn();
        next.var("hp", 3).unwrap();
        assert!(next.build().unwrap().text.ends_with(r#""hp":3}"#), "turns are independent");
    }

    #[test]
    fn tools_fall_back_to_schema_when_signatures_cannot_express_them() {
        let nested = crate::data::json::parse(
            r#"[{"name":"plan","description":"Plan.","parameters":{"type":"object","properties":{"steps":{"type":"array"}}}}]"#,
            4096,
        )
        .unwrap();
        let mut game = Context::new();
        game.tools("actions", nested).unwrap();
        assert!(game.turn().build().unwrap().text.contains(r#""wrapper":"bare""#));
    }

    #[test]
    fn tables_views_and_budget_drops() {
        let mut game = Context::new().budget_bytes(120);
        game.prompt("rules", "Be brief.").unwrap();
        let npcs = vec![
            obj! {"name" => "Velmi", "faction" => "keepers", "dist" => 12.5, "trust" => 0.533},
            obj! {"name" => "Midor", "faction" => "ashfolk", "dist" => 3, "trust" => 0.21},
        ];
        let mut turn = game.turn();
        turn.table("near", npcs, Table::new().sort_by("dist").limit(1).fields(&["name", "trust"]).round("trust", 1))
            .unwrap()
            .var_with("lore", "x".repeat(200), 1, false)
            .unwrap();
        let packet = turn.build().unwrap();
        assert_eq!(packet.text, r#"{"rules":"Be brief.","near":{"columns":["name","trust"],"rows":[["Midor",0.2]]}}"#);
        assert_eq!(packet.receipt.dropped[0].name, "lore");
    }

    #[test]
    fn filters_and_relevance() {
        let game = Context::new();
        let items = vec![
            obj! {"id" => "ash_knife", "kind" => "weapon", "count" => 20},
            obj! {"id" => "salt_sling", "kind" => "weapon", "count" => 4},
            obj! {"id" => "moon_trout", "kind" => "food", "count" => 18},
        ];
        let mut turn = game.turn();
        turn.table("plenty", items.clone(), Table::new().filter("count", ">=", 15).fields(&["id"])).unwrap();
        turn.table("asked", items.clone(), Table::new().relevant_to("got any trout?").fields(&["id"])).unwrap();
        assert!(turn.table("bad", items, Table::new().filter("count", "~", 1)).is_err());
        assert_eq!(
            turn.build().unwrap().text,
            r#"{"plenty":{"columns":["id"],"rows":[["ash_knife"],["moon_trout"]]},"asked":{"columns":["id"],"rows":[["moon_trout"]]}}"#
        );
    }

    #[test]
    fn formulas_derive_columns_for_filters() {
        let mut game = Context::new();
        game.formulas().define("danger", &["hp", "dist"], "max(0, 10 - dist) * (1 - hp / 100)", "Threat.").unwrap();
        let foes = vec![obj! {"id" => "hound", "hp" => 10, "dist" => 2}, obj! {"id" => "rat", "hp" => 90, "dist" => 1}];
        let mut turn = game.turn();
        turn.table(
            "threats",
            foes,
            Table::new().derive("danger", "danger(hp, dist)").filter("danger", ">", 2).fields(&["id", "danger"]),
        )
        .unwrap();
        assert_eq!(turn.build().unwrap().text, r#"{"threats":{"columns":["id","danger"],"rows":[["hound",7.2]]}}"#);
    }

    #[test]
    fn rejects_duplicate_names_and_bad_tables() {
        let mut game = Context::new();
        game.prompt("rules", "a").unwrap();
        assert!(game.prompt("rules", "b").is_err());
        let mut turn = game.turn();
        assert!(turn.var("rules", 1).is_err(), "turn names cannot shadow registered ones");
        assert!(turn.table("t", "not rows", Table::new()).is_err());
        assert!(turn.table("t", vec![obj! {"a" => 1}], Table::new().group_by("a")).is_err());
    }
}
