//! Task and Planning module (Gap 4).
//!
//! A task is committed work owned by one being (or a group):
//! - `id`, `owner`, optional `assigned_by` and `for` (who benefits)
//! - `motive_ref`: the goal, need, promise or routine it serves
//! - `steps`: ordered list of action references (§5), with target bindings,
//!   preconditions and expected effects
//! - `status`: proposed, accepted, active, blocked (with reason), suspended,
//!   done, failed, abandoned
//! - `progress` per step, `deadline` and `priority` (0..=100)
//! - `reserved_resources`: items, stations, places locked
//! - `on_fail` / `on_block`: bounded recovery policy (replan, retry, ask_help, suspend, abandon)
//! - `replan_count` (<= 3), `retry_count` (<= 3)
//! - `history`: confirmed step outcomes (only confirmed outcomes advance a step)
//!
//! Host-owned rules:
//! 1. A model may propose a task or replan. The host validates it against the Action Catalog,
//!    target bindings, bounds, and causal conditions, and accepts or rejects it.
//! 2. Only confirmed outcomes advance a step.
//! 3. Deterministic planner generates causal step chains for item acquisition, recipes,
//!    travel and need satisfaction.

use std::collections::{BTreeMap, BTreeSet};

use crate::checked as c;
use crate::data::json::Value;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result};

pub const MAX_TASKS: usize = 64;
pub const MAX_STEPS: usize = 16;
pub const MAX_PLAN_DEPTH: usize = 8;
pub const MAX_REPLANS: u64 = 3;
pub const MAX_RETRIES: u64 = 3;
pub const MAX_RESERVED_RESOURCES: usize = 16;
pub const MAX_HISTORY_ENTRIES: usize = 32;
pub const MAX_OPS: usize = 64;

/// Host-set limits: `tasks`, `steps` per plan, `reserved` resources per task and `history`
/// entries kept per task (defaults are the `MAX_*` constants). Plan depth, retries and replans
/// stay fixed.
pub const LIMITS: &[crate::character::limits::Limit] = &[
    crate::character::limits::Limit { name: "tasks", default: MAX_TASKS, ceiling: 1_000_000 },
    crate::character::limits::Limit { name: "steps", default: MAX_STEPS, ceiling: 256 },
    crate::character::limits::Limit { name: "reserved", default: MAX_RESERVED_RESOURCES, ceiling: 256 },
    crate::character::limits::Limit { name: "history", default: MAX_HISTORY_ENTRIES, ceiling: 1024 },
];
crate::character::limits::module_limits!(TaskLimits, LIMITS);

pub const KNOWN_ACTIONS: [&str; 49] = [
    "activity",
    "add_member",
    "animal_train",
    "approach",
    "ask",
    "barter",
    "behave",
    "build",
    "calm",
    "can_carry",
    "check_relation",
    "cook",
    "craft",
    "create",
    "debt_query",
    "decline",
    "destroy",
    "dismantle",
    "enter",
    "feed",
    "fitness_train",
    "followup",
    "gesture",
    "gift",
    "harvest",
    "help",
    "lead",
    "load",
    "loan",
    "look",
    "move",
    "pet",
    "prop_interact",
    "remove_member",
    "repair",
    "repay",
    "ride",
    "service",
    "set_customs",
    "set_relation",
    "silence",
    "store",
    "take",
    "talk",
    "transfer",
    "travel",
    "travel_cost",
    "unload",
    "wait",
];

pub const VALID_STATUSES: [&str; 8] =
    ["proposed", "accepted", "active", "blocked", "suspended", "done", "failed", "abandoned"];

pub const VALID_POLICIES: [&str; 5] = ["replan", "retry", "ask_help", "suspend", "abandon"];

fn is_known_action(act: &str) -> bool {
    KNOWN_ACTIONS.contains(&act)
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

fn ident_bound(v: &Value) -> Result<&str> {
    let s = ident(v)?;
    if s.is_empty() || s.len() > 64 {
        return Err(c::bad());
    }
    Ok(s)
}

fn uint_val(v: &Value) -> Result<u64> {
    match v {
        Value::Number(s) => s.parse::<u64>().map_err(|_| c::bad()),
        _ => Err(c::bad()),
    }
}

/// A single step in a task.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskStep {
    pub id: String,
    pub action: String,
    pub target_bindings: Value,
    pub preconditions: Option<Value>,
    pub expected_effects: Option<Value>,
    pub status: String, // pending, active, done, failed, skipped
    pub estimated_duration: u64,
    pub estimated_fatigue: u64,
}

impl TaskStep {
    pub fn to_value(&self) -> Value {
        obj! {
            "id" => Value::str(&self.id),
            "action" => Value::str(&self.action),
            "target_bindings" => self.target_bindings.clone(),
            "preconditions" => self.preconditions.clone().unwrap_or(Value::Null),
            "expected_effects" => self.expected_effects.clone().unwrap_or(Value::Null),
            "status" => Value::str(&self.status),
            "estimated_duration" => Value::int(self.estimated_duration as i64),
            "estimated_fatigue" => Value::int(self.estimated_fatigue as i64)
        }
    }
}

