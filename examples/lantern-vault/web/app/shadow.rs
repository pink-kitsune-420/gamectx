//! Measurement only: with `GAMECTX_SHADOW=<file>` set, every `chat` also runs a fresh
//! `RoleplayEngine` on the same turn and appends one JSON line comparing the engine's
//! outcome with what Lantern Vault did. Nothing here changes the game.
use super::App;
use gamectx::{obj, Value};
use std::io::Write;

impl App {
    pub(super) fn shadow_engine(&self, text: &str) -> Value {
        let run = || -> gamectx::Result<Value> {
            let mut engine = match self.npc.as_str() {
                "keeper" => self.engine_keeper.clone(),
                "hound" => self.engine_hound.clone(),
                _ => self.engine_merchant.clone(),
            };
            let items = self.game.visible_items();
            let labels = items
                .iter()
                .map(|i| obj! {"id"=>i.get("id").cloned().unwrap_or(Value::Null),"aliases"=>vec![i.get("label").cloned().unwrap_or(Value::Null)]})
                .collect::<Vec<_>>();
            let mut targets = vec![
                obj! {"kind"=>"character","id"=>"merchant","names"=>vec!["Ilen","merchant"]},
                obj! {"kind"=>"character","id"=>"keeper","names"=>vec!["Tovin","keeper"]},
                obj! {"kind"=>"character","id"=>"hound","names"=>vec!["Hound","hound"]},
            ];
            for c in self.game.choices() {
                if c.get("action").and_then(Value::as_str) == Some("go") {
                    if let Some(room) = c.get("target").and_then(Value::as_str).and_then(|t| t.strip_prefix("room:")) {
                        targets.push(obj! {"kind"=>"place","id"=>room,"names"=>vec![room]});
                    }
                }
            }
            let place = if self.game.room == "workshop" { "workplace" } else { "social" };
            let focus = self
                .focus
                .as_ref()
                .filter(|(_, at)| self.tick.saturating_sub(*at) <= 4)
                .map_or(Value::Null, |(id, _)| Value::str(id));
            let trust = self.attitude_level("trust").unwrap_or(1);
            let turn = obj! {
                "listener"=>self.player.clone(),
                "text"=>text,
                "at"=>self.tick,
                "token"=>self.seq+1,
                "scene"=>obj!{
                    "id"=>self.game.room.clone(),
                    "label"=>self.game.room.replace('_'," "),
                    "place"=>place,
                    "atmosphere"=>"calm",
                    "visit"=>1
                },
                "trust"=>trust,
                "choices"=>self.game.choices(),
                "labels"=>labels,
                "focus"=>focus,
                "targets"=>targets
            };
            engine.turn(&turn)
        };
        match run() {
            Ok(v) => {
                obj! {"route"=>v.get("route").cloned().unwrap_or(Value::Null),"kind"=>v.get("kind").cloned().unwrap_or(Value::Null),
                "intent"=>v.get("intent").cloned().unwrap_or(Value::Null),"text"=>v.get("text").cloned().unwrap_or(Value::Null),
                "decision"=>v.get("decision").cloned().unwrap_or(Value::Null)}
            }
            Err(_) => obj! {"route"=>"error"},
        }
    }

    pub(super) fn shadow_record(&self, text: &str, active: bool, engine: Value, lv: &Value, said: &[Value]) {
        let Ok(path) = std::env::var("GAMECTX_SHADOW") else { return };
        let row = obj! {"text"=>text,"npc"=>self.npc.clone(),"room"=>self.game.room.clone(),"active"=>active,"engine"=>engine,"lv"=>lv.clone(),"said"=>said.to_vec()};
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(format!("{}\n", row.to_json()).as_bytes());
        }
    }
}
