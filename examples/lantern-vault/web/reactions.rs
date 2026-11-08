//! Original host adapters. Policies authorize; transactions conserve actual resources.
use super::*;
use gamectx::{character::roleplay::ReactionPolicy, obj};
#[derive(Clone)]
pub struct Reactions {
    pub config: Value,
    pub records: Vec<Value>,
    banks: [u64; 2],
}
fn array(v: &Value) -> Result<&[Value]> {
    v.as_array().ok_or_else(err)
}
fn index(actor: &str) -> Result<usize> {
    match actor {
        "merchant" => Ok(0),
        "keeper" => Ok(1),
        _ => Err(err()),
    }
}
impl Reactions {
    pub fn new(dir: &std::path::Path) -> Result<Self> {
        let p = dir.join("reactions.json");
        let config = if p.exists() { parse(&read_file(&p, 65536)?)? } else { parse(include_str!("reactions.json"))? };
        Self::from_config(config)
    }
    fn from_config(config: Value) -> Result<Self> {
        fields(&config, &["version", "enabled", "characters"])?;
        if num(&config, "version", 1)? != 1 || !matches!(get(&config, "enabled")?, Value::Bool(_)) {
            return Err(err());
        }
        let chars = array(get(&config, "characters")?)?;
        if chars.len() != 2 {
            return Err(err());
        }
        let mut banks = [0; 2];
        let mut seen = Vec::new();
        for ch in chars {
            fields(ch, &["actor", "coins", "policy"])?;
            let actor = text(ch, "actor")?;
            let i = index(actor)?;
            if seen.contains(&i) {
                return Err(err());
            }
            seen.push(i);
            banks[i] = num(ch, "coins", 10000)?;
            ReactionPolicy::from_value(get(ch, "policy")?)?;
        }
        Ok(Self { config, records: Vec::new(), banks })
    }
    pub fn enabled(&self) -> bool {
        self.config.get("enabled") == Some(&true.into())
    }
    pub fn fresh(&self) -> Result<Self> {
        Self::from_config(self.config.clone())
    }
    pub fn policy(&self, actor: &str) -> Result<ReactionPolicy> {
        let ch = array(get(&self.config, "characters")?)?
            .iter()
            .find(|c| c.get("actor").and_then(Value::as_str) == Some(actor))
            .ok_or_else(err)?;
        let mut p = get(ch, "policy")?.clone();
        if !self.enabled() {
            set(&mut p, "enabled", false.into());
        }
        ReactionPolicy::from_value(&p)
    }
    pub fn used(&self, actor: &str, player: &str) -> Vec<Value> {
        self.records
            .iter()
            .filter(|r| {
                r.get("actor").and_then(Value::as_str) == Some(actor)
                    && r.get("listener").and_then(Value::as_str) == Some(player)
                    && r.get("op").and_then(Value::as_str) == Some("grant")
            })
            .filter_map(|r| r.get("id").cloned())
            .collect()
    }
    pub fn resources(&self, actor: &str, game: &game::Game) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        let coins = self.banks[index(actor)?];
        if coins > 0 {
            out.push(obj! {"kind"=>"coins","id"=>"coin","amount"=>coins});
        }
        for i in game.visible_items().iter().filter(|i| i.get("owner").and_then(Value::as_str) == Some(actor)) {
            out.push(obj! {"kind"=>"item","id"=>get(i,"id")?.clone(),"amount"=>1});
        }
        Ok(out)
    }
    pub fn trade(&mut self, before: &game::Game, decision: &Value) -> Result<()> {
        if decision.get("action").and_then(Value::as_str) == Some("buy") {
            let items = before.visible_items();
            let item = items.iter().find(|i| i.get("id") == decision.get("target")).ok_or_else(err)?;
            let i = index(text(item, "owner")?)?;
            self.banks[i] =
                self.banks[i].checked_add(num(item, "price", 10000)?).filter(|n| *n <= 1_000_000).ok_or_else(err)?;
        }
        Ok(())
    }
    pub fn debt(&self, actor: &str, player: &str) -> Option<&Value> {
        self.records.iter().find(|r| {
            r.get("actor").and_then(Value::as_str) == Some(actor)
                && r.get("listener").and_then(Value::as_str) == Some(player)
                && r.get("handler").and_then(Value::as_str) == Some("loan")
                && r.get("op").and_then(Value::as_str) == Some("grant")
                && !self.records.iter().any(|paid| {
                    paid.get("op").and_then(Value::as_str) == Some("repay")
                        && paid.get("actor") == r.get("actor")
                        && paid.get("listener") == r.get("listener")
                        && paid.get("id") == r.get("id")
                })
        })
    }
    fn transfer(&mut self, game: &mut game::Game, actor: &str, r: &Value, to_player: bool) -> Result<()> {
        let i = index(actor)?;
        if game.room != (if actor == "merchant" { "workshop" } else { "cistern" }) || game.hp == 0 || game.won {
            return Err(err());
        }
        let amount = num(r, "amount", 1_000_000)?;
        if text(r, "kind")? == "coins" {
            if text(r, "id")? != "coin" {
                return Err(err());
            }
            if to_player {
                if self.banks[i] < amount {
                    return Err(err());
                }
                let coins = game.coins.checked_add(amount as u32).filter(|n| *n <= 1_000_000).ok_or_else(err)?;
                self.banks[i] -= amount;
                game.coins = coins;
            } else {
                if (game.coins as u64) < amount {
                    return Err(err());
                }
                let bank = self.banks[i].checked_add(amount).filter(|n| *n <= 1_000_000).ok_or_else(err)?;
                game.coins -= amount as u32;
                self.banks[i] = bank;
            }
        } else if text(r, "kind")? == "item" && amount == 1 {
            game.npc_transfer(actor, text(r, "id")?, to_player)?;
        } else {
            return Err(err());
        }
        Ok(())
    }
    pub fn can_grant(&self, handler: &str) -> bool {
        let open = self
            .records
            .iter()
            .filter(|r| {
                r.get("op").and_then(Value::as_str) == Some("grant")
                    && r.get("handler").and_then(Value::as_str) == Some("loan")
                    && !self.records.iter().any(|paid| {
                        paid.get("op").and_then(Value::as_str) == Some("repay")
                            && paid.get("actor") == r.get("actor")
                            && paid.get("listener") == r.get("listener")
                            && paid.get("id") == r.get("id")
                    })
            })
            .count();
        self.records.len() + 1 + open + usize::from(handler == "loan") <= 128
    }
    pub fn grant(&mut self, game: &mut game::Game, frame: &Value, id: &str, after: usize) -> Result<Value> {
        if self.records.len() >= 128 {
            return Err(err());
        }
        let actor = text(frame, "actor")?;
        let listener = text(frame, "listener")?;
        if game.room != (if actor == "merchant" { "workshop" } else { "cistern" })
            || game.hp == 0
            || game.won
            || get(frame, "resources")? != &Value::from(self.resources(actor, game)?)
            || get(frame, "used")? != &Value::from(self.used(actor, listener))
        {
            return Err(err());
        }
        let cap = self.policy(actor)?.authorize(frame, &obj! {"id"=>id,"revision"=>get(frame,"revision")?.clone()})?;
        let handler = text(&cap, "handler")?;
        if !["gift", "loan", "support"].contains(&handler) || !self.can_grant(handler) {
            return Err(err());
        }
        let resource = get(&cap, "resource")?;
        if handler == "support" && resource != &Value::Null || handler != "support" && resource == &Value::Null {
            return Err(err());
        }
        let mut next = self.clone();
        let mut updated = game.clone();
        if resource != &Value::Null {
            next.transfer(&mut updated, actor, resource, true)?;
        }
        let record = obj! {"op"=>"grant","id"=>id,"actor"=>actor,"listener"=>listener,"handler"=>handler,"resource"=>resource.clone(),"after"=>after,"frame"=>frame.clone()};
        next.records.push(record.clone());
        *self = next;
        *game = updated;
        Ok(record)
    }
    pub fn repay(&mut self, game: &mut game::Game, actor: &str, player: &str, after: usize) -> Result<Value> {
        if self.records.len() >= 128 {
            return Err(err());
        }
        let debt = self.debt(actor, player).ok_or_else(err)?.clone();
        let mut next = self.clone();
        let mut updated = game.clone();
        next.transfer(&mut updated, actor, get(&debt, "resource")?, false)?;
        let record = obj! {"op"=>"repay","id"=>get(&debt,"id")?.clone(),"actor"=>actor,"listener"=>player,"handler"=>"loan","resource"=>get(&debt,"resource")?.clone(),"after"=>after,"frame"=>Value::Null};
        next.records.push(record.clone());
        *self = next;
        *game = updated;
        Ok(record)
    }
    fn digest(&self) -> Result<String> {
        gamectx::media::visual::content_digest(get(&self.config, "characters")?.to_json().as_bytes())
    }
    pub fn checkpoint(&self) -> Result<Value> {
        Ok(obj! {"version"=>1,"config_digest"=>self.digest()?,"records"=>self.records.clone()})
    }
    pub fn replay(&self, saved: Option<&Value>, seed: u32, actions: &[Value]) -> Result<(Self, game::Game)> {
        let mut state = self.fresh()?;
        set(&mut state.config, "enabled", true.into());
        let mut game = game::Game::new(seed)?;
        let records = if let Some(s) = saved {
            fields(s, &["version", "config_digest", "records"])?;
            if num(s, "version", 1)? != 1 || text(s, "config_digest")? != self.digest()? {
                return Err(err());
            }
            array(get(s, "records")?)?
        } else {
            &[]
        };
        if records.len() > 128 {
            return Err(err());
        }
        let mut cursor = 0;
        for at in 0..=actions.len() {
            while cursor < records.len() && num(&records[cursor], "after", 256)? as usize == at {
                let r = &records[cursor];
                fields(r, &["op", "id", "actor", "listener", "handler", "resource", "after", "frame"])?;
                let actor = text(r, "actor")?;
                let player = text(r, "listener")?;
                if !["player_1", "player_2"].contains(&player) {
                    return Err(err());
                }
                let result = if text(r, "op")? == "grant" {
                    let f = get(r, "frame")?;
                    if f.get("actor") != r.get("actor")
                        || f.get("listener") != r.get("listener")
                        || get(f, "resources")? != &Value::from(state.resources(actor, &game)?)
                        || get(f, "used")? != &Value::from(state.used(actor, player))
                    {
                        return Err(err());
                    }
                    // Stored host checkpoints own the historical attitude/physical evidence.
                    state.grant(&mut game, f, text(r, "id")?, at)?
                } else if text(r, "op")? == "repay" {
                    state.repay(&mut game, actor, player, at)?
                } else {
                    return Err(err());
                };
                if &result != r {
                    return Err(err());
                }
                cursor += 1;
            }
            if at < actions.len() {
                let a = &actions[at];
                let before = game.clone();
                game.act(get(a, "revision")?, get(a, "decision")?);
                if game.revision != before.revision + 1 {
                    return Err(err());
                }
                state.trade(&before, get(a, "decision")?)?;
            }
        }
        if cursor != records.len() {
            return Err(err());
        }
        set(&mut state.config, "enabled", self.enabled().into());
        Ok((state, game))
    }
    pub fn configure(&mut self, v: &Value, actions: &[Value], seed: u32, dir: &std::path::Path) -> Result<()> {
        let next = Self::from_config(v.clone())?;
        if !self.records.is_empty() && next.digest() != self.digest() {
            return Err(err());
        }
        let saved = if self.records.is_empty() { None } else { Some(self.checkpoint()?) };
        let (mut next, _) = next.replay(saved.as_ref(), seed, actions)?;
        // Preserve the enable switch even while recovering already-confirmed effects.
        next.config = v.clone();
        atomic(&dir.join("reactions.json"), v.to_json().as_bytes())?;
        *self = next;
        Ok(())
    }
    pub fn snapshot(&self, offers: Vec<Value>) -> Value {
        obj! {"config"=>self.config.clone(),"banks"=>obj!{"merchant"=>self.banks[0],"keeper"=>self.banks[1]},"records"=>self.records.clone(),"offers"=>offers}
    }
}
/// Deliberately small request recognizer; never grants permissions or treats threats as proof.
pub fn intent(input: &str) -> Option<(&'static str, Value, bool)> {
    let q = input.to_lowercase();
    if ["what do i owe", "do i owe", "my loan", "loan status"].iter().any(|s| q.contains(s)) {
        return Some(("debt", Value::Null, false));
    }
    if q.contains("will repay")
        || q.contains("repay you later")
        || q.contains("pay you back later")
        || q.contains("return borrowed things later")
    {
        return None;
    }
    if [
        "repay",
        "i want to repay",
        "i would like to repay",
        "can i repay",
        "let me repay",
        "i want to pay you back",
        "pay you back",
        "i want to return borrowed",
        "i would like to return borrowed",
    ]
    .iter()
    .any(|s| q.starts_with(s))
    {
        return Some(("repay", Value::Null, false));
    }
    let trigger = if ["lend", "loan", "borrow"].iter().any(|s| q.contains(s)) {
        "loan"
    } else if ["give me", "gift", "can i have"].iter().any(|s| q.contains(s)) {
        "gift"
    } else if ["help me", "reassure me"].iter().any(|s| q.contains(s)) {
        "help"
    } else {
        return None;
    };
    let hostile = ["kill", "hurt", "stab", "or else", "attack you"].iter().any(|s| q.contains(s));
    let coin = q.contains("coin") || q.contains("money");
    let items = ["oil", "chalk", "tonic", "keeper_seal"]
        .into_iter()
        .filter(|id| q.contains(id) || *id == "keeper_seal" && q.contains("seal"))
        .collect::<Vec<_>>();
    let words = q.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>();
    let quantities = words
        .iter()
        .filter_map(|w| match *w {
            "one" => Some(1),
            "two" => Some(2),
            "three" => Some(3),
            "four" => Some(4),
            "five" => Some(5),
            "six" => Some(6),
            "seven" => Some(7),
            "eight" => Some(8),
            "nine" => Some(9),
            "ten" => Some(10),
            n if n.bytes().all(|b| b.is_ascii_digit()) => Some(n.parse::<u64>().unwrap_or(1_000_001)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let amount = quantities.first().copied().unwrap_or(1);
    let ambiguous = items.len() + usize::from(coin) > 1
        || quantities.len() > 1
        || amount == 0
        || amount > 1_000_000
        || words.iter().any(|w| ["all", "every", "hundred", "thousand", "million", "dozen"].contains(w));
    let resource = if ambiguous {
        Value::Null
    } else if coin {
        obj! {"kind"=>"coins","id"=>"coin","amount"=>amount}
    } else if items.len() == 1 && amount == 1 {
        obj! {"kind"=>"item","id"=>format!("item:{}",items[0]),"amount"=>1}
    } else {
        Value::Null
    };
    Some((trigger, resource, hostile))
}
