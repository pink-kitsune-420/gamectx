//! Witness bridge: turns timeline entries and module outcomes into Life experiences.
//!
//! A timeline entry (or outcome from recipes, species, barter, things) plus its witnesses
//! plus each being's senses produces one `direct` Life experience per perceiving witness;
//! others get a `report` experience only when told by someone who already knows.
//! Deterministic and bounded. Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::life::witness::WitnessBridge;
//!
//! let mut bridge = WitnessBridge::new();
//! let req = parse(r#"{
//!   "ops": [
//!     {
//!       "op": "witness",
//!       "args": {
//!         "entry": {
//!           "at": 100,
//!           "kind": "crash",
//!           "summary": "A grain cart overturned in the road.",
//!           "place": "road",
//!           "actors": ["carter"],
//!           "visibility": "witnessed",
//!           "noise": 40
//!         },
//!         "beings": [
//!           {"id": "baker", "place": "road", "senses": {"sight": true, "hearing": true}, "token": 101},
//!           {"id": "apprentice", "place": "mill", "senses": {"sight": true, "hearing": true, "occluded": true}, "token": 102}
//!         ]
//!       }
//!     }
//!   ]
//! }"#, 4096)?;
//! let out = bridge.workflow(&req)?;
//! assert_eq!(out.get("results").and_then(|r| r.as_array()).map(|a| a.len()), Some(1));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::checked as c;
use crate::data::json::Value;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result};

pub const MAX_BEINGS: usize = 64;
pub const MAX_OPS: usize = 64;

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

