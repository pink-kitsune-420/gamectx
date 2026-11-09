//! Season director and narrative identity (pass 5, stage 9, design `V` & `W`, local kind `narrative`).
use crate::character::limits::{Limit, Limits};
use crate::checked as c;
use crate::data::persist::{self, durable, exact_fields, Persist};
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const LIMITS: &[Limit] = &[
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "chapters", default: 8, ceiling: 16 },
    Limit { name: "setups", default: 64, ceiling: 512 },
    Limit { name: "storylets", default: 32, ceiling: 128 },
];

pub const THEMES: &[&str] = &[
    "contamination",
    "growth",
    "loss",
    "redemption",
    "steadiness",
];

pub const STORYLET_KINDS: &[&str] = &[
    "coalition_vote",
    "overdue_debt",
    "project_deadline",
    "rumor_drift",
    "strain_threshold",
];

pub const SETUP_STATUSES: &[&str] = &[
    "open",
    "paid_off",
    "retired",
];

#[derive(Clone, Debug)]
pub struct Storylet {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub tension: u64,
    pub actors: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct Setup {
    pub id: String,
    pub kind: String,
    pub description: String,
    pub due_start: u64,
    pub due_end: u64,
    pub status: String,
    pub target: Option<String>,
    pub paid_at: Option<u64>,
    pub resolution: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Chapter {
    pub chapter: u64,
    pub summary: String,
    pub theme: String,
    pub turning_point: String,
}

#[derive(Clone, Debug)]
pub struct NarrativeManager {
    pub limits: Limits,
    pub storylets: BTreeMap<String, Storylet>,
    pub setups: BTreeMap<String, Setup>,
    pub autobiographies: BTreeMap<String, Vec<Chapter>>,
    pub beings: BTreeSet<String>,
}

impl Default for NarrativeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NarrativeManager {
    pub fn new() -> Self {
        Self {
            limits: Limits::defaults(LIMITS),
            storylets: BTreeMap::new(),
            setups: BTreeMap::new(),
            autobiographies: BTreeMap::new(),
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

    pub fn op_storylet(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["id", "kind", "tension", "title"], &["actors"])?;
        let sid = ident(c::get(args, "id")?)?.to_string();
        let kind = c::text(c::get(args, "kind")?)?.to_ascii_lowercase();
        if !STORYLET_KINDS.contains(&kind.as_str()) {
            return Err(c::bad());
        }
        let title_raw = c::text(c::get(args, "title")?)?;
        if title_raw.is_empty() || title_raw.len() > 64 {
            return Err(c::bad());
        }
        let title = title_raw.to_string();
        let tension = c::uint(c::get(args, "tension")?, 0, 100)?;

        let mut actors = BTreeSet::new();
        if let Ok(v) = c::get(args, "actors") {
            if v != &Value::Null {
                for a in c::list(v)? {
                    let aid = ident(a)?.to_string();
                    actors.insert(aid);
                }
            }
        }
        for a in &actors {
            self.ensure_being(a)?;
        }

        if !self.storylets.contains_key(&sid) && self.storylets.len() + 1 > self.limits.get("storylets") {
            return Err(c::bad());
        }

        self.storylets.insert(
            sid.clone(),
            Storylet {
                id: sid.clone(),
                kind: kind.clone(),
                title: title.clone(),
                tension,
                actors: actors.clone(),
            },
        );

        let actors_arr: Vec<Value> = actors.into_iter().map(Value::str).collect();

        Ok(obj! {
            "actors" => Value::Array(actors_arr),
            "id" => sid,
            "kind" => kind,
            "tension" => Value::from(tension),
            "title" => title,
        })
    }

    pub fn op_rank(&self, args: &Value) -> Result<Value> {
        keys(args, &["season_tension"], &["storylets"])?;
        let season_t = c::uint(c::get(args, "season_tension")?, 0, 100)?;

        let candidate_ids: Vec<String> = match c::get(args, "storylets") {
            Ok(v) if v != &Value::Null => {
                let mut ids = Vec::new();
                for s in c::list(v)? {
                    let sid = ident(s)?.to_string();
                    if !self.storylets.contains_key(&sid) {
                        return Err(c::bad());
                    }
                    ids.push(sid);
                }
                ids
            }
            _ => self.storylets.keys().cloned().collect(),
        };

        struct RankedItem {
            fit: i64,
            tension: u64,
            id: String,
            kind: String,
            title: String,
        }

        let mut ranked = Vec::new();
        for sid in candidate_ids {
            let s_data = self.storylets.get(&sid).unwrap();
            let t = s_data.tension;
            let fit = 100 - (t as i64 - season_t as i64).abs();
            ranked.push(RankedItem {
                fit,
                tension: t,
                id: sid,
                kind: s_data.kind.clone(),
                title: s_data.title.clone(),
            });
        }

        // Rank by fit desc, tension desc, id asc
        ranked.sort_by(|a, b| {
            b.fit
                .cmp(&a.fit)
                .then_with(|| b.tension.cmp(&a.tension))
                .then_with(|| a.id.cmp(&b.id))
        });

        let ranked_arr: Vec<Value> = ranked
            .into_iter()
            .map(|r| {
                obj! {
                    "fit" => Value::from(r.fit),
                    "id" => r.id,
                    "kind" => r.kind,
                    "tension" => Value::from(r.tension),
                    "title" => r.title,
                }
            })
            .collect();

        Ok(obj! {
            "ranked" => Value::Array(ranked_arr),
            "season_tension" => Value::from(season_t),
        })
    }

    pub fn op_setup(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["description", "due_end", "due_start", "id", "kind"], &["target"])?;
        let sid = ident(c::get(args, "id")?)?.to_string();
        let kind_raw = c::text(c::get(args, "kind")?)?;
        if kind_raw.is_empty() || kind_raw.len() > 32 {
            return Err(c::bad());
        }
        let kind = kind_raw.to_string();

        let desc_raw = c::text(c::get(args, "description")?)?;
        if desc_raw.is_empty() || desc_raw.len() > 128 {
            return Err(c::bad());
        }
        let desc = desc_raw.to_string();

        let d_start = c::uint(c::get(args, "due_start")?, 0, 1_000_000_000)?;
        let d_end = c::uint(c::get(args, "due_end")?, d_start, 1_000_000_000)?;

        let target = match c::get(args, "target") {
            Ok(v) if v != &Value::Null => {
                let tid = ident(v)?.to_string();
                self.ensure_being(&tid)?;
                Some(tid)
            }
            _ => None,
        };

        if !self.setups.contains_key(&sid) && self.setups.len() + 1 > self.limits.get("setups") {
            return Err(c::bad());
        }

        self.setups.insert(
            sid.clone(),
            Setup {
                id: sid.clone(),
                kind: kind.clone(),
                description: desc.clone(),
                due_start: d_start,
                due_end: d_end,
                status: "open".to_string(),
                target: target.clone(),
                paid_at: None,
                resolution: None,
            },
        );

        Ok(obj! {
            "description" => desc,
            "due_end" => Value::from(d_end),
            "due_start" => Value::from(d_start),
            "id" => sid,
            "kind" => kind,
            "status" => "open",
            "target" => match target {
                Some(t) => Value::str(t),
                None => Value::Null,
            },
        })
    }

    pub fn op_payoff(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["at", "id", "resolution"], &[])?;
        let sid = ident(c::get(args, "id")?)?.to_string();
        let s_data = self.setups.get_mut(&sid).ok_or_else(c::bad)?;
        if s_data.status != "open" {
            return Err(c::bad());
        }

        let at = c::uint(c::get(args, "at")?, 0, 1_000_000_000)?;
        let res_raw = c::text(c::get(args, "resolution")?)?;
        if res_raw.is_empty() || res_raw.len() > 128 {
            return Err(c::bad());
        }
        let res = res_raw.to_string();

        s_data.status = "paid_off".to_string();
        s_data.paid_at = Some(at);
        s_data.resolution = Some(res.clone());

        Ok(obj! {
            "id" => sid,
            "paid_at" => Value::from(at),
            "resolution" => res,
            "status" => "paid_off",
        })
    }

    pub fn op_ledger(&self, args: &Value) -> Result<Value> {
        keys(args, &["clock"], &[])?;
        let clock = c::uint(c::get(args, "clock")?, 0, 1_000_000_000)?;

        let mut due_now = Vec::new();
        let mut overdue = Vec::new();
        let mut paid_off = Vec::new();
        let mut retire_candidates = Vec::new();
        let mut open_count: u64 = 0;

        for (sid, s) in &self.setups {
            if s.status == "open" {
                open_count += 1;
                if s.due_start <= clock && clock <= s.due_end {
                    due_now.push(Value::str(sid.clone()));
                } else if clock > s.due_end {
                    overdue.push(Value::str(sid.clone()));
                    if clock > s.due_end + 100 {
                        retire_candidates.push(Value::str(sid.clone()));
                    }
                }
            } else if s.status == "paid_off" {
                paid_off.push(Value::str(sid.clone()));
            }
        }

        Ok(obj! {
            "clock" => Value::from(clock),
            "due_now" => Value::Array(due_now),
            "open_count" => Value::from(open_count),
            "overdue" => Value::Array(overdue),
            "paid_off" => Value::Array(paid_off),
            "retire_candidates" => Value::Array(retire_candidates),
        })
    }

    pub fn op_chapter(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "summary", "theme", "turning_point"], &[])?;
        let bid = ident(c::get(args, "being")?)?.to_string();
        self.ensure_being(&bid)?;

