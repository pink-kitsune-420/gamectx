use super::*;
/// Host-assisted full-render counting for non-Rust integrations. The host renders
/// each candidate with all framing/tools and returns its measured or estimated count.
/// Counts remain trusted host assertions, never tokenizer output certified here.
#[derive(Clone, Debug)]
pub struct Composition {
    definition: Value,
    kept: Vec<usize>,
    omitted: Vec<Value>,
    sequence: usize,
    status: &'static str,
    receipt: Value,
    budget: RequestBudget,
}
impl Composition {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 524_288)?;
        c::fields(v, &["id", "identity", "sections", "budget"])?;
        c::scope(c::text(c::get(v, "id")?)?)?;
        crate::deployment::Identity::from_value(c::get(v, "identity")?)?;
        let rows = c::list(c::get(v, "sections")?)?;
        if rows.len() > 128 {
            return Err(c::bad());
        }
        let mut names = Vec::new();
        for r in rows {
            c::fields(r, &["id", "value", "priority", "required"])?;
            let name = c::text(c::get(r, "id")?)?;
            c::name(name)?;
            if names.contains(&name) {
                return Err(c::bad());
            }
            names.push(name);
            let p = c::get(r, "priority")?.to_json().parse::<i32>().map_err(|_| c::bad())?;
            if !(-1_000_000..=1_000_000).contains(&p) {
                return Err(c::bad());
            }
            c::boolean(c::get(r, "required")?)?;
        }
        Ok(Self {
            definition: v.clone(),
            kept: (0..rows.len()).collect(),
            omitted: Vec::new(),
            sequence: 0,
            status: "count",
            receipt: Value::Null,
            budget: RequestBudget::from_value(c::get(v, "budget")?)?,
        })
    }
    fn rows(&self) -> &[Value] {
        self.definition.get("sections").and_then(Value::as_array).expect("sections")
    }
    fn candidate(&self) -> Value {
        let packet = Value::Object(
            self.kept
                .iter()
                .map(|&i| {
                    let r = &self.rows()[i];
                    (r.get("id").and_then(Value::as_str).expect("id").into(), r.get("value").expect("value").clone())
                })
                .collect(),
        );
        obj! {"id"=>self.definition.get("id").expect("id").clone(),"identity"=>self.definition.get("identity").expect("identity").clone(),"packet"=>packet}
    }
    pub fn state(&self) -> Value {
        obj! {"status"=>self.status,"sequence"=>self.sequence,"candidate"=>self.candidate(),"budget"=>self.receipt.clone(),"omitted"=>self.omitted.clone()}
    }
    /// Exact candidate equality and sequence bind this count to this request. Changes
    /// to tool/schema/rendering must start a new request ID and composition session.
    /// Malformed acknowledgements fail without changing state. Terminal states refuse reuse.
    pub fn observe(&mut self, obs: &Value) -> Result<Value> {
        bounded(obs, 786_432)?;
        c::fields(obs, &["sequence", "candidate", "count"])?;
        if self.status != "count"
            || c::uint(c::get(obs, "sequence")?, 0, 128)? as usize != self.sequence
            || c::get(obs, "candidate")? != &self.candidate()
        {
            return Err(c::bad());
        }
        let receipt = self.budget.assess(Count::from_value(c::get(obs, "count")?)?)?;
        if receipt.fits() {
            self.status = "ready";
            self.receipt = receipt.to_value();
        } else {
            let victim = self
                .kept
                .iter()
                .copied()
                .filter(|&i| self.rows()[i].get("required").and_then(Value::as_bool) == Some(false))
                .min_by_key(|&i| {
                    (
                        self.rows()[i].get("priority").expect("priority").to_json().parse::<i32>().expect("priority"),
                        std::cmp::Reverse(i),
                    )
                });
            if let Some(i) = victim {
                self.omitted.push(self.rows()[i].get("id").expect("id").clone());
                self.kept.retain(|&j| j != i);
                self.sequence += 1;
                self.receipt = Value::Null;
            } else {
                self.status = "overflow";
                self.receipt = receipt.to_value();
            }
        }
        Ok(self.state())
    }
}
