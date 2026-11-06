//! Explicit host contracts and inspectable query structure.
use super::{Expr, Query};
use crate::{checked as c, obj, Error, Result, Value};

/// An optional host-declared query shape. This checks structure, not natural-language
/// intent, filter values/operators, selected fields, sort order or permissions.
/// Duplicate filter columns express multiple bounds on the same column.
#[derive(Clone, Debug)]
pub struct Shape {
    table: String,
    filter_columns: Vec<String>,
    limit: Option<u32>,
    aggregate: bool,
}
impl Shape {
    /// Reads exactly `table`, `filter_columns`, `limit` (null or unsigned integer)
    /// and `aggregate` (boolean). Identifiers are ASCII, at most 64 bytes; at most
    /// 16 filter columns. The contract should come from trusted host logic.
    pub fn from_value(v: &Value) -> Result<Self> {
        c::fields(v, &["table", "filter_columns", "limit", "aggregate"])?;
        fn name(v: &Value) -> Result<String> {
            let s = c::text(v)?;
            if s.is_empty()
                || s.len() > 64
                || !(s.as_bytes()[0].is_ascii_alphabetic() || s.starts_with('_'))
                || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                return Err(c::bad());
            }
            Ok(s.to_owned())
        }
        let columns = c::list(c::get(v, "filter_columns")?)?;
        if columns.len() > 16 {
            return Err(c::bad());
        }
        let mut filter_columns = columns.iter().map(name).collect::<Result<Vec<_>>>()?;
        filter_columns.sort();
        let limit = match c::get(v, "limit")? {
            Value::Null => None,
            n => Some(c::uint(n, 0, u32::MAX as u64)? as u32),
        };
        Ok(Self {
            table: name(c::get(v, "table")?)?,
            filter_columns,
            limit,
            aggregate: c::boolean(c::get(v, "aggregate")?)?,
        })
    }
    /// Canonical contract, with sorted filter column names (duplicates retained).
    pub fn to_value(&self) -> Value {
        obj! {"table"=>self.table.clone(), "filter_columns"=>Value::strings(&self.filter_columns),
        "limit"=>self.limit.map(Value::from).unwrap_or(Value::Null), "aggregate"=>self.aggregate}
    }
}
impl Query {
    /// Structure of this parsed query. Filter numeric spellings are preserved.
    /// This is an explanation for host review, not a proof of correct intent.
    pub fn plan(&self) -> Value {
        let mut columns = Vec::new();
        for item in &self.items {
            item.columns(&mut columns);
        }
        columns.extend(self.view.filters.iter().map(|(c, _, _)| c.as_str()));
        columns.extend(self.view.sort_by.iter().map(|(c, _)| c.as_str()));
        let mut unique = Vec::new();
        for column in columns {
            if !unique.contains(&column) {
                unique.push(column);
            }
        }
        let filters = self
            .view
            .filters
            .iter()
            .map(|(column, op, value)| {
                use crate::view::Op;
                let op = match op {
                    Op::Eq => "=",
                    Op::Ne => "!=",
                    Op::Gt => ">",
                    Op::Ge => ">=",
                    Op::Lt => "<",
                    Op::Le => "<=",
                    Op::In => "in",
                };
                obj! {"column"=>column.clone(), "op"=>op, "value"=>value.clone()}
            })
            .collect();
        obj! {"table"=>self.table.clone(), "aggregate"=>self.items.iter().any(Expr::has_agg),
        "columns"=>Value::strings(&unique), "filters"=>Value::Array(filters),
        "order_by"=>self.view.sort_by.as_ref().map(|(c,d)|obj!{"column"=>c.clone(),"descending"=>*d}).unwrap_or(Value::Null),
        "limit"=>self.view.limit.map(Value::from).unwrap_or(Value::Null)}
    }
    /// Enforces an independent host contract before execution. Equivalent queries may
    /// have different shapes; use only when exact structure is a host requirement.
    pub fn require_shape(&self, shape: &Shape) -> Result<()> {
        let mut columns: Vec<_> = self.view.filters.iter().map(|(c, _, _)| c.as_str()).collect();
        columns.sort();
        if self.table != shape.table
            || self.items.iter().any(Expr::has_agg) != shape.aggregate
            || self.view.limit != shape.limit.map(|n| n as usize)
            || columns != shape.filter_columns
        {
            return Err(Error::new("query shape differs from host requirements"));
        }
        Ok(())
    }
    /// Refuses oversized input before execution and oversized compact JSON output
    /// after computation. Results are never truncated. These are row/byte bounds,
    /// not a CPU deadline or a bound on host cell sizes/transient allocations.
    pub fn run_bounded(&self, rows: &[Value], max_rows: usize, result_bytes: usize) -> Result<Value> {
        if rows.len() > max_rows {
            return Err(Error::new("query input row limit"));
        }
        let result = self.run(rows)?;
        if result.json_len() > result_bytes {
            return Err(Error::new("query result byte limit"));
        }
        Ok(result)
    }
}