        let theme_raw = c::text(c::get(args, "theme")?)?.to_ascii_lowercase();
        if !THEMES.contains(&theme_raw.as_str()) {
            return Err(c::bad());
        }
        let theme = theme_raw;

        let sum_raw = c::text(c::get(args, "summary")?)?;
        if sum_raw.is_empty() || sum_raw.len() > 128 {
            return Err(c::bad());
        }
        let summary = sum_raw.to_string();

        let tp_raw = c::text(c::get(args, "turning_point")?)?;
        if tp_raw.is_empty() || tp_raw.len() > 64 {
            return Err(c::bad());
        }
        let turning_point = tp_raw.to_string();

        let chapters = self.autobiographies.entry(bid.clone()).or_default();
        if chapters.len() + 1 > self.limits.get("chapters") {
            return Err(c::bad());
        }

        let ch_num = (chapters.len() + 1) as u64;
        chapters.push(Chapter {
            chapter: ch_num,
            summary: summary.clone(),
            theme: theme.clone(),
            turning_point: turning_point.clone(),
        });

        Ok(obj! {
            "being" => bid,
            "chapter" => Value::from(ch_num),
            "summary" => summary,
            "theme" => theme,
            "turning_point" => turning_point,
        })
    }

    pub fn op_reminisce(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &["chapter"])?;
        let bid = ident(c::get(args, "being")?)?.to_string();
        self.ensure_being(&bid)?;

        let empty_vec = Vec::new();
        let chapters = self.autobiographies.get(&bid).unwrap_or(&empty_vec);

        if let Ok(v) = c::get(args, "chapter") {
            if v != &Value::Null {
                let ch_idx = c::uint(v, 1, self.limits.get("chapters") as u64)? as usize;
                if ch_idx > chapters.len() {
                    return Err(c::bad());
                }
                let ch = &chapters[ch_idx - 1];
                let rem_line = format!("I recall {}: {} ({}).", ch.theme, ch.summary, ch.turning_point);
                return Ok(obj! {
                    "being" => bid,
                    "chapter" => Value::from(ch_idx as u64),
                    "reminiscence_line" => rem_line,
                    "summary" => ch.summary.clone(),
                    "theme" => ch.theme.clone(),
                    "turning_point" => ch.turning_point.clone(),
                });
            }
        }

        if chapters.is_empty() {
            return Ok(obj! {
                "being" => bid,
                "dominant_theme" => "steadiness",
                "reminiscence_line" => "My life has kept a steady course.",
                "total_chapters" => Value::from(0),
            });
        }

        let mut theme_counts: BTreeMap<String, usize> = BTreeMap::new();
        for ch in chapters {
            *theme_counts.entry(ch.theme.clone()).or_default() += 1;
        }

        // Dominant: highest count, alphabetical tie break
        let mut sorted_themes: Vec<(&String, &usize)> = theme_counts.iter().collect();
        sorted_themes.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let dominant = sorted_themes[0].0.clone();
        let latest = chapters.last().unwrap();
        let rem_line = format!("My path has been one of {}; lately, {}.", dominant, latest.summary);

        Ok(obj! {
            "being" => bid,
            "dominant_theme" => dominant,
            "reminiscence_line" => rem_line,
            "total_chapters" => Value::from(chapters.len() as u64),
        })
    }

    pub fn settle_state(&mut self) -> Result<()> {
        if self.beings.len() > self.limits.get("beings") {
            return Err(c::bad());
        }
        if self.storylets.len() > self.limits.get("storylets") {
            return Err(c::bad());
        }
        if self.setups.len() > self.limits.get("setups") {
            return Err(c::bad());
        }
        let max_ch = self.limits.get("chapters");
        for chs in self.autobiographies.values() {
            if chs.len() > max_ch {
                return Err(c::bad());
            }
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
                "storylet" => self.op_storylet(args)?,
                "rank" => self.op_rank(args)?,
                "setup" => self.op_setup(args)?,
                "payoff" => self.op_payoff(args)?,
                "ledger" => self.op_ledger(args)?,
                "chapter" => self.op_chapter(args)?,
                "reminisce" => self.op_reminisce(args)?,
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
                        ("chapters".to_string(), Value::from(self.limits.get("chapters") as u64)),
                        ("setups".to_string(), Value::from(self.limits.get("setups") as u64)),
                        ("storylets".to_string(), Value::from(self.limits.get("storylets") as u64)),
                    ]),
                ));
            }
        }
        Ok(out)
    }
}

