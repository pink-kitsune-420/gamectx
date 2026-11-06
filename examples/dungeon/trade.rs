//! Example-only English purchase routing and game terms, using shared confirmation.
use super::game::{get, pair, text, uint, Game};
use gamectx::{decision::PendingAction, obj, Error, Result, Value};
#[derive(Clone, Debug, Default)]
pub struct Trade {
    pending: Option<PendingAction>,
    sequence: u32,
}
fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}
fn reply(s: impl Into<String>) -> Value {
    obj! {"handled"=>true,"reply"=>s.into()}
}
impl Trade {
    pub fn cancel(&mut self) -> Value {
        self.pending = None;
        obj! {"cancelled"=>true}
    }
    pub fn exchange(&mut self, game: &mut Game, speaker: &str, input: &str) -> Result<Value> {
        if input.trim().is_empty() || input.chars().count() > 240 || input.chars().any(char::is_control) {
            return Err(Error::new("invalid trade input"));
        }
        if speaker != "merchant" {
            self.pending = None;
            return Ok(obj! {"handled"=>false});
        }
        let words = words(input);
        let yes = [vec!["yes"], vec!["yes", "please"], vec!["y"], vec!["confirm"]].iter().any(|v| *v == words);
        let no = [vec!["no"], vec!["no", "thanks"], vec!["n"], vec!["cancel"]].iter().any(|v| *v == words);
        if yes || no {
            let Some(mut pending) = self.pending.take() else {
                return Ok(reply("No purchase is waiting. Tell me which item you would like to buy."));
            };
            if no {
                pending.respond("merchant", game.revision, false, &Value::Null, |_| false)?;
                return Ok(reply("Purchase cancelled. Your coins and inventory are unchanged."));
            }
            let offer = pending.offer();
            let decision = get(&offer, "decision")?;
            let items = game.visible_items();
            let item = items.iter().find(|i| i.get("id") == decision.get("target"));
            let terms=item.map(|i|obj! {"item"=>i.get("id").expect("id").clone(),"price"=>i.get("price").expect("price").clone(),"quantity"=>1}).unwrap_or(Value::Null);
            let accepted =
                pending.respond("merchant", game.revision, true, &terms, |p| game.choices().iter().any(|q| q.same(p)));
            if accepted.is_err() {
                return Ok(reply("That offer is no longer current. Ask to buy the item again."));
            }
            let item = item.expect("live approved item");
            let result = game.act(&game.revision.into(), decision);
            if result.get("status").and_then(Value::as_str) != Some("accepted") {
                return Ok(reply("That offer is no longer current. Ask to buy the item again."));
            }
            return Ok(
                obj! {"handled"=>true,"reply"=>format!("You bought {} for {} coins. You have {} coins left.",text(item,"label")?,uint(get(item,"price")?,u32::MAX)?,game.coins),"result"=>result},
            );
        }
        self.pending = None;
        let lower = input.to_lowercase();
        let negative = ["not", "don", "never", "without", "cannot"].iter().any(|w| words.iter().any(|v| v == w))
            || lower.contains("can't");
        let request = ["buy", "purchase"].iter().any(|w| words.iter().any(|v| v == w))
            || lower.starts_with("i'd like ")
            || lower.starts_with("i would like ");
        if negative || !request {
            return Ok(obj! {"handled"=>false});
        }
        let items = game.visible_items();
        let pairs = items.iter().map(|i| pair("buy", i.get("id").and_then(Value::as_str))).collect::<Vec<_>>();
        let resolution = super::intent::resolve(&pairs, &items, input, None, None)?;
        if resolution.candidates.len() != 1 {
            return Ok(reply("Which item would you like to buy? Name one visible item."));
        }
        let id = get(&resolution.candidates[0], "id")?;
        let item = items.iter().find(|i| i.get("id") == Some(id)).expect("visible item");
        if text(item, "owner")? != "merchant" {
            return Ok(reply("That item is not for sale by this merchant."));
        }
        let price = uint(get(item, "price")?, u32::MAX)?;
        if price > game.coins {
            return Ok(reply(format!(
                "The {} costs {} coins. You have {}, so you need {} more.",
                text(item, "label")?,
                price,
                game.coins,
                price - game.coins
            )));
        }
        let decision = pair("buy", id.as_str());
        if !game.choices().iter().any(|p| p.same(&decision)) {
            return Ok(reply("That item cannot be bought right now."));
        }
        if self.sequence == u32::MAX {
            return Err(Error::new("trade sequence exhausted"));
        }
        self.sequence += 1;
        let offer = obj! {"id"=>format!("trade_{}",self.sequence),"scope"=>"merchant","revision"=>game.revision,"decision"=>decision,
        "terms"=>obj! {"item"=>id.clone(),"price"=>price,"quantity"=>1}};
        self.pending = Some(PendingAction::new(&offer)?);
        Ok(
            obj! {"handled"=>true,"reply"=>format!("The {} costs {} coins. You have {}. Buy one? Say yes or no.",text(item,"label")?,price,game.coins),"offer"=>offer},
        )
    }
}