fn uint_bound(v: &Value, lo: u64, hi: u64) -> Result<u64> {
    match v {
        Value::Number(s) => {
            let n = s.parse::<u64>().map_err(|_| c::bad())?;
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

/// Extract timeline-ready entry from either raw timeline entry or module outcome.
pub fn extract_entry(item: &Value) -> &Value {
    if let Some(entry) = item.get("entry").filter(|x| x.as_object().is_some()) {
        entry
    } else {
        item
    }
}

/// Evaluates whether a being with current senses and location perceives an event.
/// Returns `(perceived: bool, reason: &str)`.
pub fn can_perceive(being: &Value, entry: &Value) -> (bool, &'static str) {
    let being_id = being.get("id").and_then(Value::as_str).unwrap_or("");
    let being_place = being.get("place").and_then(Value::as_str);
    let senses = being.get("senses");
    let conditions = senses.and_then(|s| s.get("conditions")).and_then(Value::as_array);

    // 1. Physical incapacity
    if let Some(conds) = conditions {
        if conds.iter().any(|c| c.as_str() == Some("dead")) {
            return (false, "dead");
        }
        if conds.iter().any(|c| c.as_str() == Some("unconscious")) {
            return (false, "unconscious");
        }
    }

    // 2. Asleep: can only wake up if noise is loud (>= 80)
    let noise = entry.get("noise").and_then(|v| uint_bound(v, 0, 100).ok()).unwrap_or(10);
    if let Some(conds) = conditions {
        if conds.iter().any(|c| c.as_str() == Some("asleep")) && noise < 80 {
            return (false, "asleep_noise_too_low");
        }
    }

    // 3. Visibility and scope rules
    let vis = entry.get("visibility").and_then(Value::as_str).unwrap_or("public");
    let actors = entry.get("actors").and_then(Value::as_array);
    let witnesses = entry.get("witnesses").and_then(Value::as_array);
    let event_place = entry.get("place").and_then(Value::as_str);

    // Actors always perceive their own action if conscious
    if let Some(arr) = actors {
        if arr.iter().any(|a| a.as_str() == Some(being_id)) {
            return (true, "actor");
        }
    }

    // Private entries are known only to actors
    if vis == "private" {
        return (false, "private_not_actor");
    }

    // Explicitly named witnesses
    let is_named_witness = witnesses.is_some_and(|arr| arr.iter().any(|w| w.as_str() == Some(being_id)));

    // Location check
    let in_same_place = being_place.is_some() && event_place.is_some() && being_place == event_place;

    // Occlusion check
    let occluded = senses.and_then(|s| s.get("occluded")).and_then(|v| bool_bound(v).ok()).unwrap_or(false);

    // Sensory faculties
    let sight = senses.and_then(|s| s.get("sight")).and_then(|v| bool_bound(v).ok()).unwrap_or(true);
    let hearing = senses.and_then(|s| s.get("hearing")).and_then(|v| bool_bound(v).ok()).unwrap_or(true);

    if in_same_place {
        if !occluded && sight {
            return (true, "seen_in_place");
        }
        if hearing && noise >= 15 {
            return (true, "heard_in_place");
        }
        if is_named_witness {
            return (true, "witness_in_place");
        }
        return (false, "occluded_cannot_sense");
    }

    // If in different place
    if is_named_witness && !occluded && sight {
        return (true, "witness_line_of_sight");
    }

    // Loud public events can be heard from adjacent places
    if vis == "public" && hearing && noise >= 60 {
        return (true, "heard_at_distance");
    }

    (false, "different_place")
}

/// Constructs a valid `direct` Life experience conforming to `validate_event`.
pub fn make_direct_experience(token: u64, being_id: &str, entry: &Value) -> Value {
    let at = entry.get("at").and_then(|v| uint_bound(v, 0, u64::MAX).ok()).unwrap_or(0);
    let tag = entry.get("kind").and_then(Value::as_str).unwrap_or("event");
    let actors = entry.get("actors").and_then(Value::as_array);
    let subject = actors
        .and_then(|arr| arr.first())
        .and_then(Value::as_str)
        .or_else(|| entry.get("place").and_then(Value::as_str))
        .unwrap_or(being_id);
    let default_summary = format!("{tag} occurred.");
    let summary = entry.get("summary").and_then(Value::as_str).unwrap_or(&default_summary);

    obj! {
        "token" => token,
        "at" => at,
        "kind" => "direct",
        "tag" => tag,
        "subject" => subject,
        "source" => obj! {
            "id" => format!("wit_{token}"),
            "speaker" => being_id,
            "text" => summary,
        },
        "audience" => vec![Value::str(being_id)],
        "text" => summary,
        "causes" => Vec::<Value>::new(),
        "retain" => true,
    }
}

/// Constructs a valid `report` Life experience conforming to `validate_event`.
pub fn make_report_experience(token: u64, listener_id: &str, teller_id: &str, event: &Value, at: Option<u64>) -> Value {
    let event_at = at.unwrap_or_else(|| event.get("at").and_then(|v| uint_bound(v, 0, u64::MAX).ok()).unwrap_or(0));
    let tag = event.get("tag").or_else(|| event.get("kind")).and_then(Value::as_str).unwrap_or("report");
    let subject = event.get("subject").and_then(Value::as_str).unwrap_or(teller_id);
    let text_content =
        event.get("text").or_else(|| event.get("summary")).and_then(Value::as_str).unwrap_or("Reported event.");

    let causes = match event.get("token").and_then(|v| uint_bound(v, 1, u32::MAX as u64).ok()) {
        Some(t) => vec![Value::from(t)],
        None => Vec::new(),
    };

    let mut audience = vec![listener_id.to_string(), teller_id.to_string()];
    audience.sort();
    let audience_vals = audience.into_iter().map(Value::from).collect::<Vec<_>>();

    obj! {
        "token" => token,
        "at" => event_at,
        "kind" => "report",
        "tag" => tag,
        "subject" => subject,
        "source" => obj! {
            "id" => format!("rep_{token}"),
            "speaker" => teller_id,
            "text" => text_content,
        },
        "audience" => audience_vals,
        "text" => text_content,
        "causes" => causes,
        "retain" => true,
    }
}

/// Witness bridge engine for sensory perception and news transmission.
#[derive(Clone, Debug)]
pub struct WitnessBridge {
    next_token: u64,
}

impl Default for WitnessBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl WitnessBridge {
    pub fn new() -> Self {
        WitnessBridge { next_token: 1000 }
    }

    pub fn allocate_token(&mut self) -> u64 {
        let t = self.next_token;
        self.next_token = self.next_token.saturating_add(1);
        t
    }

    pub fn op_witness(&mut self, entry_item: &Value, beings: &[Value], base_token: Option<u64>) -> Result<Value> {
        let entry = extract_entry(entry_item);
        let entry_kind = entry.get("kind").and_then(Value::as_str).unwrap_or("event");

        let mut sorted_beings = beings.to_vec();
        sorted_beings.sort_by(|a, b| {
            let id_a = a.get("id").and_then(Value::as_str).unwrap_or("");
            let id_b = b.get("id").and_then(Value::as_str).unwrap_or("");
            id_a.cmp(id_b)
        });

        let mut cur_token = base_token.unwrap_or(self.next_token);
        let mut outcomes = Vec::new();
        let mut perceived_count = 0u64;

        for b in &sorted_beings {
            let being_id = ident_bound(c::get(b, "id")?)?;
            let (perceived, reason) = can_perceive(b, entry);

            let mut out = obj! {
                "being" => being_id,
                "perceived" => perceived,
                "reason" => reason,
            };

            if perceived {
                perceived_count += 1;
                let token = match b.get("token") {
                    Some(t_val) => uint_bound(t_val, 1, u32::MAX as u64)?,
                    None => {
                        let t = cur_token;
                        if base_token.is_none() {
                            cur_token += 1;
                            self.next_token = self.next_token.max(cur_token);
                        }
                        t
                    }
                };
                let exp = make_direct_experience(token, being_id, entry);
                crate::character::life::validate_event(&exp, being_id, &[being_id.to_string()])?;
                if let Value::Object(ref mut fields) = out {
                    fields.push(("experience".to_string(), exp));
                }
            }

            outcomes.push(out);
        }

        Ok(obj! {
            "entry_kind" => entry_kind,
            "outcomes" => outcomes,
            "perceived_count" => perceived_count,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn op_tell(
        &mut self,
        teller: &str,
        listener: &str,
        event: &Value,
        at: Option<u64>,
        token: Option<u64>,
        teller_place: Option<&str>,
        listener_place: Option<&str>,
    ) -> Result<Value> {
        if teller == listener {
            return Err(c::bad());
        }

        if let (Some(tp), Some(lp)) = (teller_place, listener_place) {
            if tp != lp {
                return Err(c::bad());
            }
        }

        let t = match token {
            Some(tok) => tok,
            None => self.allocate_token(),
        };

        let exp = make_report_experience(t, listener, teller, event, at);

        let mut aud = vec![listener.to_string(), teller.to_string()];
        aud.sort();
        crate::character::life::validate_event(&exp, listener, &aud)?;

        Ok(obj! {
            "ok" => true,
            "teller" => teller,
            "listener" => listener,
            "experience" => exp,
        })
    }

    pub fn workflow(&mut self, req: &Value) -> Result<Value> {
        keys(req, &["ops"], &["beings"])?;
        let ops = c::get(req, "ops")?.as_array().ok_or_else(c::bad)?;
        if ops.len() > MAX_OPS {
            return Err(c::bad());
        }

        let default_beings = req.get("beings").and_then(Value::as_array);

        let mut results = Vec::new();
        for op_call in ops {
            keys(op_call, &["op"], &["args"])?;
            let op_name = text_bound(c::get(op_call, "op")?, 32)?;
            let empty_args = Value::Object(Vec::new());
            let args = op_call.get("args").filter(|x| **x != Value::Null).unwrap_or(&empty_args);

            match op_name {
                "witness" => {
                    keys(args, &["entry"], &["beings", "base_token"])?;
                    let entry = c::get(args, "entry")?;
                    // Validate entry fields
                    let actual_entry = extract_entry(entry);
                    if let Some(at_val) = actual_entry.get("at") {
                        uint_bound(at_val, 0, u64::MAX)?;
                    }
                    let beings_arr = args.get("beings").and_then(Value::as_array).or(default_beings);
                    let beings = beings_arr.unwrap_or(&[]);
                    if beings.len() > MAX_BEINGS {
                        return Err(c::bad());
                    }
                    let base_token = args
                        .get("base_token")
                        .filter(|x| **x != Value::Null)
                        .map(|v| uint_bound(v, 1, u32::MAX as u64))
                        .transpose()?;
                    results.push(self.op_witness(entry, beings, base_token)?);
                }
                "tell" => {
                    keys(args, &["teller", "listener", "event"], &["at", "token", "teller_place", "listener_place"])?;
                    let teller = ident_bound(c::get(args, "teller")?)?;
                    let listener = ident_bound(c::get(args, "listener")?)?;
                    let event = c::get(args, "event")?;
                    let at = args
                        .get("at")
                        .filter(|x| **x != Value::Null)
                        .map(|v| uint_bound(v, 0, u64::MAX))
                        .transpose()?;
                    let token = args
                        .get("token")
                        .filter(|x| **x != Value::Null)
                        .map(|v| uint_bound(v, 1, u32::MAX as u64))
                        .transpose()?;
                    let teller_place = args
                        .get("teller_place")
                        .filter(|x| **x != Value::Null)
                        .map(|v| text_bound(v, 64))
                        .transpose()?;
                    let listener_place = args
                        .get("listener_place")
                        .filter(|x| **x != Value::Null)
                        .map(|v| text_bound(v, 64))
                        .transpose()?;
                    results.push(self.op_tell(teller, listener, event, at, token, teller_place, listener_place)?);
                }
                _ => return Err(c::bad()),
            }
        }

        Ok(obj! {
            "results" => results,
        })
    }
}

pub fn workflow(a: &Value) -> Result<Value> {
    let mut bridge = WitnessBridge::new();
    bridge.workflow(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    #[test]
    fn witness_cart_crash_and_tell_report() -> Result<()> {
        let mut bridge = WitnessBridge::new();
        let req = parse(
            r#"{
          "ops": [
            {
              "op": "witness",
              "args": {
                "entry": {
                  "at": 100,
                  "kind": "crash",
                  "summary": "A grain cart overturned in the road.",
                  "place": "road",
                  "actors": ["carter"],
                  "witnesses": ["baker"],
                  "visibility": "witnessed",
                  "noise": 40
                },
                "beings": [
                  {"id": "baker", "place": "road", "senses": {"sight": true, "hearing": true}, "token": 101},
                  {"id": "apprentice", "place": "mill", "senses": {"sight": true, "hearing": true, "occluded": true}, "token": 102}
                ]
              }
            }
          ]
        }"#,
            4096,
        )?;

        let out = bridge.workflow(&req)?;
        let outcomes =
            out.get("results").and_then(Value::as_array).unwrap()[0].get("outcomes").and_then(Value::as_array).unwrap();

        // Apprentice in mill was occluded in different place
        assert_eq!(outcomes[0].get("being").and_then(Value::as_str), Some("apprentice"));
        assert_eq!(outcomes[0].get("perceived").and_then(|v| bool_bound(v).ok()), Some(false));

        // Baker on road perceived direct experience
        assert_eq!(outcomes[1].get("being").and_then(Value::as_str), Some("baker"));
        assert_eq!(outcomes[1].get("perceived").and_then(|v| bool_bound(v).ok()), Some(true));
        let exp = outcomes[1].get("experience").unwrap();
        assert_eq!(exp.get("kind").and_then(Value::as_str), Some("direct"));
        assert_eq!(exp.get("token").and_then(|v| uint_bound(v, 0, 1000).ok()), Some(101));

        // Tell operation
        let req_tell = parse(
            r#"{
          "ops": [
            {
              "op": "tell",
              "args": {
                "teller": "baker",
                "listener": "player",
                "event": {
                  "token": 101,
                  "at": 100,
                  "kind": "crash",
                  "summary": "A grain cart overturned in the road."
                },
                "at": 120,
                "token": 201,
                "teller_place": "bakery",
                "listener_place": "bakery"
              }
            }
          ]
        }"#,
            4096,
        )?;

        let out_tell = bridge.workflow(&req_tell)?;
        let res_tell = &out_tell.get("results").and_then(Value::as_array).unwrap()[0];
        assert_eq!(res_tell.get("ok").and_then(|v| bool_bound(v).ok()), Some(true));
        let rep_exp = res_tell.get("experience").unwrap();
        assert_eq!(rep_exp.get("kind").and_then(Value::as_str), Some("report"));
        assert_eq!(rep_exp.get("causes").and_then(Value::as_array).unwrap().len(), 1);
        Ok(())
    }
}
