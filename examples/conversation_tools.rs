//! A multiplayer-capable API example; no network, model or game engine required.
use gamectx::{dialogue::InteractionTools, obj, Result, Value};
pub fn main() -> Result<()> {
    let mut tools =
        InteractionTools::new("shop:party", "npc:merchant", &vec!["npc:merchant", "player:fen", "player:mira"].into())?;
    let evidence: Value =
        vec![obj! {"id"=>"line:1","speaker"=>"player:fen","text"=>"I will bring you oil. Please speak quietly."}]
            .into();
    tools.publish(&obj!{"id"=>"trade:1","turn"=>7,"subject"=>"oil:17","kind"=>"ownership","value"=>obj!{"owner"=>"player:fen"},"audience"=>vec!["npc:merchant","player:fen"]})?;
    let events =
        tools.call("events_recent", r#"{"since":7,"subject":null,"limit":2}"#, &evidence, 7, 1024, |_| false)?;
    let promise = tools.call(
        "promise_remember",
        r#"{"promisor":"player:fen","beneficiary":"npc:merchant","task":"bring oil","source":"line:1"}"#,
        &evidence,
        7,
        1024,
        |_| false,
    )?;
    assert_eq!(promise.get("status"), Some(&Value::str("reported")));
    tools.set_progress(&obj!{"id"=>1,"expected"=>0,"state"=>"accepted","source"=>obj!{"id"=>"request:1","speaker"=>"engine","text"=>"The merchant accepted the request."}})?;
    tools.set_progress(&obj!{"id"=>1,"expected"=>1,"state"=>"fulfilled","source"=>obj!{"id"=>"delivery:1","speaker"=>"engine","text"=>"Fen delivered the oil."}})?;
    let promises =
        tools.call("promise_lookup", r#"{"participant":"player:fen","limit":2}"#, &evidence, 7, 2048, |_| false)?;
    tools.call(
        "relationship_note",
        r#"{"other":"player:fen","note":"Prefers quiet conversation.","source":"line:1"}"#,
        &evidence,
        7,
        1024,
        |_| false,
    )?;
    let relationship =
        tools.call("relationship_lookup", r#"{"other":"player:fen","limit":2}"#, &evidence, 7, 1024, |_| false)?;
    tools.set_references(&obj!{"entities"=>vec![obj!{"id"=>"key:bronze","label"=>"bronze key","kind"=>"item","aliases"=>vec!["key","bronze key"]},obj!{"id"=>"key:iron","label"=>"iron key","kind"=>"item","aliases"=>vec!["key","iron key"]}],"input"=>"Could I see the key?","kind"=>Value::Null,"scope"=>"shop:party","revision"=>7,"max_age"=>4,"focus"=>Value::Null,"selection"=>Value::Null,"pronouns"=>vec!["it"]})?;
    let clarification = tools.call("dialogue_resolve", r#"{"kind":"item"}"#, &evidence, 7, 1024, |_| false)?;
    assert_eq!(clarification.get("reason"), Some(&Value::str("ambiguous")));
    let decision = obj! {"action"=>"help","target"=>"player:fen"};
    let terms = obj! {"task"=>"repair lamp","cost"=>0};
    tools.set_offers(&vec![obj!{"recipient"=>"player:fen","offer"=>obj!{"id"=>"help:1","scope"=>"shop:party","revision"=>7,"decision"=>decision.clone(),"terms"=>terms.clone()}}].into())?;
    let offered = tools.call("interaction_offer", r#"{"id":"help:1"}"#, &evidence, 7, 1024, |p| p.same(&decision))?;
    assert!(tools.respond("player:mira", 7, true, &terms, |_| true).is_err());
    let confirmed = tools.respond("player:fen", 7, true, &terms, |p| p.same(&decision))?;
    assert_eq!(confirmed.get("status"), Some(&Value::str("confirmed")));
    // The host still executes help atomically, checking the same revision and terms.
    println!("{}",(obj!{"events"=>events,"promises"=>promises,"relationship"=>relationship,"clarification"=>clarification,"offered"=>offered,"confirmed"=>confirmed}).to_json());
    let names = [
        "events_recent",
        "promise_remember",
        "promise_lookup",
        "relationship_note",
        "relationship_lookup",
        "dialogue_resolve",
        "interaction_offers",
        "interaction_offer",
    ];
    println!("{}", tools.tools(&names)?.to_json());
    Ok(())
}
