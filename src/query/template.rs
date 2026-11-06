use super::{Limits, Query};
use crate::{checked as c, obj, request::bounded, view::Op, Result, Value};

/// Host-owned SQL and typed predicate slots. Arguments become AST values, never SQL.
#[derive(Clone, Debug)]
pub struct Template {
    definition: Value,
    query: Query,
    tool: Value,
    max_rows: usize,
    result_bytes: usize,
}
impl Template {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 32_768)?;
        c::fields(v, &["name", "description", "sql", "parameters", "max_rows", "result_bytes"])?;
        c::name(c::text(c::get(v, "name")?)?)?;
        if c::text(c::get(v, "description")?)?.len() > 2048 {
            return Err(c::bad());
        }
        let query = Query::parse(c::text(c::get(v, "sql")?)?)?;
        let params = c::list(c::get(v, "parameters")?)?;
        if params.len() + query.view.filters.len() > Limits::default().conditions {
            return Err(c::bad());
        }
        let mut properties: Vec<(String, Value)> = Vec::new();
        let mut required = Vec::new();
        for p in params {
            c::fields(p, &["name", "column", "op", "kind"])?;
            let name = c::text(c::get(p, "name")?)?;
            c::name(name)?;
            c::name(c::text(c::get(p, "column")?)?)?;
            if properties.iter().any(|(n, _)| n == name) {
                return Err(c::bad());
            }
            let kind = c::text(c::get(p, "kind")?)?;
            if !["string", "number", "boolean"].contains(&kind) {
                return Err(c::bad());
            }
            let op = Op::parse(c::text(c::get(p, "op")?)?)?;
            if c::text(c::get(p, "op")?)? == "==" {
                return Err(c::bad());
            }
            let mut schema = obj! {"type"=>kind};
            if kind == "string" {
                if let Value::Object(o) = &mut schema {
                    o.push(("maxLength".into(), 256.into()));
                }
            }
            if op == Op::In {
                schema = obj! {"type"=>"array","items"=>schema,"minItems"=>1,"maxItems"=>64};
            }
            properties.push((name.into(), schema));
            required.push(name.to_owned());
        }
        let tool = obj! {"name"=>c::get(v,"name")?.clone(),"description"=>c::get(v,"description")?.clone(),
        "parameters"=>obj! {"type"=>"object","properties"=>Value::Object(properties),"required"=>Value::strings(&required),"additionalProperties"=>false}};
        Ok(Self {
            definition: v.clone(),
            query,
            tool,
            max_rows: c::uint(c::get(v, "max_rows")?, 0, 65536)? as usize,
            result_bytes: c::uint(c::get(v, "result_bytes")?, 0, 65536)? as usize,
        })
    }
    pub fn tool(&self) -> Value {
        self.tool.clone()
    }
    pub fn max_rows(&self) -> usize {
        self.max_rows
    }
    pub fn result_bytes(&self) -> usize {
        self.result_bytes
    }
    /// Returns an independent prepared query. All slots are required; extras fail.
    pub fn bind(&self, arguments: &Value) -> Result<Query> {
        bounded(arguments, 65_536)?;
        let params = c::list(c::get(&self.definition, "parameters")?)?;
        let names = params.iter().map(|p| c::text(c::get(p, "name")?)).collect::<Result<Vec<_>>>()?;
        c::fields(arguments, &names)?;
        let mut query = self.query.clone();
        for p in params {
            let value = c::get(arguments, c::text(c::get(p, "name")?)?)?;
            let op = Op::parse(c::text(c::get(p, "op")?)?)?;
            let values = if op == Op::In {
                let vs = c::list(value)?;
                if vs.is_empty() || vs.len() > 64 {
                    return Err(c::bad());
                }
                vs
            } else {
                std::slice::from_ref(value)
            };
            for val in values {
                let valid = match c::text(c::get(p, "kind")?)? {
                    "string" => val.as_str().is_some_and(|s| s.chars().count() <= 256),
                    "boolean" => val.as_bool().is_some(),
                    "number" => matches!(val,Value::Number(n) if n.parse::<f64>().is_ok_and(f64::is_finite)),
                    _ => false,
                };
                if !valid {
                    return Err(c::bad());
                }
            }
            query.view.filters.push((c::text(c::get(p, "column")?)?.into(), op, value.clone()));
        }
        Ok(query)
    }
    pub fn run(&self, arguments: &Value, rows: &[Value]) -> Result<Value> {
        self.bind(arguments)?.run_bounded(rows, self.max_rows, self.result_bytes)
    }
}
