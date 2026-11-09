//! Group dynamics, crowds and councils (pass 5, stage 7, design `R`, local kind `group`).
use crate::character::limits::{Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, exact_fields, Persist};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "coalitions", default: 16, ceiling: 64 },
    Limit { name: "contagion_cap", default: 15, ceiling: 50 },
    Limit { name: "max_shift", default: 25, ceiling: 100 },
    Limit { name: "questions", default: 16, ceiling: 64 },
];

pub const DECISION_MODES: &[&str] = &["consensus", "leader", "vote"];

fn int_range(v: &Value, lo: i64, hi: i64) -> Result<i64> {
    match v {
        Value::Number(s) => s.parse::<i64>().ok().filter(|n| (lo..=hi).contains(n)).ok_or_else(c::bad),
        _ => Err(c::bad()),
    }
}

#[derive(Clone, Debug)]
pub struct Question {
    pub id: String,
    pub prompt: String,
    pub mode: String,
    pub leader: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Coalition {
    pub id: String,
    pub question: String,
    pub members: BTreeSet<String>,
    pub speaker: String,
    pub cohesion: u64,
}

#[derive(Clone, Debug)]
pub struct GroupManager {
    pub limits: Limits,
    pub questions: BTreeMap<String, Question>,
    pub stances: BTreeMap<String, BTreeMap<String, i64>>,
    pub coalitions: BTreeMap<String, Coalition>,
    pub beings: BTreeSet<String>,
}

impl Default for GroupManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GroupManager {
    pub fn new() -> Self {
        Self {
            limits: Limits::defaults(LIMITS),
            questions: BTreeMap::new(),
            stances: BTreeMap::new(),
            coalitions: BTreeMap::new(),
            beings: BTreeSet::new(),
        }
    }

    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    fn ensure_being(&mut self, bid: &str) -> Result<()> {
        if !self.beings.contains(bid) {
            if self.beings.len() + 1 > self.limits.get("beings") {
                return Err(c::bad());
            }
            self.beings.insert(bid.to_string());
        }
        Ok(())
    }

    pub fn op_question(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["id", "prompt"], &["leader", "mode"])?;
        let qid = ident(c::get(args, "id")?)?.to_string();
        let prompt_raw = c::text(c::get(args, "prompt")?)?;
        if prompt_raw.is_empty() || prompt_raw.len() > 128 {
            return Err(c::bad());
        }
        let prompt = prompt_raw.to_string();

        let mode = match c::get(args, "mode") {
            Ok(v) if v != &Value::Null => {
                let m = c::text(v)?;
                if m.is_empty() || m.len() > 16 {
                    return Err(c::bad());
                }
                m.to_ascii_lowercase()
            }
            _ => "vote".to_string(),
        };
        if !DECISION_MODES.contains(&mode.as_str()) {
            return Err(c::bad());
        }

        let leader = match c::get(args, "leader") {
            Ok(v) if v != &Value::Null => {
                let lid = ident(v)?.to_string();
                self.ensure_being(&lid)?;
                Some(lid)
            }
            _ => {
                if mode == "leader" {
                    return Err(c::bad());
                }
                None
            }
        };

        if !self.questions.contains_key(&qid) && self.questions.len() + 1 > self.limits.get("questions") {
            return Err(c::bad());
        }

        self.questions.insert(
            qid.clone(),
            Question {
                id: qid.clone(),
                prompt: prompt.clone(),
                mode: mode.clone(),
                leader: leader.clone(),
            },
        );

        if !self.stances.contains_key(&qid) {
            self.stances.insert(qid.clone(), BTreeMap::new());
        }

        Ok(obj! {
            "id" => qid,
            "leader" => match leader {
                Some(l) => Value::str(l),
                None => Value::Null,
            },
            "mode" => mode,
            "prompt" => prompt,
        })
    }

