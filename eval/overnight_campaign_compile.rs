//! Evaluation host: recover actual scoped Life/notebook checkpoints, then use the
//! existing PartyScene compiler (never the ungated legacy single-Life renderer).
use gamectx::{
    archetype::Blueprint,
    cast::CastPack,
    dialogue::InteractionTools,
    life::{Config, Definition, Life, Setting},
    lore::Lore,
    obj,
    party::{AttitudeGates, ModelRoute, PartyScene, SceneTurn},
    Error, Result, Value,
};
use std::{
    io::{self, Read},
    sync::Arc,
};
fn get<'a>(v: &'a Value, k: &str) -> Result<&'a Value> {
    v.get(k).ok_or_else(|| Error::new("missing campaign fixture field"))
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    get(v, k)?.as_str().ok_or_else(|| Error::new("invalid campaign fixture text"))
}
fn set(v: &mut Value, k: &str, x: Value) {
    if let Value::Object(rows) = v {
        if let Some((_, value)) = rows.iter_mut().find(|(key, _)| key == k) {
            *value = x;
        } else {
            rows.push((k.into(), x));
        }
    }
}
fn run(a: &Value) -> Result<Value> {
    let pack = get(a, "pack")?;
    CastPack::from_value(pack)?;
    let actor = text(a, "actor")?;
    let player = text(a, "player")?;
    let ch = get(pack, "characters")?
        .as_array()
        .and_then(|xs| xs.iter().find(|ch| ch.get("id").and_then(Value::as_str) == Some(actor)))
        .ok_or_else(|| Error::new("unknown actor"))?;
    let saved = get(a, "saved")?;
    let checkpoint = get(saved, "life")?;
    let d = Arc::new(Definition::draft(get(ch, "definition")?, &Setting::from_value(get(pack, "setting")?)?)?);
    let mut life = Life::accept(
        text(checkpoint, "scope")?,
        actor,
        get(checkpoint, "participants")?,
        d,
        Config::resolve(get(checkpoint, "config")?, &obj! {})?,
    )?;
    let state = get(checkpoint, "state")?;
    life.restore_checkpoint(get(a,"identity")?,checkpoint,&obj!{"min_revision"=>0,"max_revision"=>get(state,"revision")?.clone(),"min_token"=>0,"max_token"=>get(state,"watermark")?.clone(),"max_tick"=>get(state,"clock")?.clone()})?;
    let tools = InteractionTools::recover(get(saved, "interactions")?, 0, 1000)?;
    let notebook = tools.to_value();
    if text(&notebook, "scope")? != text(checkpoint, "scope")?
        || text(&notebook, "actor")? != actor
        || get(&notebook, "participants")? != get(checkpoint, "participants")?
    {
        return Err(Error::new("foreign interactions"));
    }
    let mut scene = PartyScene::from_value(get(a, "scene")?)?;
    for receipt in get(a, "receipts")?.as_array().ok_or_else(|| Error::new("invalid receipts"))? {
        scene.receipt(receipt)?;
    }
    let r = get(a, "speech")?;
    scene.receipt(&obj!{"token"=>4,"speaker"=>player,"recipients"=>vec![actor],"heard"=>vec![actor,player],"text"=>get(r,"input")?.clone(),"parent"=>Value::Null})?;
    let mut loadout = get(ch, "blueprint")?.clone();
    let mut modules = get(&loadout, "modules")?.clone();
    set(&mut modules, "recall", true.into());
    set(&mut modules, "promises", true.into());
    set(&mut loadout, "modules", modules);
    let b = Blueprint::from_value(&loadout)?.for_route(&ModelRoute::from_value(
        &obj! {"name"=>"campaign_local","size"=>"3b","calls"=>1,"cost_micros"=>0},
    )?)?;
    let gates = AttitudeGates::from_value(get(ch, "gates")?)?;
    let lore = Lore::from_value(get(pack, "lore")?)?;
    let q = obj! {"trigger"=>4,"revision"=>1000,"life_revision"=>life.revision(),"listeners"=>vec![player],"subjects"=>Vec::<Value>::new(),"tags"=>get(r,"tags")?.clone(),"turn"=>get(r,"turn")?.clone(),"rules"=>get(r,"rules")?.clone(),"stable_scene"=>get(r,"stable_scene")?.clone(),"lore_select"=>obj!{"actor"=>actor,"grants"=>Vec::<Value>::new(),"query"=>get(r,"input")?.clone(),"subjects"=>vec![actor],"required"=>Vec::<Value>::new(),"limit"=>0,"max_bytes"=>4096},"affordances"=>get(ch,"affordances")?.clone(),"tools"=>Vec::<Value>::new()};
    let mut result =
        scene.compile(&SceneTurn { life: &life, blueprint: &b, gates: &gates, lore: &lore, request: &q })?;
    let selected = life.recall_selected(get(a, "selection")?)?;
    if selected.as_array().map_or(true, |rows| rows.len() > b.caps().episodes) {
        return Err(Error::new("episode attention exceeded"));
    }
    let notes = get(&tools.to_value(), "notes")?
        .as_array()
        .ok_or_else(|| Error::new("notes"))?
        .iter()
        .filter(|n| n.get("subject").and_then(Value::as_str) == Some(player))
        .cloned()
        .collect::<Vec<_>>();
    let mut request = get(&result, "request")?.clone();
    let mut messages = get(&request, "messages")?.as_array().ok_or_else(|| Error::new("messages"))?.to_vec();
    let mut changing = gamectx::json::parse(text(&messages[2], "content")?, 65536)?;
    set(&mut changing, "episodes", selected.clone());
    set(&mut changing, "accepted_notes", notes.into());
    set(
        &mut changing,
        "accepted_promises",
        tools
            .accepted_promises()
            .into_iter()
            .filter(|p| p.get("promisor").and_then(Value::as_str) == Some(player))
            .collect::<Vec<_>>()
            .into(),
    );
    messages[2] = obj! {"role"=>"user","content"=>changing.to_json()};
    set(&mut request, "messages", messages.into());
    set(&mut result, "request", request);
    set(&mut result, "selected", selected);
    Ok(result)
}
fn main() {
    let mut raw = String::new();
    let result = io::stdin()
        .take(1_572_865)
        .read_to_string(&mut raw)
        .map_err(|_| Error::new("read failed"))
        .and_then(|_| gamectx::json::parse(&raw, 1_572_864))
        .and_then(|v| run(&v));
    match result {
        Ok(v) => println!("{}", v.to_json()),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
