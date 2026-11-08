//! Relationship reconciliation: reconciles Sheet relationships, Life attitudes,
//! community relations, and viewpoints into unified precedence and appraisal.
//!
//! Precedence order:
//! 1. Dynamic Life attitudes (highest personal precedence)
//! 2. Sheet relationships (authored baseline priors)
//! 3. Community relations (group-level standing bias)
//! 4. Viewpoints (stylistic stance)
//!
//! Action outcomes follow a single appraisal path updating personal attitudes,
//! which aggregate back into community relations.
//! Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::relationship::RelationshipManager;
//!
//! let mut mgr = RelationshipManager::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {
//!       "op": "resolve",
//!       "args": {
//!         "subject": "baker",
//!         "target": "player",
//!         "sheet_relationship": {"with": "player", "kind": "friend"}
//!       }
//!     }
//!   ]
//! }"#, 4096)?;
//! let out = mgr.workflow(&req)?;
//! assert_eq!(out.get("version").and_then(|v| v.as_usize()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::affect::ATTITUDE_KINDS;
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::BTreeMap;

pub const MAX_RELATIONS: usize = 64;
pub const MAX_OPS: usize = 64;

/// Host-set limits: sheet `subjects` and `relations` per subject (default [`MAX_RELATIONS`]),
/// personal `attitudes` and `community_pairs`. Pre-declared entries, `init` and the ops that add
/// attitudes or community trust all respect them.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "subjects", default: 4096, ceiling: 1_000_000 },
    crate::character::limits::Limit { name: "relations", default: MAX_RELATIONS, ceiling: 4096 },
    crate::character::limits::Limit { name: "attitudes", default: 4096, ceiling: 1_000_000 },
    crate::character::limits::Limit { name: "community_pairs", default: 4096, ceiling: 1_000_000 },
];
crate::character::limits::module_limits!(RelationshipLimits, LIMITS);

fn room<K: Ord, V>(map: &BTreeMap<K, V>, key: &K, max: usize) -> Result<()> {
    if map.len() >= max && !map.contains_key(key) {
        return Err(c::bad());
    }
    Ok(())
}

fn ident_bound(v: &Value) -> Result<&str> {
    let s = ident(v)?;
    if s.is_empty() || s.len() > 64 {
        return Err(c::bad());
    }
    Ok(s)
}

fn text_bound(v: &Value, max_len: usize) -> Result<&str> {
    let s = v.as_str().ok_or_else(c::bad)?;
    if s.is_empty() || s.chars().count() > max_len {
        return Err(c::bad());
    }
    if s.chars().any(|ch| (ch as u32) < 32 || (127..=159).contains(&(ch as u32))) {
        return Err(c::bad());
    }
    Ok(s)
}

fn i64_bound(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    match v {
        Value::Number(s) => {
            let n = s.parse::<i64>().map_err(|_| c::bad())?;
            if n < lo || n > hi {
                return Err(c::bad());
            }
            Ok(n)
        }
        _ => Err(c::bad()),
    }
}

fn bool_bound(v: &Value) -> Result<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(c::bad()),
    }
}

pub fn stance_from_trust(trust: i64) -> &'static str {
    if trust >= 50 {
        "allied"
    } else if trust >= 20 {
        "friendly"
    } else if trust >= -15 {
        "neutral"
    } else if trust >= -50 {
        "wary"
    } else {
        "hostile"
    }
}

