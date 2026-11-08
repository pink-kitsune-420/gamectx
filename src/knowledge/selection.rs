//! Dependency-complete, bounded host evidence selection. It neither discovers
//! dependencies nor certifies truth/privacy. Supply already authorized records.
use crate::{checked as c, obj, packet::request::bounded, Result, Value};
use std::collections::BTreeMap;
/// Host-set limits (defaults are the earlier fixed bounds): most evidence `records` and the
/// most serialized `bytes` in the record list. A selected packet stays within 262,144 bytes.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "records", default: 128, ceiling: 1024 },
    crate::character::limits::Limit { name: "bytes", default: 262_144, ceiling: 4_194_304 },
];
crate::character::limits::module_limits!(EvidenceLimits, LIMITS);

#[derive(Clone, Debug)]
pub struct Evidence {
    rows: Vec<Value>,
    closures: Vec<Vec<usize>>,
}
impl Evidence {
    pub fn from_value(v: &Value) -> Result<Self> {
        Self::from_value_within(v, &EvidenceLimits::default())
    }
    /// [`from_value`](Self::from_value) with host-set [limits](LIMITS).
    pub fn from_value_within(v: &Value, l: &crate::character::limits::Limits) -> Result<Self> {
        bounded(v, l.get("bytes"))?;
        let rows = c::list(v)?;
        if rows.len() > l.get("records") {
            return Err(c::bad());
        }
        let mut ids = BTreeMap::new();
        for row in rows {
            c::fields(row, &["id", "value", "depends", "priority", "required"])?;
            let id = c::text(c::get(row, "id")?)?;
            c::name(id)?;
            if ids.insert(id, ids.len()).is_some() {
                return Err(c::bad());
            }
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
                let i = *ids.get(name).ok_or_else(c::bad)?;
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
        // Packet bytes are summed per row (`"id":value` plus a comma between rows), so each
        // candidate is priced without rebuilding the packet.
        let row_bytes = self
            .rows
            .iter()
            .map(|r| {
                crate::data::json::string_len(r.get("id").and_then(Value::as_str).expect("id"))
                    + 1
                    + r.get("value").expect("value").json_len()
            })
            .collect::<Vec<_>>();
        let (mut sum, mut count) = (0usize, 0usize);
        for (i, k) in kept.iter().enumerate() {
            if *k {
                sum += row_bytes[i];
                count += 1;
            }
        }
        let mut order = (0..self.rows.len()).collect::<Vec<_>>();
        order.sort_by_key(|&i| {
            std::cmp::Reverse(
                self.rows[i].get("priority").expect("priority").to_json().parse::<i32>().expect("integer"),
            )
        });
        for i in order {
            let new = self.closures[i].iter().filter(|j| !kept[**j]);
            let (add, n) = new.fold((0usize, 0usize), |(s, n), j| (s + row_bytes[*j], n + 1));
            if 2 + sum + add + (count + n).saturating_sub(1) <= max_bytes {
                for &j in &self.closures[i] {
                    kept[j] = true;
                }
                sum += add;
                count += n;
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

/// The `evidence` local kind: `{records, max_bytes ≤262144, limits?}` →
/// `{limits?, packet, kept, omitted, bytes}`. `limits` are the [host-set limits](LIMITS),
/// echoed when supplied.
pub fn workflow(a: &Value) -> Result<Value> {
    let mut limits = EvidenceLimits::default();
    let stripped = crate::character::limits::take(a, &mut limits)?;
    let a = stripped.as_ref().unwrap_or(a);
    c::fields(a, &["records", "max_bytes"])?;
    let max_bytes = c::uint(c::get(a, "max_bytes")?, 0, 262_144)? as usize;
    let out = Evidence::from_value_within(c::get(a, "records")?, &limits)?.select(max_bytes)?;
    Ok(crate::character::limits::echo(out, &limits, stripped.is_some()))
}