impl Persist for NarrativeManager {
    fn save(&self) -> Value {
        let mut st_out = Vec::new();
        for (sid, s) in &self.storylets {
            let a_arr: Vec<Value> = s.actors.iter().cloned().map(Value::str).collect();
            st_out.push(Value::Array(vec![
                Value::Array(a_arr),
                Value::str(sid.clone()),
                Value::str(s.kind.clone()),
                Value::from(s.tension),
                Value::str(s.title.clone()),
            ]));
        }

        let mut setups_out = Vec::new();
        for (sid, su) in &self.setups {
            setups_out.push(Value::Array(vec![
                Value::str(su.description.clone()),
                Value::from(su.due_end),
                Value::from(su.due_start),
                Value::str(sid.clone()),
                Value::str(su.kind.clone()),
                Value::from(su.paid_at.unwrap_or(0)),
                Value::str(su.resolution.clone().unwrap_or_default()),
                Value::str(su.status.clone()),
                Value::str(su.target.clone().unwrap_or_default()),
            ]));
        }

        let mut bio_out = Vec::new();
        for (bid, chapters) in &self.autobiographies {
            let mut ch_list = Vec::new();
            for ch in chapters {
                ch_list.push(Value::Array(vec![
                    Value::from(ch.chapter),
                    Value::str(ch.summary.clone()),
                    Value::str(ch.theme.clone()),
                    Value::str(ch.turning_point.clone()),
                ]));
            }
            bio_out.push(Value::Array(vec![Value::str(bid.clone()), Value::Array(ch_list)]));
        }

        let b_arr: Vec<Value> = self.beings.iter().cloned().map(Value::str).collect();

        obj! {
            "autobiographies" => Value::Array(bio_out),
            "beings" => Value::Array(b_arr),
            "setups" => Value::Array(setups_out),
            "storylets" => Value::Array(st_out),
        }
    }