pub fn sheet_kind_baselines(kind: &str) -> (i64, i64) {
    // Returns (default_trust, default_affection)
    match kind {
        "friend" => (40, 40),
        "mentor" => (50, 30),
        "apprentice" => (30, 20),
        "ally" => (35, 10),
        "rival" => (-20, -10),
        "enemy" => (-50, -50),
        _ => (0, 0),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SheetRel {
    pub target: String,
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonalAtt {
    pub subject: String,
    pub target: String,
    pub kind: String,
    pub level: i64,
}

#[derive(Clone, Debug, Default)]
pub struct RelationshipManager {
    // subject -> (target -> SheetRel)
    pub sheet_relationships: BTreeMap<String, BTreeMap<String, SheetRel>>,
    // (subject, target, kind) -> level
    pub personal_attitudes: BTreeMap<(String, String, String), i64>,
    // (community, target_community) -> trust
    pub community_trust: BTreeMap<(String, String), i64>,
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: RelationshipLimits,
}

impl RelationshipManager {
    pub fn new() -> Self {
        Self::default()
    }

    crate::character::limits::limited_workflow!();

    fn put_sheet(&mut self, subject: &str, with: &str, kind: &str) -> Result<()> {
        room(&self.sheet_relationships, &subject.to_string(), self.limits.get("subjects"))?;
        let max_relations = self.limits.get("relations");
        let sheet = self.sheet_relationships.entry(subject.to_string()).or_default();
        room(sheet, &with.to_string(), max_relations)?;
        sheet.insert(with.to_string(), SheetRel { target: with.to_string(), kind: kind.to_string() });
        Ok(())
    }

    fn put_attitude(&mut self, key: (String, String, String), level: i64) -> Result<()> {
        room(&self.personal_attitudes, &key, self.limits.get("attitudes"))?;
        self.personal_attitudes.insert(key, level);
        Ok(())
    }

    fn put_community(&mut self, key: (String, String), trust: i64) -> Result<()> {
        room(&self.community_trust, &key, self.limits.get("community_pairs"))?;
        self.community_trust.insert(key, trust);
        Ok(())
    }

    pub fn set_attitude(&mut self, subject: &str, target: &str, kind: &str, delta: i64) -> i64 {
        let key = (subject.to_string(), target.to_string(), kind.to_string());
        let current = self.personal_attitudes.get(&key).copied().unwrap_or(0);
        let next = (current + delta).clamp(-100, 100);
        self.personal_attitudes.insert(key, next);
        next
    }

    fn run_workflow(&mut self, request: &Value) -> Result<Value> {
        keys(request, &["ops"], &["sheet_relationships", "attitudes", "communities"])?;
        let ops = request.get("ops").and_then(Value::as_array).ok_or_else(c::bad)?;
        if ops.is_empty() || ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        // Optional pre-declared sheet relationships
        if let Some(sheets_arr) = request.get("sheet_relationships").and_then(Value::as_array) {
            for item in sheets_arr {
                keys(item, &["subject", "with", "kind"], &[])?;
                let s = ident_bound(c::get(item, "subject")?)?;
                let w = ident_bound(c::get(item, "with")?)?;
                let k = text_bound(c::get(item, "kind")?, 32)?;
                self.put_sheet(s, w, k)?;
            }
        }

        // Optional pre-declared personal attitudes
        if let Some(atts_arr) = request.get("attitudes").and_then(Value::as_array) {
            for item in atts_arr {
                keys(item, &["subject", "target", "kind", "level"], &[])?;
                let s = ident_bound(c::get(item, "subject")?)?;
                let t = ident_bound(c::get(item, "target")?)?;
                let k = text_bound(c::get(item, "kind")?, 32)?;
                if !ATTITUDE_KINDS.contains(&k) {
                    return Err(c::bad());
                }
                let lvl = i64_bound(c::get(item, "level")?, -100, 100)?;
                self.put_attitude((s.to_string(), t.to_string(), k.to_string()), lvl)?;
            }
        }

        // Optional pre-declared communities
        if let Some(comms_arr) = request.get("communities").and_then(Value::as_array) {
            for item in comms_arr {
                keys(item, &["community", "target", "trust"], &["stance"])?;
                let c1 = ident_bound(c::get(item, "community")?)?;
                let c2 = ident_bound(c::get(item, "target")?)?;
                let tr = i64_bound(c::get(item, "trust")?, -100, 100)?;
                self.put_community((c1.to_string(), c2.to_string()), tr)?;
            }
        }

        let mut results = Vec::new();
        for op_val in ops {
            keys(op_val, &["op", "args"], &[])?;
            let op = text_bound(c::get(op_val, "op")?, 32)?;
            let args = c::get(op_val, "args")?;

            let res = match op {
                "init" => self.op_init(args)?,
                "assess" => self.op_assess(args)?,
                "resolve" => self.op_resolve(args)?,
                "appraise_action" => self.op_appraise_action(args)?,
                "aggregate_community_trust" => self.op_aggregate_community_trust(args)?,
                _ => return Err(c::bad()),
            };
            results.push(res);
        }

        Ok(obj! {
            "version" => Value::int(1),
            "results" => Value::Array(results),
        })
    }

    fn op_init(&mut self, args: &Value) -> Result<Value> {
        keys(args, &[], &["sheet_relationships", "attitudes", "communities"])?;
        self.sheet_relationships.clear();
        self.personal_attitudes.clear();
        self.community_trust.clear();

        if let Some(sheets_arr) = args.get("sheet_relationships").and_then(Value::as_array) {
            for item in sheets_arr {
                keys(item, &["subject", "with", "kind"], &[])?;
                let s = ident_bound(c::get(item, "subject")?)?;
                let w = ident_bound(c::get(item, "with")?)?;
                let k = text_bound(c::get(item, "kind")?, 32)?;
                self.put_sheet(s, w, k)?;
            }
        }

        if let Some(atts_arr) = args.get("attitudes").and_then(Value::as_array) {
            for item in atts_arr {
                keys(item, &["subject", "target", "kind", "level"], &[])?;
                let s = ident_bound(c::get(item, "subject")?)?;
                let t = ident_bound(c::get(item, "target")?)?;
                let k = text_bound(c::get(item, "kind")?, 32)?;
                if !ATTITUDE_KINDS.contains(&k) {
                    return Err(c::bad());
                }
                let lvl = i64_bound(c::get(item, "level")?, -100, 100)?;
                self.put_attitude((s.to_string(), t.to_string(), k.to_string()), lvl)?;
            }
        }

        if let Some(comms_arr) = args.get("communities").and_then(Value::as_array) {
            for item in comms_arr {
                keys(item, &["community", "target", "trust"], &["stance"])?;
                let c1 = ident_bound(c::get(item, "community")?)?;
                let c2 = ident_bound(c::get(item, "target")?)?;
                let tr = i64_bound(c::get(item, "trust")?, -100, 100)?;
                self.put_community((c1.to_string(), c2.to_string()), tr)?;
            }
        }

        Ok(obj! {
            "ok" => Value::Bool(true),
        })
    }

    fn op_assess(&self, args: &Value) -> Result<Value> {
        keys(args, &["subject"], &["target"])?;
        let subject = ident_bound(c::get(args, "subject")?)?;
        let filter_target = args.get("target").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;

        let mut attitudes_list = Vec::new();
        // Keys are ordered (subject, target, kind): read only this subject's (and target's) range.
        let start = (subject.to_string(), filter_target.unwrap_or("").to_string(), String::new());
        for ((s, t, k), lvl) in self.personal_attitudes.range(start..) {
            if s != subject || filter_target.is_some_and(|tgt| t != tgt) {
                break;
            }
            attitudes_list.push(obj! {
                "kind" => Value::str(k),
                "level" => Value::from(*lvl),
                "target" => Value::str(t),
            });
        }
        attitudes_list.sort_by(|a, b| {
            let a_t = a.get("target").and_then(Value::as_str).unwrap_or("");
            let b_t = b.get("target").and_then(Value::as_str).unwrap_or("");
            let a_k = a.get("kind").and_then(Value::as_str).unwrap_or("");
            let b_k = b.get("kind").and_then(Value::as_str).unwrap_or("");
            (a_t, a_k).cmp(&(b_t, b_k))
        });

        let mut sheets_list = Vec::new();
        if let Some(map) = self.sheet_relationships.get(subject) {
            for (w, rel) in map {
                if let Some(tgt) = filter_target {
                    if w != tgt {
                        continue;
                    }
                }
                sheets_list.push(obj! {
                    "kind" => Value::str(&rel.kind),
                    "with" => Value::str(w),
                });
            }
        }
        sheets_list.sort_by(|a, b| {
            let a_w = a.get("with").and_then(Value::as_str).unwrap_or("");
            let b_w = b.get("with").and_then(Value::as_str).unwrap_or("");
            a_w.cmp(b_w)
        });

        Ok(obj! {
            "attitudes" => Value::Array(attitudes_list),
            "sheet_relationships" => Value::Array(sheets_list),
            "subject" => Value::str(subject),
        })
    }

    fn op_resolve(&self, args: &Value) -> Result<Value> {
        keys(args, &["subject", "target"], &["sheet_relationship", "community", "target_community", "viewpoint"])?;
        let subject = ident_bound(c::get(args, "subject")?)?;
        let target = ident_bound(c::get(args, "target")?)?;

        // 1. Personal attitudes (Highest Precedence)
        let personal_trust_opt =
            self.personal_attitudes.get(&(subject.to_string(), target.to_string(), "trust".to_string())).copied();
        let personal_affection_opt =
            self.personal_attitudes.get(&(subject.to_string(), target.to_string(), "affection".to_string())).copied();

        // 2. Sheet relationship (Fallback Baseline)
        let sheet_kind = if let Some(sheet_arg) = args.get("sheet_relationship").filter(|x| **x != Value::Null) {
            keys(sheet_arg, &["with", "kind"], &[])?;
            let w = ident_bound(c::get(sheet_arg, "with")?)?;
            if w != target {
                return Err(c::bad());
            }
            Some(text_bound(c::get(sheet_arg, "kind")?, 32)?.to_string())
        } else {
            self.sheet_relationships.get(subject).and_then(|m| m.get(target)).map(|r| r.kind.clone())
        };

        let (sheet_trust, sheet_affection) = match sheet_kind.as_deref() {
            Some(k) => sheet_kind_baselines(k),
            None => (0, 0),
        };

        let base_trust = personal_trust_opt.unwrap_or(sheet_trust);
        let base_affection = personal_affection_opt.unwrap_or(sheet_affection);

        // 3. Community standing bias
        let mut community_mod = 0;
        let comm_subj = args.get("community").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
        let comm_tgt = args.get("target_community").filter(|x| **x != Value::Null).map(ident_bound).transpose()?;
        if let (Some(c1), Some(c2)) = (comm_subj, comm_tgt) {
            if let Some(ct) = self.community_trust.get(&(c1.to_string(), c2.to_string())).copied() {
                // Integer division matching Python int(ct / 4)
                let raw_mod = ct / 4;
                // If strong personal trust exists (>= 50), group prejudice is halved
                community_mod = if base_trust.abs() >= 50 { raw_mod / 2 } else { raw_mod };
            }
        }

        let effective_trust = (base_trust + community_mod).clamp(-100, 100);
        let effective_affection = base_affection.clamp(-100, 100);
        let dominant_stance = stance_from_trust(effective_trust);

        // 4. Viewpoint styling
        let viewpoint_stance =
            args.get("viewpoint").filter(|x| **x != Value::Null).map(|v| text_bound(v, 64)).transpose()?;

        let precedence_reason = if personal_trust_opt.is_some() {
            if community_mod != 0 {
                format!("personal_attitude:trust({base_trust}) + community_bias({community_mod})")
            } else {
                format!("personal_attitude:trust({base_trust})")
            }
        } else if let Some(ref k) = sheet_kind {
            if community_mod != 0 {
                format!("sheet_relation:{k}({base_trust}) + community_bias({community_mod})")
            } else {
                format!("sheet_relation:{k}({base_trust})")
            }
        } else if community_mod != 0 {
            format!("community_bias({community_mod})")
        } else {
            "default_neutral".to_string()
        };

        let mut out = vec![
            ("dominant_stance".to_string(), Value::str(dominant_stance)),
            ("effective_affection".to_string(), Value::from(effective_affection)),
            ("effective_trust".to_string(), Value::from(effective_trust)),
            ("precedence_reason".to_string(), Value::str(&precedence_reason)),
            ("subject".to_string(), Value::str(subject)),
            ("target".to_string(), Value::str(target)),
        ];

        if let Some(ref k) = sheet_kind {
            out.push(("sheet_kind".to_string(), Value::str(k)));
        }
        if let Some(vp) = viewpoint_stance {
            out.push(("viewpoint_stance".to_string(), Value::str(vp)));
        }

        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(Value::Object(out))
    }

    fn op_appraise_action(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["actor", "target", "action"], &["success", "reason", "extra_effects"])?;
        let actor = ident_bound(c::get(args, "actor")?)?;
        let target = ident_bound(c::get(args, "target")?)?;
        let action = text_bound(c::get(args, "action")?, 32)?;
        let success = args.get("success").map(bool_bound).transpose()?.unwrap_or(true);

        // Standard appraisal deltas target -> actor (how target's feelings toward actor change)
        let mut deltas: Vec<(&'static str, i64, &'static str)> = Vec::new();

        match action {
            "barter" => {
                if success {
                    deltas.push(("trust", 5, "successful_trade"));
                    deltas.push(("gratitude", 5, "fair_exchange"));
                } else {
                    deltas.push(("trust", -10, "failed_barter"));
                    deltas.push(("resentment", 5, "unfair_terms"));
                }
            }
            "broken_promise" => {
                deltas.push(("trust", -20, "broken_promise"));
                deltas.push(("resentment", 25, "neglected_obligation"));
            }
            "fulfilled_promise" => {
                deltas.push(("trust", 25, "kept_promise"));
                deltas.push(("respect", 15, "reliable_partner"));
                deltas.push(("gratitude", 10, "honored_commitment"));
            }
            "attack" => {
                deltas.push(("trust", -60, "physical_aggression"));
                deltas.push(("resentment", 60, "violent_attack"));
                deltas.push(("fear", 30, "threat_to_safety"));
            }
            "help" => {
                deltas.push(("trust", 20, "received_assistance"));
                deltas.push(("gratitude", 25, "helpful_deed"));
                deltas.push(("affection", 15, "kindness"));
            }
            "gift" => {
                deltas.push(("gratitude", 15, "received_gift"));
                deltas.push(("affection", 15, "generosity"));
            }
            "teach" => {
                deltas.push(("respect", 15, "learned_skill"));
                deltas.push(("gratitude", 15, "guided_instruction"));
            }
            "deception_exposed" => {
                deltas.push(("trust", -25, "caught_in_lie"));
                deltas.push(("resentment", 15, "dishonesty"));
            }
            _ => {
                if !success {
                    deltas.push(("trust", -5, "action_failure"));
                }
            }
        }

        let mut applied = Vec::new();
        for (kind, delta, default_reason) in deltas {
            room(
                &self.personal_attitudes,
                &(target.to_string(), actor.to_string(), kind.to_string()),
                self.limits.get("attitudes"),
            )?;
            let next_lvl = self.set_attitude(target, actor, kind, delta);
            applied.push(obj! {
                "attitude" => Value::str(kind),
                "delta" => Value::from(delta),
                "new_level" => Value::from(next_lvl),
                "reason" => Value::str(default_reason),
                "subject" => Value::str(target),
                "target" => Value::str(actor),
            });
        }

        applied.sort_by(|a, b| {
            let a_att = a.get("attitude").and_then(Value::as_str).unwrap_or("");
            let b_att = b.get("attitude").and_then(Value::as_str).unwrap_or("");
            a_att.cmp(b_att)
        });

        Ok(obj! {
            "action" => Value::str(action),
            "actor" => Value::str(actor),
            "applied_effects" => Value::Array(applied),
            "ok" => Value::Bool(true),
            "target" => Value::str(target),
        })
    }

    fn op_aggregate_community_trust(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["community", "target_community", "members", "target_members"], &[])?;
        let comm1 = ident_bound(c::get(args, "community")?)?;
        let comm2 = ident_bound(c::get(args, "target_community")?)?;
        let members = c::get(args, "members")?.as_array().ok_or_else(c::bad)?;
        let target_members = c::get(args, "target_members")?.as_array().ok_or_else(c::bad)?;

        let mut sum_trust: i64 = 0;
        let mut count: i64 = 0;

        for m1_val in members {
            let m1 = ident_bound(m1_val)?;
            for m2_val in target_members {
                let m2 = ident_bound(m2_val)?;
                let trust = self
                    .personal_attitudes
                    .get(&(m1.to_string(), m2.to_string(), "trust".to_string()))
                    .copied()
                    .unwrap_or(0);
                sum_trust += trust;
                count += 1;
            }
        }

        let avg_trust = if count > 0 {
            // Integer division truncation
            (sum_trust / count).clamp(-100, 100)
        } else {
            0
        };

        self.put_community((comm1.to_string(), comm2.to_string()), avg_trust)?;
        let stance = stance_from_trust(avg_trust);

        Ok(obj! {
            "community" => Value::str(comm1),
            "derived_stance" => Value::str(stance),
            "ok" => Value::Bool(true),
            "target_community" => Value::str(comm2),
            "trust" => Value::from(avg_trust),
        })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    let mut mgr = RelationshipManager::new();
    mgr.workflow(req)
}