/// A confirmed outcome recorded in task history.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskHistoryEntry {
    pub step_id: String,
    pub action: String,
    pub success: bool,
    pub outcome: Value,
    pub tick: Option<u64>,
}

impl TaskHistoryEntry {
    pub fn to_value(&self) -> Value {
        obj! {
            "step_id" => Value::str(&self.step_id),
            "action" => Value::str(&self.action),
            "success" => Value::Bool(self.success),
            "outcome" => self.outcome.clone(),
            "tick" => self.tick.map(|t| Value::int(t as i64)).unwrap_or(Value::Null)
        }
    }
}

/// A task object.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: String,
    pub owner: String,
    pub assigned_by: Option<String>,
    pub assigned_for: Option<String>,
    pub motive_ref: Option<Value>,
    pub steps: Vec<TaskStep>,
    pub status: String,
    pub blocked_reason: Option<String>,
    pub progress: usize,
    pub deadline: Option<u64>,
    pub priority: u64,
    pub reserved_resources: Vec<String>,
    pub on_fail: String,
    pub on_block: String,
    pub replan_count: u64,
    pub retry_count: u64,
    pub history: Vec<TaskHistoryEntry>,
}

impl Task {
    pub fn to_value(&self) -> Value {
        let steps_val = Value::Array(self.steps.iter().map(TaskStep::to_value).collect());
        let hist_val = Value::Array(self.history.iter().map(TaskHistoryEntry::to_value).collect());
        let res_val = Value::Array(self.reserved_resources.iter().map(Value::str).collect());

        obj! {
            "id" => Value::str(&self.id),
            "owner" => Value::str(&self.owner),
            "assigned_by" => self.assigned_by.as_deref().map(Value::str).unwrap_or(Value::Null),
            "for" => self.assigned_for.as_deref().map(Value::str).unwrap_or(Value::Null),
            "motive_ref" => self.motive_ref.clone().unwrap_or(Value::Null),
            "steps" => steps_val,
            "status" => Value::str(&self.status),
            "blocked_reason" => self.blocked_reason.as_deref().map(Value::str).unwrap_or(Value::Null),
            "progress" => Value::int(self.progress as i64),
            "deadline" => self.deadline.map(|d| Value::int(d as i64)).unwrap_or(Value::Null),
            "priority" => Value::int(self.priority as i64),
            "reserved_resources" => res_val,
            "on_fail" => Value::str(&self.on_fail),
            "on_block" => Value::str(&self.on_block),
            "replan_count" => Value::int(self.replan_count as i64),
            "retry_count" => Value::int(self.retry_count as i64),
            "history" => hist_val
        }
    }
}

/// Task manager and deterministic planner.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskManager {
    pub tasks: BTreeMap<String, Task>,
    /// Host-set capacity limits (`{ops, limits?}` or `with_limits`).
    pub limits: TaskLimits,
}

impl TaskManager {
    pub fn new() -> Self {
        Self::default()
    }

    crate::character::limits::limited_workflow!();

    fn run_workflow(&mut self, a: &Value) -> Result<Value> {
        keys(a, &["ops"], &[])?;
        let ops = c::get(a, "ops")?.as_array().ok_or_else(c::bad)?;
        if ops.is_empty() || ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        let mut results = Vec::new();
        for op_spec in ops {
            keys(op_spec, &["op"], &["args"])?;
            let op = ident(c::get(op_spec, "op")?)?;
            let args = op_spec.get("args").cloned().unwrap_or(Value::Object(Vec::new()));

            let res = match op {
                "propose" => self.op_propose(&args)?,
                "accept" => self.op_accept(&args)?,
                "advance" => self.op_advance(&args)?,
                "block" => self.op_block(&args)?,
                "replan" => self.op_replan(&args)?,
                "suspend" => self.op_suspend(&args)?,
                "resume" => self.op_resume(&args)?,
                "abandon" => self.op_abandon(&args)?,
                "get" => self.op_get(&args)?,
                "list" => self.op_list(&args)?,
                "validate" => self.op_validate(&args)?,
                _ => return Err(c::bad()),
            };
            results.push(res);
        }

        Ok(obj! {
            "results" => Value::Array(results),
            "total_tasks" => Value::int(self.tasks.len() as i64)
        })
    }