    pub fn op_stance(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "question"], &["delta", "reason", "stance"])?;
        let bid = ident(c::get(args, "being")?)?.to_string();
        let qid = ident(c::get(args, "question")?)?.to_string();
        if !self.questions.contains_key(&qid) {
            return Err(c::bad());
        }
        self.ensure_being(&bid)?;

        let current = self.stances.get(&qid).and_then(|m| m.get(&bid)).copied().unwrap_or(0);

        let v_stance = c::get(args, "stance").ok().filter(|v| *v != &Value::Null);
        let v_delta = c::get(args, "delta").ok().filter(|v| *v != &Value::Null);

        let (applied_delta, new_s) = match (v_stance, v_delta) {
            (Some(s_val), None) => {
                let raw_s = int_range(s_val, -100, 100)?;
                (raw_s - current, raw_s)
            }
            (None, Some(d_val)) => {
                let max_s = self.limits.get("max_shift") as i64;
                let raw_delta = int_range(d_val, -max_s, max_s)?;
                let new_s = (current + raw_delta).max(-100).min(100);
                (raw_delta, new_s)
            }
            _ => return Err(c::bad()),
        };

        let reason = match c::get(args, "reason") {
            Ok(v) if v != &Value::Null => {
                let r = c::text(v)?;
                if r.is_empty() || r.len() > 32 {
                    return Err(c::bad());
                }
                Some(r.to_string())
            }
            _ => None,
        };

        self.stances.entry(qid.clone()).or_default().insert(bid.clone(), new_s);

        let mut res = vec![
            (String::from("being"), Value::str(bid)),
            (String::from("delta"), Value::from(applied_delta)),
            (String::from("question"), Value::str(qid)),
        ];
        if let Some(r) = reason {
            res.push((String::from("reason"), Value::str(r)));
        }
        res.push((String::from("stance"), Value::from(new_s)));
        res.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(Value::Object(res))
    }

    pub fn op_coalition(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["id", "members", "question"], &["speaker"])?;
        let cid = ident(c::get(args, "id")?)?.to_string();
        let qid = ident(c::get(args, "question")?)?.to_string();
        if !self.questions.contains_key(&qid) {
            return Err(c::bad());
        }

        let raw_m = c::list(c::get(args, "members")?)?;
        if raw_m.len() < 2 {
            return Err(c::bad());
        }

        let mut members = BTreeSet::new();
        for m in raw_m {
            let id = ident(m)?.to_string();
            if !members.insert(id) {
                return Err(c::bad()); // duplicate rejected
            }
        }

        for m in &members {
            self.ensure_being(m)?;
        }

        let speaker = match c::get(args, "speaker") {
            Ok(v) if v != &Value::Null => {
                let sp = ident(v)?.to_string();
                if !members.contains(&sp) {
                    return Err(c::bad());
                }
                sp
            }
            _ => members.iter().next().unwrap().clone(),
        };

        let q_stances = self.stances.get(&qid);
        let m_vec: Vec<&String> = members.iter().collect();
        let mut diff_sum: i64 = 0;
        let mut diff_count: usize = 0;

        for i in 0..m_vec.len() {
            let s_i = q_stances.and_then(|m| m.get(m_vec[i])).copied().unwrap_or(0);
            for j in (i + 1)..m_vec.len() {
                let s_j = q_stances.and_then(|m| m.get(m_vec[j])).copied().unwrap_or(0);
                diff_sum += (s_i - s_j).abs();
                diff_count += 1;
            }
        }

        let avg_diff = if diff_count > 0 { diff_sum / diff_count as i64 } else { 0 };
        let cohesion = (100 - avg_diff).max(0).min(100) as u64;

        if !self.coalitions.contains_key(&cid) && self.coalitions.len() + 1 > self.limits.get("coalitions") {
            return Err(c::bad());
        }

        self.coalitions.insert(
            cid.clone(),
            Coalition {
                id: cid.clone(),
                question: qid.clone(),
                members: members.clone(),
                speaker: speaker.clone(),
                cohesion,
            },
        );

        let members_arr: Vec<Value> = members.into_iter().map(Value::str).collect();

        Ok(obj! {
            "cohesion" => Value::from(cohesion),
            "id" => cid,
            "members" => Value::Array(members_arr),
            "question" => qid,
            "speaker" => speaker,
        })
    }

    pub fn op_break_ranks(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "coalition"], &[])?;
        let bid = ident(c::get(args, "being")?)?.to_string();
        let cid = ident(c::get(args, "coalition")?)?.to_string();

        let c_data = self.coalitions.get_mut(&cid).ok_or_else(c::bad)?;
        if !c_data.members.contains(&bid) || c_data.members.len() <= 1 {
            return Err(c::bad());
        }

        c_data.members.remove(&bid);
        if c_data.speaker == bid {
            c_data.speaker = c_data.members.iter().next().unwrap().clone();
        }

        let qid = c_data.question.clone();
        let q_stances = self.stances.get(&qid);
        let m_vec: Vec<&String> = c_data.members.iter().collect();
        let mut diff_sum: i64 = 0;
        let mut diff_count: usize = 0;

        for i in 0..m_vec.len() {
            let s_i = q_stances.and_then(|m| m.get(m_vec[i])).copied().unwrap_or(0);
            for j in (i + 1)..m_vec.len() {
                let s_j = q_stances.and_then(|m| m.get(m_vec[j])).copied().unwrap_or(0);
                diff_sum += (s_i - s_j).abs();
                diff_count += 1;
            }
        }

        let avg_diff = if diff_count > 0 { diff_sum / diff_count as i64 } else { 0 };
        c_data.cohesion = (100 - avg_diff).max(0).min(100) as u64;

        let rem_members: Vec<Value> = c_data.members.iter().cloned().map(Value::str).collect();
        let cohesion = c_data.cohesion;

        Ok(obj! {
            "being" => bid,
            "coalition" => cid,
            "cohesion" => Value::from(cohesion),
            "penalty" => Value::from(30),
            "remaining_members" => Value::Array(rem_members),
        })
    }

    pub fn op_audience(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["audience", "speaker", "threat"], &["confession_risk", "trust"])?;
        let speaker = ident(c::get(args, "speaker")?)?.to_string();
        self.ensure_being(&speaker)?;

        let raw_aud = c::list(c::get(args, "audience")?)?;
        let mut aud = BTreeSet::new();
        for a in raw_aud {
            let id = ident(a)?.to_string();
            if id != speaker {
                aud.insert(id);
            }
        }
        for a in &aud {
            self.ensure_being(a)?;
        }

        let threat = int_range(c::get(args, "threat")?, 0, 100)? as u64;

        let c_risk = match c::get(args, "confession_risk") {
            Ok(v) if v != &Value::Null => int_range(v, 0, 100)? as u64,
            _ => 50,
        };

        let mut trust_map = BTreeMap::new();
        if let Ok(v) = c::get(args, "trust") {
            if v != &Value::Null {
                let obj = v.as_object().ok_or_else(c::bad)?;
                for (k, val) in obj {
                    let k_id = ident(&Value::str(k.clone()))?.to_string();
                    let t_val = int_range(val, 0, 100)? as u64;
                    trust_map.insert(k_id, t_val);
                }
            }
        }

        let aud_size = aud.len() as u64;
        let mult = 1000 + (aud_size * 250).min(1000);
        let amp_threat = ((threat * mult) / 1000).min(100);

        let mut can_disclose = true;
        for b in &aud {
            let t = trust_map.get(b).copied().unwrap_or(50);
            if t < c_risk {
                can_disclose = false;
                break;
            }
        }

        Ok(obj! {
            "amplified_threat" => Value::from(amp_threat),
            "audience_size" => Value::from(aud_size),
            "can_disclose" => can_disclose,
            "threat_multiplier" => Value::from(mult),
        })
    }

    pub fn op_contagion(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["beings"], &[])?;
        let raw_beings = c::list(c::get(args, "beings")?)?;
        if raw_beings.is_empty() {
            return Err(c::bad());
        }

        let mut b_list = Vec::new();
        let mut total_w: i64 = 0;
        let mut w_sum: i64 = 0;

        for entry in raw_beings {
            keys(entry, &["being", "mood"], &["status"])?;
            let bid = ident(c::get(entry, "being")?)?.to_string();
            self.ensure_being(&bid)?;

            let mood = int_range(c::get(entry, "mood")?, -100, 100)?;
            let st = match c::get(entry, "status") {
                Ok(v) if v != &Value::Null => int_range(v, 1, 10)?,
                _ => 5,
            };

            b_list.push((bid, mood, st));
            total_w += st;
            w_sum += mood * st;
        }

        let room_mood = w_sum.div_euclid(total_w);
        let cap = self.limits.get("contagion_cap") as i64;

        let mut shifts = Vec::new();
        let mut new_moods = Vec::new();

        for (bid, mood, _) in b_list {
            let gap = room_mood - mood;
            let step = (gap.div_euclid(2)).max(-cap).min(cap);
            let final_m = (mood + step).max(-100).min(100);

            shifts.push(obj! {
                "being" => bid,
                "mood" => Value::from(final_m),
                "shift" => Value::from(step),
            });
            new_moods.push(final_m);
        }

        let variance = new_moods.iter().map(|m| (m - room_mood).pow(2)).sum::<i64>() / new_moods.len() as i64;

        Ok(obj! {
            "beings" => Value::Array(shifts),
            "contagion_cap" => Value::from(cap),
            "room_mood" => Value::from(room_mood),
            "variance" => Value::from(variance),
        })
    }

    pub fn op_decide(&self, args: &Value) -> Result<Value> {
        keys(args, &["question"], &[])?;
        let qid = ident(c::get(args, "question")?)?.to_string();
        let q_info = self.questions.get(&qid).ok_or_else(c::bad)?;
        let mode = &q_info.mode;
        let empty_map = BTreeMap::new();
        let q_stances = self.stances.get(&qid).unwrap_or(&empty_map);

        let mut yes_votes: u64 = 0;
        let mut no_votes: u64 = 0;
        let mut abstain_votes: u64 = 0;
        let mut dissidents = Vec::new();
        let outcome: &str;

        if mode == "vote" {
            for (_, &s) in q_stances {
                if s > 0 {
                    yes_votes += 1;
                } else if s < 0 {
                    no_votes += 1;
                } else {
                    abstain_votes += 1;
                }
            }

            let passed = yes_votes > no_votes;
            outcome = if passed { "passed" } else { "failed" };

            for (b, &s) in q_stances {
                if passed && s < 0 {
                    dissidents.push(Value::str(b.clone()));
                } else if !passed && s > 0 {
                    dissidents.push(Value::str(b.clone()));
                }
            }
        } else if mode == "consensus" {
            let mut vetoers = Vec::new();
            for (b, &s) in q_stances {
                if s >= 0 {
                    yes_votes += 1;
                } else {
                    no_votes += 1;
                    vetoers.push(Value::str(b.clone()));
                }
            }

            if !vetoers.is_empty() {
                outcome = "vetoed";
                dissidents = vetoers;
            } else {
                outcome = "passed";
            }
        } else if mode == "leader" {
            let leader = q_info.leader.as_ref().ok_or_else(c::bad)?;
            let leader_stance = q_stances.get(leader).copied().ok_or_else(c::bad)?;
            let passed = leader_stance > 0;
            outcome = if passed { "passed" } else { "failed" };

            for (b, &s) in q_stances {
                if s > 0 {
                    yes_votes += 1;
                } else if s < 0 {
                    no_votes += 1;
                } else {
                    abstain_votes += 1;
                }

                if b != leader {
                    if passed && s < 0 {
                        dissidents.push(Value::str(b.clone()));
                    } else if !passed && s > 0 {
                        dissidents.push(Value::str(b.clone()));
                    }
                }
            }
        } else {
            return Err(c::bad());
        }

        dissidents.sort_by(|a, b| a.as_str().cmp(&b.as_str()));

        Ok(obj! {
            "dissidents" => Value::Array(dissidents),
            "outcome" => outcome,
            "question" => qid,
            "votes" => obj! {
                "abstain" => Value::from(abstain_votes),
                "no" => Value::from(no_votes),
                "yes" => Value::from(yes_votes),
            },
        })
    }

    pub fn settle_state(&mut self) -> Result<()> {
        if self.beings.len() > self.limits.get("beings") {
            return Err(c::bad());
        }
        if self.questions.len() > self.limits.get("questions") {
            return Err(c::bad());
        }
        if self.coalitions.len() > self.limits.get("coalitions") {
            return Err(c::bad());
        }
        Ok(())
    }

    pub fn migrate_state(_from: u64, _state: Value) -> Result<Value> {
        Err(c::bad())
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        keys(request, &["ops"], &["limits"])?;
        let supplied = c::get(request, "limits").is_ok();
        if let Ok(l) = c::get(request, "limits") {
            self.limits.apply(l)?;
        }

        let ops = c::list(c::get(request, "ops")?)?;
        if ops.is_empty() || ops.len() > 64 {
            return Err(c::bad());
        }

        let mut results = Vec::new();
        for item in ops {
            keys(item, &["op"], &["args"])?;
            let op_name = c::text(c::get(item, "op")?)?;
            let args = c::get(item, "args").unwrap_or(&Value::Null);

            let res = match op_name {
                "question" => self.op_question(args)?,
                "stance" => self.op_stance(args)?,
                "coalition" => self.op_coalition(args)?,
                "break_ranks" => self.op_break_ranks(args)?,
                "audience" => self.op_audience(args)?,
                "contagion" => self.op_contagion(args)?,
                "decide" => self.op_decide(args)?,
                "export" => persist::export_op(self, &self.limits, args)?,
                "import" => {
                    let limits = self.limits.clone();
                    persist::import(self, &limits, args)?
                }
                _ => return Err(c::bad()),
            };
            results.push(res);
        }

        let mut out = obj! {
            "results" => Value::Array(results),
            "version" => 1,
        };
        if supplied {
            if let Value::Object(ref mut fields) = out {
                fields.insert(0, (
                    "limits".to_string(),
                    Value::Object(vec![
                        ("beings".to_string(), Value::from(self.limits.get("beings") as u64)),
                        ("coalitions".to_string(), Value::from(self.limits.get("coalitions") as u64)),
                        ("contagion_cap".to_string(), Value::from(self.limits.get("contagion_cap") as u64)),
                        ("max_shift".to_string(), Value::from(self.limits.get("max_shift") as u64)),
                        ("questions".to_string(), Value::from(self.limits.get("questions") as u64)),
                    ]),
                ));
            }
        }
        Ok(out)
    }
}

