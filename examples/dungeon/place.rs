//! Current actor-visible scene data for the reusable read-only location tool.
use super::game::Game;
use gamectx::{location::Locations, obj, Result, Value};
pub fn locations(game: &Game) -> Result<Locations> {
    let summary = match game.room.as_str() {
        "entry" => "The entrance stair above the lantern gallery.",
        "gallery" => "An underground gallery beside a bronze vault gate.",
        "workshop" => "An underground workshop where a merchant trades supplies.",
        "cistern" => "An underground cistern tended by its keeper.",
        "vault" => "The old vault chamber.",
        _ => "The daylight stair beyond the vault.",
    };
    let exits = game
        .choices()
        .iter()
        .filter(|p| p.get("action").and_then(Value::as_str) == Some("go"))
        .filter_map(|p| p.get("target").cloned())
        .collect::<Vec<_>>();
    let occupants = match game.room.as_str() {
        "workshop" => vec!["npc:merchant"],
        "cistern" => vec!["npc:keeper"],
        "vault" if game.enemy_hp > 0 => vec!["npc:sentinel"],
        _ => vec![],
    };
    Locations::from_value(
        &obj! {"version"=>1,"revision"=>game.revision,"places"=>vec![obj!{"id"=>format!("room:{}",game.room),"label"=>game.room.clone(),"summary"=>summary,"description"=>summary,"details"=>obj!{"visible_items"=>game.visible_items(),"exits"=>exits,"occupants"=>occupants,"observations"=>obj!{"outdoor_weather"=>Value::Null,"outdoor_weather_observed"=>false}}}]},
    )
}

pub fn world(game: &Game) -> Result<gamectx::world::WorldState> {
    let minutes = 8 * 60 + game.revision * 10;
    gamectx::world::WorldState::from_value(
        &obj! {"version"=>1,"id"=>"world:lantern_vault","revision"=>game.revision,"states"=>vec![
        obj!{"key"=>"time","value"=>obj!{"tick"=>game.revision,"label"=>format!("day {}, {:02}:{:02}",1+minutes/1440,(minutes/60)%24,minutes%60),"calendar"=>obj!{"day"=>1+minutes/1440,"hour"=>(minutes/60)%24,"minute"=>minutes%60},"minutes_per_action"=>10}},
        obj!{"key"=>"weather","value"=>Value::Null},obj!{"key"=>"season","value"=>Value::Null},obj!{"key"=>"lighting","value"=>Value::Null},obj!{"key"=>"temperature","value"=>Value::Null},obj!{"key"=>"events","value"=>obj!{"vault_gate_open"=>game.gate_open}}]},
    )
}