    fn op_propose(&mut self, args: &Value) -> Result<Value> {
        keys(
            args,
            &["id", "owner"],
            &[
                "assigned_by",
                "for",
                "motive_ref",
                "priority",
                "deadline",
                "reserved_resources",
                "on_fail",
                "on_block",
                "steps",
                "goal",
                "world_state",
            ],
        )?;

        let tid = ident_bound(c::get(args, "id")?)?.to_string();
        let owner = ident_bound(c::get(args, "owner")?)?.to_string();

        if self.tasks.len() >= self.limits.get("tasks") && !self.tasks.contains_key(&tid) {
            return Err(c::bad());
        }

        let priority = args.get("priority").and_then(|v| uint_val(v).ok()).unwrap_or(50).min(100);

        let on_fail = args.get("on_fail").map(|v| text_bound(v, 32)).transpose()?.unwrap_or("replan");
        if !VALID_POLICIES.contains(&on_fail) {
            return Err(c::bad());
        }

        let on_block = args.get("on_block").map(|v| text_bound(v, 32)).transpose()?.unwrap_or("replan");
        if !VALID_POLICIES.contains(&on_block) {
            return Err(c::bad());
        }

        let assigned_by = args.get("assigned_by").map(|v| ident_bound(v).map(|s| s.to_string())).transpose()?;
        let assigned_for = args.get("for").map(|v| ident_bound(v).map(|s| s.to_string())).transpose()?;
        let motive_ref = args.get("motive_ref").cloned();
        let deadline = args.get("deadline").and_then(|v| uint_val(v).ok());

        let mut reserved_resources = Vec::new();
        if let Some(r_val) = args.get("reserved_resources") {
            let arr = r_val.as_array().ok_or_else(c::bad)?;
            if arr.len() > self.limits.get("reserved") {
                return Err(c::bad());
            }
            for it in arr {
                reserved_resources.push(text_bound(it, 64)?.to_string());
            }
        }

        let empty_world = Value::Object(Vec::new());
        let world_state = args.get("world_state").unwrap_or(&empty_world);

        let steps_raw = args.get("steps");
        let goal = args.get("goal");

        let steps = if let Some(s_val) = steps_raw {
            let (valid, reason, errors, s_res) =
                Self::validate_steps_within(s_val, &owner, world_state, self.limits.get("steps"));
            if !valid {
                return Ok(obj! {
                    "accepted" => Value::Bool(false),
                    "task_id" => Value::str(&tid),
                    "reason" => Value::str(&reason),
                    "errors" => Value::Array(errors.into_iter().map(|e| Value::str(&e)).collect())
                });
            }
            s_res
        } else if let Some(g_val) = goal {
            let (success, reason, s_res) = Self::plan_deterministic(&owner, g_val, world_state);
            if !success {
                return Ok(obj! {
                    "accepted" => Value::Bool(false),
                    "task_id" => Value::str(&tid),
                    "reason" => Value::str(&reason),
                    "errors" => Value::Array(vec![Value::str(&reason)])
                });
            }
            s_res
        } else {
            return Err(c::bad());
        };

        let task = Task {
            id: tid.clone(),
            owner,
            assigned_by,
            assigned_for,
            motive_ref,
            steps,
            status: "proposed".to_string(),
            blocked_reason: None,
            progress: 0,
            deadline,
            priority,
            reserved_resources,
            on_fail: on_fail.to_string(),
            on_block: on_block.to_string(),
            replan_count: 0,
            retry_count: 0,
            history: Vec::new(),
        };

        let t_val = task.to_value();
        self.tasks.insert(tid.clone(), task);

        Ok(obj! {
            "accepted" => Value::Bool(true),
            "task_id" => Value::str(&tid),
            "task" => t_val
        })
    }

    fn op_accept(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["id"], &["start"])?;
        let tid = ident_bound(c::get(args, "id")?)?;
        let task = self.tasks.get_mut(tid).ok_or_else(c::bad)?;

        if task.status != "proposed" && task.status != "suspended" {
            return Err(c::bad());
        }

        let start = args.get("start").and_then(Value::as_bool).unwrap_or(true);
        if start {
            task.status = "active".to_string();
            if let Some(first) = task.steps.first_mut() {
                first.status = "active".to_string();
            }
        } else {
            task.status = "accepted".to_string();
        }