    fn load(v: &Value) -> Result<Self> {
        exact_fields(v, &["autobiographies", "beings", "setups", "storylets"])?;
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

        let st_list = c::list(c::get(v, "storylets")?)?;
        if st_list.len() > mgr.limits.get("storylets") {
            return Err(c::bad());
        }
        let mut new_storylets = BTreeMap::new();
        let mut last_s: Option<&str> = None;
        for srow_val in st_list {
            let srow = c::list(srow_val)?;
            if srow.len() != 5 {
                return Err(c::bad());
            }

            let raw_a = c::list(&srow[0])?;
            let mut actors = BTreeSet::new();
            let mut last_a: Option<&str> = None;
            for a_val in raw_a {
                let aid = ident(a_val)?;
                if !new_beings.contains(aid) {
                    return Err(c::bad());
                }
                if let Some(prev) = last_a {
                    if aid <= prev {
                        return Err(c::bad());
                    }
                }
                last_a = Some(aid);
                actors.insert(aid.to_string());
            }

            let sid = ident(&srow[1])?;
            if let Some(prev) = last_s {
                if sid <= prev {
                    return Err(c::bad());
                }
            }
            last_s = Some(sid);

            let kind = c::text(&srow[2])?.to_ascii_lowercase();
            if !STORYLET_KINDS.contains(&kind.as_str()) {
                return Err(c::bad());
            }

            let tension = c::uint(&srow[3], 0, 100)?;
            let title = c::text(&srow[4])?;
            if title.is_empty() || title.len() > 64 {
                return Err(c::bad());
            }

            new_storylets.insert(
                sid.to_string(),
                Storylet {
                    id: sid.to_string(),
                    kind,
                    title: title.to_string(),
                    tension,
                    actors,
                },
            );
        }

        let su_list = c::list(c::get(v, "setups")?)?;
        if su_list.len() > mgr.limits.get("setups") {
            return Err(c::bad());
        }
        let mut new_setups = BTreeMap::new();
        let mut last_su: Option<&str> = None;
        for su_row_val in su_list {
            let row = c::list(su_row_val)?;
            if row.len() != 9 {
                return Err(c::bad());
            }

            let desc = c::text(&row[0])?;
            if desc.is_empty() || desc.len() > 128 {
                return Err(c::bad());
            }

            let due_end = c::uint(&row[1], 0, 1_000_000_000)?;
            let due_start = c::uint(&row[2], 0, due_end)?;

            let su_id = ident(&row[3])?;
            if let Some(prev) = last_su {
                if su_id <= prev {
                    return Err(c::bad());
                }
            }
            last_su = Some(su_id);

            let kind = c::text(&row[4])?;
            if kind.is_empty() || kind.len() > 32 {
                return Err(c::bad());
            }

            let paid_at_num = c::uint(&row[5], 0, 1_000_000_000)?;
            let paid_at = if paid_at_num == 0 { None } else { Some(paid_at_num) };

            let res_raw = c::text(&row[6])?;
            let resolution = if res_raw.is_empty() { None } else { Some(res_raw.to_string()) };

            let status = c::text(&row[7])?.to_ascii_lowercase();
            if !SETUP_STATUSES.contains(&status.as_str()) {
                return Err(c::bad());
            }

            let tgt_raw = c::text(&row[8])?;
            let target = if tgt_raw.is_empty() {
                None
            } else {
                let tid = ident(&row[8])?;
                if !new_beings.contains(tid) {
                    return Err(c::bad());
                }
                Some(tid.to_string())
            };

            new_setups.insert(
                su_id.to_string(),
                Setup {
                    id: su_id.to_string(),
                    kind: kind.to_string(),
                    description: desc.to_string(),
                    due_start,
                    due_end,
                    status,
                    target,
                    paid_at,
                    resolution,
                },
            );
        }

        let bio_list = c::list(c::get(v, "autobiographies")?)?;
        let mut new_bios = BTreeMap::new();
        let mut last_bio_b: Option<&str> = None;
        let max_ch = mgr.limits.get("chapters");

        for bio_val in bio_list {
            let row = c::list(bio_val)?;
            if row.len() != 2 {
                return Err(c::bad());
            }

            let bid = ident(&row[0])?;
            if !new_beings.contains(bid) {
                return Err(c::bad());
            }
            if let Some(prev) = last_bio_b {
                if bid <= prev {
                    return Err(c::bad());
                }
            }
            last_bio_b = Some(bid);

            let raw_chs = c::list(&row[1])?;
            if raw_chs.len() > max_ch {
                return Err(c::bad());
            }
            let mut chapters = Vec::new();
            for (i, ch_val) in raw_chs.iter().enumerate() {
                let ch_row = c::list(ch_val)?;
                if ch_row.len() != 4 {
                    return Err(c::bad());
                }

                let ch_num = c::uint(&ch_row[0], 1, max_ch as u64)?;
                if ch_num != (i + 1) as u64 {
                    return Err(c::bad());
                }

                let summary = c::text(&ch_row[1])?;
                if summary.is_empty() || summary.len() > 128 {
                    return Err(c::bad());
                }

                let theme = c::text(&ch_row[2])?.to_ascii_lowercase();
                if !THEMES.contains(&theme.as_str()) {
                    return Err(c::bad());
                }

                let tp = c::text(&ch_row[3])?;
                if tp.is_empty() || tp.len() > 64 {
                    return Err(c::bad());
                }

                chapters.push(Chapter {
                    chapter: ch_num,
                    summary: summary.to_string(),
                    theme,
                    turning_point: tp.to_string(),
                });
            }
            new_bios.insert(bid.to_string(), chapters);
        }

        mgr.beings = new_beings;
        mgr.storylets = new_storylets;
        mgr.setups = new_setups;
        mgr.autobiographies = new_bios;
        mgr.settle_state()?;
        Ok(mgr)
    }
}

durable!(NarrativeManager, "narrative", LIMITS);

pub fn workflow(request: &Value) -> Result<Value> {
    NarrativeManager::new().workflow(request)
}