impl Persist for GroupManager {
    fn save(&self) -> Value {
        let mut q_out = Vec::new();
        for (qid, q) in &self.questions {
            q_out.push(Value::Array(vec![
                Value::str(qid.clone()),
                Value::str(q.leader.clone().unwrap_or_default()),
                Value::str(q.mode.clone()),
                Value::str(q.prompt.clone()),
            ]));
        }

        let mut s_out = Vec::new();
        for (qid, b_map) in &self.stances {
            let mut b_rows = Vec::new();
            for (bid, &s) in b_map {
                b_rows.push(Value::Array(vec![Value::str(bid.clone()), Value::from(s)]));
            }
            s_out.push(Value::Array(vec![Value::str(qid.clone()), Value::Array(b_rows)]));
        }

        let mut c_out = Vec::new();
        for (cid, c) in &self.coalitions {
            let m_arr: Vec<Value> = c.members.iter().cloned().map(Value::str).collect();
            c_out.push(Value::Array(vec![
                Value::str(cid.clone()),
                Value::from(c.cohesion),
                Value::Array(m_arr),
                Value::str(c.question.clone()),
                Value::str(c.speaker.clone()),
            ]));
        }

        let b_arr: Vec<Value> = self.beings.iter().cloned().map(Value::str).collect();

        obj! {
            "beings" => Value::Array(b_arr),
            "coalitions" => Value::Array(c_out),
            "questions" => Value::Array(q_out),
            "stances" => Value::Array(s_out),
        }
    }