        Ok(obj! {
            "accepted" => Value::Bool(true),
            "task_id" => Value::str(tid),
            "status" => Value::str(&task.status),
            "task" => task.to_value()
        })
    }

    fn op_advance(&mut self, args: &Value) -> Result<Value> {
        let max_history = self.limits.get("history");
        keys(args, &["id", "outcome"], &["step_id", "tick"])?;
        let tid = ident_bound(c::get(args, "id")?)?;
        let outcome = c::get(args, "outcome")?;
        let success = outcome.get("success").and_then(Value::as_bool).ok_or_else(c::bad)?;
        let tick = args.get("tick").and_then(|v| uint_val(v).ok());

        let task = self.tasks.get_mut(tid).ok_or_else(c::bad)?;
        if task.status != "active" {
            return Err(c::bad());
        }

        if task.progress >= task.steps.len() {
            task.status = "done".to_string();
            return Ok(obj! {
                "advanced" => Value::Bool(true),
                "task_id" => Value::str(tid),
                "task_status" => Value::str("done"),
                "next_step" => Value::Null,
                "task" => task.to_value()
            });
        }

        let cur_step = &task.steps[task.progress];
        let hist_entry = TaskHistoryEntry {
            step_id: cur_step.id.clone(),
            action: cur_step.action.clone(),
            success,
            outcome: outcome.clone(),
            tick,
        };
        if task.history.len() < max_history {
            task.history.push(hist_entry);
        }

        if success {
            task.steps[task.progress].status = "done".to_string();
            task.retry_count = 0;
            task.progress += 1;

            if task.progress >= task.steps.len() {
                task.status = "done".to_string();
                Ok(obj! {
                    "advanced" => Value::Bool(true),
                    "task_id" => Value::str(tid),
                    "task_status" => Value::str("done"),
                    "next_step" => Value::Null,
                    "task" => task.to_value()
                })
            } else {
                task.steps[task.progress].status = "active".to_string();
                let next_id = task.steps[task.progress].id.clone();
                Ok(obj! {
                    "advanced" => Value::Bool(true),
                    "task_id" => Value::str(tid),
                    "task_status" => Value::str("active"),
                    "next_step" => Value::str(&next_id),
                    "task" => task.to_value()
                })
            }
        } else {
            task.steps[task.progress].status = "failed".to_string();
            let policy = task.on_fail.clone();

            if policy == "retry" && task.retry_count < MAX_RETRIES {
                task.retry_count += 1;
                task.steps[task.progress].status = "active".to_string();
                Ok(obj! {
                    "advanced" => Value::Bool(false),
                    "task_id" => Value::str(tid),
                    "action" => Value::str("retrying"),
                    "task_status" => Value::str("active"),
                    "retry_count" => Value::int(task.retry_count as i64),
                    "task" => task.to_value()
                })
            } else if policy == "replan" && task.replan_count < MAX_REPLANS {
                task.status = "blocked".to_string();
                let reason = outcome.get("reason").and_then(Value::as_str).unwrap_or("step_failed");
                task.blocked_reason = Some(reason.to_string());
                Ok(obj! {
                    "advanced" => Value::Bool(false),
                    "task_id" => Value::str(tid),
                    "action" => Value::str("blocked_for_replan"),
                    "task_status" => Value::str("blocked"),
                    "task" => task.to_value()
                })
            } else if policy == "suspend" {
                task.status = "suspended".to_string();
                Ok(obj! {
                    "advanced" => Value::Bool(false),
                    "task_id" => Value::str(tid),
                    "action" => Value::str("suspended"),
                    "task_status" => Value::str("suspended"),
                    "task" => task.to_value()
                })
            } else {
                task.status = "failed".to_string();
                Ok(obj! {
                    "advanced" => Value::Bool(false),
                    "task_id" => Value::str(tid),
                    "action" => Value::str("failed"),
                    "task_status" => Value::str("failed"),
                    "task" => task.to_value()
                })
            }
        }
    }

    fn op_block(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["id"], &["reason"])?;
        let tid = ident_bound(c::get(args, "id")?)?;
        let task = self.tasks.get_mut(tid).ok_or_else(c::bad)?;
        let reason = args.get("reason").and_then(Value::as_str).unwrap_or("unspecified_block").to_string();

        task.status = "blocked".to_string();
        task.blocked_reason = Some(reason.clone());

        Ok(obj! {
            "blocked" => Value::Bool(true),
            "task_id" => Value::str(tid),
            "task_status" => Value::str("blocked"),
            "reason" => Value::str(&reason),
            "task" => task.to_value()
        })
    }

    fn op_replan(&mut self, args: &Value) -> Result<Value> {
        let max_steps = self.limits.get("steps");
        keys(args, &["id"], &["steps", "goal", "world_state"])?;
        let tid = ident_bound(c::get(args, "id")?)?;
        let task = self.tasks.get_mut(tid).ok_or_else(c::bad)?;

        if task.replan_count >= MAX_REPLANS {
            task.status = "failed".to_string();
            return Ok(obj! {
                "replanned" => Value::Bool(false),
                "task_id" => Value::str(tid),
                "reason" => Value::str("replan_limit_exceeded"),
                "task_status" => Value::str("failed"),
                "task" => task.to_value()
            });
        }

        let empty_world = Value::Object(Vec::new());
        let world_state = args.get("world_state").unwrap_or(&empty_world);

        let steps_raw = args.get("steps");
        let goal = args.get("goal");

        let mut new_steps = if let Some(s_val) = steps_raw {
            let (valid, reason, errors, s_res) =
                Self::validate_steps_within(s_val, &task.owner, world_state, max_steps);
            if !valid {
                return Ok(obj! {
                    "replanned" => Value::Bool(false),
                    "task_id" => Value::str(tid),
                    "reason" => Value::str(&reason),
                    "errors" => Value::Array(errors.into_iter().map(|e| Value::str(&e)).collect()),
                    "task" => task.to_value()
                });
            }
            s_res
        } else if let Some(g_val) = goal {
            let (success, reason, s_res) = Self::plan_deterministic(&task.owner, g_val, world_state);
            if !success {
                return Ok(obj! {
                    "replanned" => Value::Bool(false),
                    "task_id" => Value::str(tid),
                    "reason" => Value::str(&reason),
                    "errors" => Value::Array(vec![Value::str(&reason)]),
                    "task" => task.to_value()
                });
            }
            s_res
        } else {
            return Err(c::bad());
        };

        let mut completed: Vec<TaskStep> = task.steps.iter().filter(|s| s.status == "done").cloned().collect();
        if completed.len() + new_steps.len() > max_steps {
            return Err(c::bad());
        }

        if let Some(first) = new_steps.first_mut() {
            first.status = "active".to_string();
        }

        task.progress = completed.len();
        completed.extend(new_steps);
        task.steps = completed;
        task.status = "active".to_string();
        task.blocked_reason = None;
        task.replan_count += 1;
        task.retry_count = 0;

        Ok(obj! {
            "replanned" => Value::Bool(true),
            "task_id" => Value::str(tid),
            "replan_count" => Value::int(task.replan_count as i64),
            "task_status" => Value::str("active"),
            "task" => task.to_value()
        })
    }

    fn op_suspend(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["id"], &[])?;
        let tid = ident_bound(c::get(args, "id")?)?;
        let task = self.tasks.get_mut(tid).ok_or_else(c::bad)?;
        task.status = "suspended".to_string();
        Ok(obj! {
            "suspended" => Value::Bool(true),
            "task_id" => Value::str(tid),
            "task_status" => Value::str("suspended"),
            "task" => task.to_value()
        })
    }

    fn op_resume(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["id"], &[])?;
        let tid = ident_bound(c::get(args, "id")?)?;
        let task = self.tasks.get_mut(tid).ok_or_else(c::bad)?;
        task.status = "active".to_string();
        Ok(obj! {
            "resumed" => Value::Bool(true),
            "task_id" => Value::str(tid),
            "task_status" => Value::str("active"),
            "task" => task.to_value()
        })
    }

    fn op_abandon(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["id"], &["reason"])?;
        let tid = ident_bound(c::get(args, "id")?)?;
        let task = self.tasks.get_mut(tid).ok_or_else(c::bad)?;
        let reason = args.get("reason").and_then(Value::as_str).unwrap_or("abandoned_by_actor").to_string();

        task.status = "abandoned".to_string();
        task.blocked_reason = Some(reason);

        Ok(obj! {
            "abandoned" => Value::Bool(true),
            "task_id" => Value::str(tid),
            "task_status" => Value::str("abandoned"),
            "task" => task.to_value()
        })
    }

    fn op_get(&self, args: &Value) -> Result<Value> {
        keys(args, &["id"], &[])?;
        let tid = ident_bound(c::get(args, "id")?)?;
        let task = self.tasks.get(tid).ok_or_else(c::bad)?;
        Ok(obj! {
            "task" => task.to_value()
        })
    }

    fn op_list(&self, args: &Value) -> Result<Value> {
        keys(args, &[], &["owner", "status"])?;
        let owner = args.get("owner").and_then(Value::as_str);
        let status = args.get("status").and_then(Value::as_str);

        let mut list = Vec::new();
        for t in self.tasks.values() {
            if let Some(o) = owner {
                if t.owner != o {
                    continue;
                }
            }
            if let Some(s) = status {
                if t.status != s {
                    continue;
                }
            }
            list.push(t.to_value());
        }

        Ok(obj! {
            "tasks" => Value::Array(list)
        })
    }

    fn op_validate(&self, args: &Value) -> Result<Value> {
        keys(args, &["steps"], &["owner", "world_state"])?;
        let steps_raw = c::get(args, "steps")?;
        let owner = args.get("owner").and_then(Value::as_str).unwrap_or("unknown");
        let empty_world = Value::Object(Vec::new());
        let world_state = args.get("world_state").unwrap_or(&empty_world);

        let (valid, reason, errors, _) =
            Self::validate_steps_within(steps_raw, owner, world_state, self.limits.get("steps"));
        let count = steps_raw.as_array().map(|a| a.len()).unwrap_or(0);

        Ok(obj! {
            "valid" => Value::Bool(valid),
            "reason" => if valid { Value::Null } else { Value::str(&reason) },
            "errors" => Value::Array(errors.into_iter().map(|e| Value::str(&e)).collect()),
            "step_count" => Value::int(count as i64)
        })
    }

    pub fn validate_steps(
        steps_raw: &Value,
        owner: &str,
        world_state: &Value,
    ) -> (bool, String, Vec<String>, Vec<TaskStep>) {
        Self::validate_steps_within(steps_raw, owner, world_state, MAX_STEPS)
    }

    /// [`validate_steps`](Self::validate_steps) with a host-set step limit.
    pub fn validate_steps_within(
        steps_raw: &Value,
        _owner: &str,
        world_state: &Value,
        max_steps: usize,
    ) -> (bool, String, Vec<String>, Vec<TaskStep>) {
        let arr = match steps_raw.as_array() {
            Some(a) => a,
            None => {
                return (false, "steps_not_a_list".to_string(), vec!["steps must be an array".to_string()], Vec::new());
            }
        };

        if arr.is_empty() {
            return (false, "empty_steps".to_string(), vec!["plan cannot be empty".to_string()], Vec::new());
        }

        if arr.len() > max_steps {
            return (
                false,
                "too_many_steps".to_string(),
                vec![format!("step count {} > {}", arr.len(), max_steps)],
                Vec::new(),
            );
        }

        let mut errors = Vec::new();
        let mut cleaned_steps = Vec::new();
        let mut seen_ids = BTreeSet::new();

        let mut sim_location = world_state.get("location").and_then(Value::as_str).map(String::from);
        let mut sim_inventory: BTreeMap<String, i64> = BTreeMap::new();
        if let Some(inv_obj) = world_state.get("inventory").and_then(Value::as_object) {
            for (k, v) in inv_obj {
                let cnt = v.as_usize().map(|n| n as i64).unwrap_or(0);
                sim_inventory.insert(k.clone(), cnt);
            }
        }

        for (idx, s) in arr.iter().enumerate() {
            if s.as_object().is_none() {
                errors.push(format!("step[{idx}] must be an object"));
                continue;
            }

            let default_sid = format!("step_{}", idx + 1);
            let sid = s.get("id").and_then(Value::as_str).unwrap_or(&default_sid);
            if sid.is_empty() || sid.len() > 64 {
                errors.push(format!("step[{idx}] invalid step id: {sid}"));
            }
            if seen_ids.contains(sid) {
                errors.push(format!("step[{idx}] duplicate step id: {sid}"));
            }
            seen_ids.insert(sid.to_string());

            let action = match s.get("action").and_then(Value::as_str) {
                Some(a) => a,
                None => {
                    errors.push(format!("step[{idx}] missing action"));
                    continue;
                }
            };

            if !is_known_action(action) {
                errors.push(format!("step[{idx}] unknown action catalog verb: {action}"));
                continue;
            }

            let bindings = match s.get("target_bindings") {
                Some(b) if b.as_object().is_some() => b.clone(),
                None => Value::Object(Vec::new()),
                _ => {
                    errors.push(format!("step[{idx}] target_bindings must be an object"));
                    continue;
                }
            };

            // Check required slots
            match action {
                "travel" => {
                    if bindings.get("destination").and_then(Value::as_str).is_none() {
                        errors.push(format!("step[{idx}] action 'travel' missing required target slot 'destination'"));
                    }
                }
                "cook" | "craft" => {
                    if bindings.get("recipe").and_then(Value::as_str).is_none() {
                        errors.push(format!("step[{idx}] action '{action}' missing required target slot 'recipe'"));
                    }
                }
                "barter" => {
                    for slot in &["partner", "offer", "want"] {
                        if bindings.get(slot).and_then(Value::as_str).is_none() {
                            errors.push(format!("step[{idx}] action 'barter' missing required target slot '{slot}'"));
                        }
                    }
                }
                _ => {}
            }

            // Causal checks
            if action == "travel" {
                if let Some(dest) = bindings.get("destination").and_then(Value::as_str) {
                    sim_location = Some(dest.to_string());
                }
            } else if action == "cook" || action == "craft" {
                if let Some(recipe_name) = bindings.get("recipe").and_then(Value::as_str) {
                    if let Some(recipes) = world_state.get("recipes").and_then(Value::as_object) {
                        if let Some(rinfo) = recipes.iter().find(|(k, _)| k == recipe_name).map(|(_, v)| v) {
                            if let Some(req_station) = rinfo.get("station").and_then(Value::as_str) {
                                if let Some(ref cur_loc) = sim_location {
                                    if cur_loc != req_station {
                                        errors.push(format!(
                                            "step[{idx}] action '{action}' requires station at '{req_station}', but actor is at '{cur_loc}'"
                                        ));
                                    }
                                }
                            }
                            if let Some(inputs) = rinfo.get("inputs").and_then(Value::as_array) {
                                for inp in inputs {
                                    if let Some(item) = inp.get("item").and_then(Value::as_str) {
                                        let cnt =
                                            inp.get("count").and_then(|v| v.as_usize().map(|n| n as i64)).unwrap_or(1);
                                        let avail = *sim_inventory.get(item).unwrap_or(&0);
                                        if avail < cnt {
                                            errors.push(format!(
                                                "step[{idx}] action '{action}' missing input ingredient '{item}' ({avail} < {cnt})"
                                            ));
                                        } else {
                                            sim_inventory.insert(item.to_string(), avail - cnt);
                                        }
                                    }
                                }
                            }
                            if let Some(outputs) = rinfo.get("outputs").and_then(Value::as_array) {
                                for out in outputs {
                                    if let Some(item) = out.get("item").and_then(Value::as_str) {
                                        let cnt =
                                            out.get("count").and_then(|v| v.as_usize().map(|n| n as i64)).unwrap_or(1);
                                        let curr = *sim_inventory.get(item).unwrap_or(&0);
                                        sim_inventory.insert(item.to_string(), curr + cnt);
                                    }
                                }
                            }
                        }
                    }
                }
            } else if action == "barter" {
                let offer = bindings.get("offer").and_then(Value::as_str);
                let want = bindings.get("want").and_then(Value::as_str);
                if let Some(off) = offer {
                    let avail = *sim_inventory.get(off).unwrap_or(&0);
                    if avail <= 0 {
                        errors.push(format!("step[{idx}] action 'barter' cannot offer '{off}', actor has none"));
                    } else {
                        sim_inventory.insert(off.to_string(), avail - 1);
                    }
                }
                if let Some(w) = want {
                    let curr = *sim_inventory.get(w).unwrap_or(&0);
                    sim_inventory.insert(w.to_string(), curr + 1);
                }
            }

            let preconds = s.get("preconditions").cloned();
            let effects = s.get("expected_effects").cloned();
            let est_dur = s.get("estimated_duration").and_then(|v| uint_val(v).ok()).unwrap_or(1);
            let est_fat = s.get("estimated_fatigue").and_then(|v| uint_val(v).ok()).unwrap_or(1);

            cleaned_steps.push(TaskStep {
                id: sid.to_string(),
                action: action.to_string(),
                target_bindings: bindings,
                preconditions: preconds,
                expected_effects: effects,
                status: "pending".to_string(),
                estimated_duration: est_dur,
                estimated_fatigue: est_fat,
            });
        }

        if !errors.is_empty() {
            (false, "step_validation_failed".to_string(), errors, Vec::new())
        } else {
            (true, String::new(), Vec::new(), cleaned_steps)
        }
    }

    pub fn plan_deterministic(_owner: &str, goal: &Value, world_state: &Value) -> (bool, String, Vec<TaskStep>) {
        let gkind = match goal.get("kind").and_then(Value::as_str) {
            Some(k) => k,
            None => return (false, "invalid_goal_object".to_string(), Vec::new()),
        };

        let mut cur_location = world_state.get("location").and_then(Value::as_str).unwrap_or("home").to_string();
        let mut inventory: BTreeMap<String, i64> = BTreeMap::new();
        if let Some(inv_obj) = world_state.get("inventory").and_then(Value::as_object) {
            for (k, v) in inv_obj {
                let cnt = v.as_usize().map(|n| n as i64).unwrap_or(0);
                inventory.insert(k.clone(), cnt);
            }
        }

        let recipes = world_state.get("recipes").and_then(Value::as_object);
        let sources = world_state.get("sources").and_then(Value::as_array);

        let mut steps = Vec::new();
        let mut step_num = 1;

        match gkind {
            "cook" | "craft" => {
                let recipe_name = match goal.get("recipe").and_then(Value::as_str) {
                    Some(r) => r,
                    None => return (false, "missing_recipe".to_string(), Vec::new()),
                };

                let rinfo = match recipes.and_then(|ro| ro.iter().find(|(k, _)| k == recipe_name).map(|(_, v)| v)) {
                    Some(r) => r,
                    None => return (false, format!("unknown_recipe:{recipe_name}"), Vec::new()),
                };

                let station = rinfo.get("station").and_then(Value::as_str);
                let inputs = rinfo.get("inputs").and_then(Value::as_array);

                let mut missing_items = Vec::new();
                if let Some(inp_arr) = inputs {
                    for inp in inp_arr {
                        if let Some(item) = inp.get("item").and_then(Value::as_str) {
                            let needed = inp.get("count").and_then(|v| v.as_usize().map(|n| n as i64)).unwrap_or(1);
                            let avail = *inventory.get(item).unwrap_or(&0);
                            if avail < needed {
                                missing_items.push((item.to_string(), needed - avail));
                            }
                        }
                    }
                }

                // Acquire missing items
                for (item, deficit) in missing_items {
                    let source =
                        sources.and_then(|sa| sa.iter().find(|s| s.get("item").and_then(Value::as_str) == Some(&item)));

                    let s_obj = match source {
                        Some(s) => s,
                        None => return (false, format!("no_source_for:{item}"), Vec::new()),
                    };

                    let source_place = s_obj.get("place").and_then(Value::as_str);
                    if let Some(sp) = source_place {
                        if cur_location != sp {
                            steps.push(TaskStep {
                                id: format!("step_{step_num}"),
                                action: "travel".to_string(),
                                target_bindings: obj! {"destination" => Value::str(sp)},
                                preconditions: None,
                                expected_effects: None,
                                status: "pending".to_string(),
                                estimated_duration: 5,
                                estimated_fatigue: 2,
                            });
                            step_num += 1;
                            cur_location = sp.to_string();
                        }
                    }

                    let partner = s_obj.get("partner").and_then(Value::as_str).unwrap_or("merchant");
                    let cost_item = s_obj.get("cost_item").and_then(Value::as_str).unwrap_or("coins");

                    steps.push(TaskStep {
                        id: format!("step_{step_num}"),
                        action: "barter".to_string(),
                        target_bindings: obj! {
                            "partner" => Value::str(partner),
                            "offer" => Value::str(cost_item),
                            "want" => Value::str(&item)
                        },
                        preconditions: None,
                        expected_effects: None,
                        status: "pending".to_string(),
                        estimated_duration: 2,
                        estimated_fatigue: 1,
                    });
                    step_num += 1;
                    let curr = *inventory.get(&item).unwrap_or(&0);
                    inventory.insert(item, curr + deficit);
                }

                // Travel to station if required
                if let Some(st) = station {
                    if cur_location != st {
                        steps.push(TaskStep {
                            id: format!("step_{step_num}"),
                            action: "travel".to_string(),
                            target_bindings: obj! {"destination" => Value::str(st)},
                            preconditions: None,
                            expected_effects: None,
                            status: "pending".to_string(),
                            estimated_duration: 5,
                            estimated_fatigue: 2,
                        });
                        step_num += 1;
                    }
                }

                steps.push(TaskStep {
                    id: format!("step_{step_num}"),
                    action: gkind.to_string(),
                    target_bindings: obj! {"recipe" => Value::str(recipe_name)},
                    preconditions: None,
                    expected_effects: None,
                    status: "pending".to_string(),
                    estimated_duration: 10,
                    estimated_fatigue: 3,
                });

                (true, String::new(), steps)
            }
            "travel" => {
                let dest = match goal.get("destination").and_then(Value::as_str) {
                    Some(d) => d,
                    None => return (false, "missing_destination".to_string(), Vec::new()),
                };

                if cur_location != dest {
                    steps.push(TaskStep {
                        id: format!("step_{step_num}"),
                        action: "travel".to_string(),
                        target_bindings: obj! {"destination" => Value::str(dest)},
                        preconditions: None,
                        expected_effects: None,
                        status: "pending".to_string(),
                        estimated_duration: 5,
                        estimated_fatigue: 2,
                    });
                }
                (true, String::new(), steps)
            }
            "satisfy_need" => {
                let need = match goal.get("need").and_then(Value::as_str) {
                    Some(n) => n,
                    None => return (false, "missing_need".to_string(), Vec::new()),
                };

                if need == "hunger" {
                    let food_item = goal.get("item").and_then(Value::as_str).unwrap_or("bread");
                    let avail = *inventory.get(food_item).unwrap_or(&0);
                    if avail > 0 {
                        steps.push(TaskStep {
                            id: format!("step_{step_num}"),
                            action: "activity".to_string(),
                            target_bindings: obj! {"name" => Value::str(format!("eat_{food_item}"))},
                            preconditions: None,
                            expected_effects: None,
                            status: "pending".to_string(),
                            estimated_duration: 2,
                            estimated_fatigue: 0,
                        });
                    } else {
                        let pantry = goal.get("pantry").and_then(Value::as_str).unwrap_or("pantry");
                        steps.push(TaskStep {
                            id: format!("step_{step_num}"),
                            action: "take".to_string(),
                            target_bindings: obj! {"item" => Value::str(food_item), "container" => Value::str(pantry)},
                            preconditions: None,
                            expected_effects: None,
                            status: "pending".to_string(),
                            estimated_duration: 1,
                            estimated_fatigue: 0,
                        });
                        step_num += 1;
                        steps.push(TaskStep {
                            id: format!("step_{step_num}"),
                            action: "activity".to_string(),
                            target_bindings: obj! {"name" => Value::str(format!("eat_{food_item}"))},
                            preconditions: None,
                            expected_effects: None,
                            status: "pending".to_string(),
                            estimated_duration: 2,
                            estimated_fatigue: 0,
                        });
                    }
                    (true, String::new(), steps)
                } else if need == "rest" {
                    let home = goal.get("place").and_then(Value::as_str).unwrap_or("home");
                    if cur_location != home {
                        steps.push(TaskStep {
                            id: format!("step_{step_num}"),
                            action: "travel".to_string(),
                            target_bindings: obj! {"destination" => Value::str(home)},
                            preconditions: None,
                            expected_effects: None,
                            status: "pending".to_string(),
                            estimated_duration: 5,
                            estimated_fatigue: 1,
                        });
                        step_num += 1;
                    }
                    steps.push(TaskStep {
                        id: format!("step_{step_num}"),
                        action: "wait".to_string(),
                        target_bindings: obj! {"duration" => Value::str("long")},
                        preconditions: None,
                        expected_effects: None,
                        status: "pending".to_string(),
                        estimated_duration: 30,
                        estimated_fatigue: 0,
                    });
                    (true, String::new(), steps)
                } else {
                    (false, format!("unhandled_need:{need}"), Vec::new())
                }
            }
            "barter" => {
                let partner = match goal.get("partner").and_then(Value::as_str) {
                    Some(p) => p,
                    None => return (false, "missing_barter_parameters".to_string(), Vec::new()),
                };
                let offer = match goal.get("offer").and_then(Value::as_str) {
                    Some(o) => o,
                    None => return (false, "missing_barter_parameters".to_string(), Vec::new()),
                };
                let want = match goal.get("want").and_then(Value::as_str) {
                    Some(w) => w,
                    None => return (false, "missing_barter_parameters".to_string(), Vec::new()),
                };
                let place = goal.get("place").and_then(Value::as_str);

                if let Some(pl) = place {
                    if cur_location != pl {
                        steps.push(TaskStep {
                            id: format!("step_{step_num}"),
                            action: "travel".to_string(),
                            target_bindings: obj! {"destination" => Value::str(pl)},
                            preconditions: None,
                            expected_effects: None,
                            status: "pending".to_string(),
                            estimated_duration: 5,
                            estimated_fatigue: 2,
                        });
                        step_num += 1;
                    }
                }

                steps.push(TaskStep {
                    id: format!("step_{step_num}"),
                    action: "barter".to_string(),
                    target_bindings: obj! {
                        "partner" => Value::str(partner),
                        "offer" => Value::str(offer),
                        "want" => Value::str(want)
                    },
                    preconditions: None,
                    expected_effects: None,
                    status: "pending".to_string(),
                    estimated_duration: 2,
                    estimated_fatigue: 1,
                });

                (true, String::new(), steps)
            }
            _ => (false, format!("unsupported_goal_kind:{gkind}"), Vec::new()),
        }
    }
}

pub fn workflow(a: &Value) -> Result<Value> {
    let mut manager = TaskManager::new();
    manager.workflow(a)
}
