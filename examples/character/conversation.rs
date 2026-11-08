//! Reuse dialogue memory and explicit action confirmation in any host.
use gamectx::{character::dialogue::Conversation, data::query, obj, runtime::decision::PendingAction, Result};
pub fn main() -> Result<()> {
    let mut conversation = Conversation::default();
    let ticket = conversation.prepare(&obj! {"scope"=>"player_1:trader","speaker"=>"trader","revision"=>7,
    "input"=>"My name is Fen.","facts"=>obj! {"price"=>3},"recall"=>2,"history"=>2})?;
    assert!(conversation.reply_schema()?.get("properties").is_some());
    conversation.finish(
        "player_1:trader",
        7,
        ticket.get("id").unwrap().as_str().unwrap(),
        r#"{"speech":"Welcome, Fen.","tone":"neutral","fact_ids":[],"memory_ids":[]}"#,
    )?;
    assert_eq!(conversation.state("player_1:trader")?.get("memories").unwrap().as_array().unwrap().len(), 1);
    // The host supplies exact inventory numbers; the model does no arithmetic.
    let rows = vec![obj! {"id"=>"oil","count"=>2}, obj! {"id"=>"chalk","count"=>4}];
    assert_eq!(query::run("SELECT SUM(count) FROM inventory", &[("inventory", &rows)])?.to_json(), "6");
    let terms = obj! {"unit_price"=>3,"quantity"=>1};
    let mut pending = PendingAction::new(&obj! {"id"=>"offer_1","scope"=>"player_1:trader","revision"=>7,
    "decision"=>obj! {"action"=>"buy","target"=>"item:oil"},"terms"=>terms.clone()})?;
    // In a real host this bool is a human response, and the predicate reads live
    // permissions/stock/funds. Confirmation returns a proposal, without executing it.
    let approved = pending.respond("player_1:trader", 7, true, &terms, |_| true)?;
    assert_eq!(approved.get("status").unwrap().as_str(), Some("confirmed"));
    assert!(pending.respond("player_1:trader", 7, true, &terms, |_| true).is_err());
    Ok(())
}
