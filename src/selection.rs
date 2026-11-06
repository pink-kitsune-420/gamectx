//! Dependency-complete, bounded host evidence selection. It neither discovers
//! dependencies nor certifies truth/privacy. Supply already authorized records.
use crate::{checked as c, obj, request::bounded, Result, Value};

#[derive(Clone, Debug)]
pub struct Evidence {
    rows: Vec<Value>,
    closures: Vec<Vec<usize>>,
}
impl Evidence {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 262_144)?;
        let rows = c::list(v)?;
        if rows.len() > 128 {
            return Err(c::bad());
        }
        let mut ids = Vec::new();
        for row in rows {
            c::fields(row, &["id", "value", "depends", "priority", "required"])?;
            let id = c::text(c::get(row, "id")?)?;
            c::name(id)?;
            if ids.contains(&id) {
                return Err(c::bad());
            }
            ids.push(id);
            let p = c::get(row, "priority")?.to_json().parse::<i32>().map_err(|_| c::bad())?;
            if !(-1_000_000..=1_000_000).contains(&p) {
                return Err(c::bad());
            }
            c::boolean(c::get(row, "required")?)?;
        }
        let mut edges = Vec::new();
        for row in rows {
            let deps = c::list(c::get(row, "depends")?)?;
            if deps.len() > 16 {
                return Err(c::bad());
            }
            let mut indices = Vec::new();
            for dep in deps {
                let name = c::text(dep)?;
                let i = ids.iter().position(|s| *s == name).ok_or_else(c::bad)?;
                if indices.contains(&i) {
                    return Err(c::bad());
                }
                indices.push(i);
            }
            edges.push(indices);
        }
        fn visit(
            i: usize,
            edges: &[Vec<usize>],
            active: &mut [bool],
            memo: &mut [Option<Vec<usize>>],
        ) -> Result<Vec<usize>> {
            if active[i] {
                return Err(c::bad());
            }
            if let Some(v) = &memo[i] {
                return Ok(v.clone());
            }
            active[i] = true;
            let mut out = vec![i];
            for &j in &edges[i] {
                for k in visit(j, edges, active, memo)? {
                    if !out.contains(&k) {
                        out.push(k);
                    }
                }
            }
            active[i] = false;
            memo[i] = Some(out.clone());
            Ok(out)
        }
        let mut memo = vec![None; rows.len()];
        let mut active = vec![false; rows.len()];
        let mut closures = Vec::new();
        for i in 0..rows.len() {
            closures.push(visit(i, &edges, &mut active, &mut memo)?);
        }
        Ok(Self { rows: rows.to_vec(), closures })
    }
    fn packet(&self, kept: &[bool]) -> Value {
        Value::Object(
            self.rows
                .iter()
                .enumerate()
                .filter(|(i, _)| kept[*i])
                .map(|(_, r)| {
                    (r.get("id").and_then(Value::as_str).expect("id").into(), r.get("value").expect("value").clone())
                })
                .collect(),
        )
    }
    /// Greedy priority selection, preserving input order on ties and in output.
    /// Required dependency closures must fit. The cap applies to packet JSON only.
    pub fn select(&self, max_bytes: usize) -> Result<Value> {
        if max_bytes > 262_144 {
            return Err(c::bad());
        }
        let mut kept = vec![false; self.rows.len()];
        for (i, r) in self.rows.iter().enumerate() {
            if r.get("required").and_then(Value::as_bool) == Some(true) {
                for &j in &self.closures[i] {
                    kept[j] = true;
                }
            }
        }
        if self.packet(&kept).json_len() > max_bytes {
            return Err(c::bad());
        }
        let mut order = (0..self.rows.len()).collect::<Vec<_>>();
        order.sort_by_key(|&i| {
            std::cmp::Reverse(
                self.rows[i].get("priority").expect("priority").to_json().parse::<i32>().expect("integer"),
            )
        });
        for i in order {
            let mut candidate = kept.clone();
            for &j in &self.closures[i] {
                candidate[j] = true;
            }
            if self.packet(&candidate).json_len() <= max_bytes {
                kept = candidate;
            }
        }
        let packet = self.packet(&kept);
        let bytes = packet.json_len();
        let names = |want| {
            self.rows
                .iter()
                .enumerate()
                .filter(|(i, _)| kept[*i] == want)
                .map(|(_, r)| r.get("id").expect("id").clone())
                .collect::<Vec<_>>()
        };
        Ok(obj! {"packet"=>packet,"kept"=>names(true),"omitted"=>names(false),"bytes"=>bytes})
    }
}