    fn load(v: &Value) -> Result<Self> {
        exact_fields(v, &["beings", "coalitions", "questions", "stances"])?;
        let mut mgr = Self::new();

        let b_list = c::list(c::get(v, "beings")?)?;
        if b_list.len() > mgr.limits.get("beings") {
            return Err(c::bad());
        }
        let mut new_beings = BTreeSet::new();
        let mut last_b: Option<&str> = None;
        for b_val in b_list {
            let bid = ident(b_val)?;
            if let Some(prev) = last_b {
                if bid <= prev {
                    return Err(c::bad());
                }
            }
            last_b = Some(bid);
            new_beings.insert(bid.to_string());
        }

        let q_list = c::list(c::get(v, "questions")?)?;
        if q_list.len() > mgr.limits.get("questions") {
            return Err(c::bad());
        }
        let mut new_questions = BTreeMap::new();
        let mut last_q: Option<&str> = None;
        for q_val in q_list {
            let row = c::list(q_val)?;
            if row.len() != 4 {
                return Err(c::bad());
            }
            let qid = ident(&row[0])?;
            if let Some(prev) = last_q {
                if qid <= prev {
                    return Err(c::bad());
                }
            }
            last_q = Some(qid);

            let l_raw = c::text(&row[1])?;
            let leader = if l_raw.is_empty() {
                None
            } else {
                let lid = ident(&row[1])?;
                Some(lid.to_string())
            };

            let mode = c::text(&row[2])?.to_ascii_lowercase();
            if !DECISION_MODES.contains(&mode.as_str()) {
                return Err(c::bad());
            }

            let prompt = c::text(&row[3])?;
            if prompt.is_empty() || prompt.len() > 128 {
                return Err(c::bad());
            }

            new_questions.insert(
                qid.to_string(),
                Question {
                    id: qid.to_string(),
                    leader,
                    mode,
                    prompt: prompt.to_string(),
                },
            );
        }

        let s_list = c::list(c::get(v, "stances")?)?;
        let mut new_stances = BTreeMap::new();
        let mut last_sq: Option<&str> = None;
        for s_val in s_list {
            let row = c::list(s_val)?;
            if row.len() != 2 {
                return Err(c::bad());
            }
            let qid = ident(&row[0])?;
            if !new_questions.contains_key(qid) {
                return Err(c::bad());
            }
            if let Some(prev) = last_sq {
                if qid <= prev {
                    return Err(c::bad());
                }
            }
            last_sq = Some(qid);

            let b_entries = c::list(&row[1])?;
            let mut b_map = BTreeMap::new();
            let mut last_sb: Option<&str> = None;
            for pair_val in b_entries {
                let pair = c::list(pair_val)?;
                if pair.len() != 2 {
                    return Err(c::bad());
                }
                let bid = ident(&pair[0])?;
                if !new_beings.contains(bid) {
                    return Err(c::bad());
                }
                if let Some(prev) = last_sb {
                    if bid <= prev {
                        return Err(c::bad());
                    }
                }
                last_sb = Some(bid);
                let s_val = int_range(&pair[1], -100, 100)?;
                b_map.insert(bid.to_string(), s_val);
            }
            new_stances.insert(qid.to_string(), b_map);
        }

        let c_list = c::list(c::get(v, "coalitions")?)?;
        if c_list.len() > mgr.limits.get("coalitions") {
            return Err(c::bad());
        }
        let mut new_coalitions = BTreeMap::new();
        let mut last_c: Option<&str> = None;
        for c_val in c_list {
            let row = c::list(c_val)?;
            if row.len() != 5 {
                return Err(c::bad());
            }
            let cid = ident(&row[0])?;
            if let Some(prev) = last_c {
                if cid <= prev {
                    return Err(c::bad());
                }
            }
            last_c = Some(cid);

            let cohesion = c::uint(&row[1], 0, 100)?;
            let raw_m = c::list(&row[2])?;
            if raw_m.len() < 2 {
                return Err(c::bad());
            }
            let mut m_set = BTreeSet::new();
            let mut last_cm: Option<&str> = None;
            for m in raw_m {
                let mid = ident(m)?;
                if !new_beings.contains(mid) {
                    return Err(c::bad());
                }
                if let Some(prev) = last_cm {
                    if mid <= prev {
                        return Err(c::bad());
                    }
                }
                last_cm = Some(mid);
                m_set.insert(mid.to_string());
            }

            let qid = ident(&row[3])?;
            if !new_questions.contains_key(qid) {
                return Err(c::bad());
            }

            let speaker = ident(&row[4])?;
            if !m_set.contains(speaker) {
                return Err(c::bad());
            }

            new_coalitions.insert(
                cid.to_string(),
                Coalition {
                    id: cid.to_string(),
                    cohesion,
                    members: m_set,
                    question: qid.to_string(),
                    speaker: speaker.to_string(),
                },
            );
        }

        mgr.beings = new_beings;
        mgr.questions = new_questions;
        mgr.stances = new_stances;
        mgr.coalitions = new_coalitions;
        mgr.settle_state()?;
        Ok(mgr)
    }
}

durable!(GroupManager, "group", LIMITS);

pub fn workflow(request: &Value) -> Result<Value> {
    GroupManager::new().workflow(request)
}
