//! Editable host-authored dispositions and delivery-confirmed subjective stances.
use super::*;
use gamectx::{character::roleplay::validate_viewpoint_line, character::roleplay::Viewpoints, obj};
#[derive(Clone)]
pub struct Mind {
    pub config: Value,
    pub views: Viewpoints,
}
impl Mind {
    pub fn new(dir: &std::path::Path) -> Result<Self> {
        let path = dir.join("personalities.json");
        let config =
            if path.exists() { parse(&read_file(&path, 16384)?)? } else { parse(include_str!("personalities.json"))? };
        validate(&config)?;
        let path = dir.join("viewpoints.json");
        let views = if path.exists() {
            Viewpoints::recover(&parse(&read_file(&path, 256 * 1024)?)?)?
        } else {
            Viewpoints::new()
        };
        let mind = Self { config, views };
        mind.validate_actors()?;
        Ok(mind)
    }
    fn validate_actors(&self) -> Result<()> {
        for e in array(get(&self.views.checkpoint(), "entries")?)? {
            if !["merchant", "keeper"].contains(&text(e, "actor")?)
                || !["player_1", "player_2"].contains(&text(e, "listener")?)
            {
                return Err(err());
            }
        }
        Ok(())
    }
    pub fn replace(&mut self, views: Viewpoints, dir: &std::path::Path) -> Result<()> {
        atomic(&dir.join("viewpoints.json"), views.checkpoint().to_json().as_bytes())?;
        self.views = views;
        Ok(())
    }
    pub fn confirm(&mut self, view: &Value, heard: &str, dir: &std::path::Path) -> Result<()> {
        let mut views = self.views.clone();
        views.confirm(view, heard)?;
        self.replace(views, dir)
    }
    pub fn profile(&self, actor: &str) -> Result<&Value> {
        array(get(&self.config, "profiles")?)?
            .iter()
            .find(|v| v.get("actor").and_then(Value::as_str) == Some(actor))
            .ok_or_else(err)
    }
    pub fn enabled(&self) -> bool {
        self.config.get("enabled") == Some(&Value::Bool(true))
    }
    pub fn topic(&self, input: &str, trust: i64) -> Option<String> {
        if !self.enabled() {
            return None;
        }
        let q = input.to_lowercase();
        if ["born", "weather", "password", "watchword", "stock", "sell", "price", "lend", "loan", "give me", "gift"]
            .iter()
            .any(|s| q.contains(s))
        {
            return None;
        }
        if ["hobby", "hobbies", "for fun", "spare time", "enjoy doing"].iter().any(|s| q.contains(s)) {
            return Some("hobbies".into());
        }
        if ["dream", "hope", "aspiration"].iter().any(|s| q.contains(s)) {
            return Some("dreams".into());
        }
        if ["feel about me", "think of me", "like me", "trust me"].iter().any(|s| q.contains(s)) {
            return Some(format!(
                "listener_{}",
                if trust < 0 {
                    "guarded"
                } else if trust < 25 {
                    "cautious"
                } else {
                    "warm"
                }
            ));
        }
        let stem = [
            "opinion of ",
            "opinion on ",
            "think of ",
            "think about ",
            "feel about ",
            "do you like ",
            "do you prefer ",
            "your favorite ",
            "your favourite ",
        ]
        .iter()
        .find_map(|p| q.split_once(p).map(|(_, s)| s));
        if let Some(s) = stem {
            let topic = s.trim_end_matches(['?', '.', '!']).split_whitespace().collect::<Vec<_>>().join("_");
            if !topic.is_empty() && topic.len() <= 40 && topic.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                return Some(format!("about_{topic}"));
            }
        }
        None
    }
    pub fn reason(&self, actor: &str) -> Result<String> {
        Ok(self
            .profile(actor)?
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or(if actor == "merchant" {
                "I value fair treatment because useful work means less to me without honesty."
            } else {
                "I value patience because people and places deserve careful attention."
            })
            .into())
    }
    pub fn fallback(&self, actor: &str, topic: &str, trust: i64) -> Result<String> {
        Ok(if topic == "hobbies" {
            if actor == "merchant" {
                "I would like to spend quiet moments sketching useful little designs."
            } else {
                "I prefer quiet word games and imagining how a forgotten place might be cared for."
            }
            .into()
        } else if topic == "dreams" {
            text(self.profile(actor)?, "dream")?.into()
        } else if topic.starts_with("listener_") {
            if trust < 0 {
                "I feel guarded with you, and would prefer to take things slowly."
            } else if trust >= 25 {
                "I feel warmer toward you because of the care you have shown."
            } else {
                "I feel curious about you, though I prefer to let trust grow slowly."
            }
            .into()
        } else {
            text(self.profile(actor)?, "preference")?.into()
        })
    }
    pub fn configure(&mut self, v: &Value, dir: &std::path::Path) -> Result<()> {
        validate(v)?;
        atomic(&dir.join("personalities.json"), v.to_json().as_bytes())?;
        self.config = v.clone();
        Ok(())
    }
    pub fn snapshot(&self) -> Value {
        obj! {"config"=>self.config.clone(),"viewpoints"=>self.views.checkpoint()}
    }
}
fn array(v: &Value) -> Result<&[Value]> {
    v.as_array().ok_or_else(err)
}
fn validate(v: &Value) -> Result<()> {
    fields(v, &["version", "enabled", "profiles"])?;
    if num(v, "version", 1)? != 1 || !matches!(get(v, "enabled")?, Value::Bool(_)) {
        return Err(err());
    }
    let profiles = array(get(v, "profiles")?)?;
    if profiles.len() != 2 {
        return Err(err());
    }
    for (index, p) in profiles.iter().enumerate() {
        let mut keys = vec!["actor", "traits", "dream", "preference", "generosity", "prudence"];
        if p.get("reason").is_some() {
            keys.push("reason");
            let r = text(p, "reason")?;
            if r.chars().count() > 180 {
                return Err(err());
            }
            validate_viewpoint_line(r)?;
        }
        for field in ["story_seeds", "interests"] {
            if let Some(seeds) = p.get(field) {
                keys.push(field);
                let xs = array(seeds)?;
                if xs.is_empty() || xs.len() > 8 {
                    return Err(err());
                }
                for x in xs {
                    let s = x.as_str().ok_or_else(err)?;
                    if s.is_empty() || s.len() > 512 || s.chars().any(char::is_control) {
                        return Err(err());
                    }
                }
            }
        }
        fields(p, &keys)?;
        let actor = text(p, "actor")?;
        if !["merchant", "keeper"].contains(&actor)
            || profiles[..index].iter().any(|old| old.get("actor") == p.get("actor"))
        {
            return Err(err());
        }
        let t = text(p, "traits")?;
        if t.is_empty() || t.len() > 512 || t.chars().any(char::is_control) {
            return Err(err());
        }
        for k in ["dream", "preference"] {
            validate_viewpoint_line(text(p, k)?)?;
        }
        for k in ["generosity", "prudence"] {
            num(p, k, 100)?;
        }
    }
    Ok(())
}
