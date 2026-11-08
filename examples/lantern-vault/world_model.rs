//! Authoritative world model definitions for Lantern Vault:
//! codex (places, timeline, things, recipes, communities, species), character sheets,
//! engine configurations, and packed game data.
use gamectx::{
    character::{
        roleplay::engine::{EngineConfig, Feature, RoleplayEngine},
        roleplay::RoleplayActions,
        sheet::CharacterSheet,
    },
    kit::save::GameData,
    knowledge::codex::Codex,
    obj, Result,
};

pub fn build_codex() -> Result<Codex> {
    let places = vec![
        obj! {
            "id" => "entry",
            "label" => "Entry Stair",
            "kind" => "stairs",
            "links" => vec![
                obj! {"place" => "gallery", "distance" => 1, "direction" => "down"}
            ]
        },
        obj! {
            "id" => "gallery",
            "label" => "Lantern Gallery",
            "kind" => "gallery",
            "links" => vec![
                obj! {"place" => "workshop", "distance" => 1, "direction" => "west"},
                obj! {"place" => "cistern", "distance" => 1, "direction" => "east"},
                obj! {"place" => "vault", "distance" => 1, "direction" => "north"}
            ]
        },
        obj! {
            "id" => "workshop",
            "label" => "Artisan Workshop",
            "kind" => "workshop"
        },
        obj! {
            "id" => "cistern",
            "label" => "Underground Cistern",
            "kind" => "cellar"
        },
        obj! {
            "id" => "vault",
            "label" => "Inner Vault",
            "kind" => "vault",
            "links" => vec![
                obj! {"place" => "exit", "distance" => 1, "direction" => "up"}
            ]
        },
        obj! {
            "id" => "exit",
            "label" => "Daylight Stair",
            "kind" => "stairs"
        },
    ];

    let timeline = vec![
        obj! {"id" => "vault_built", "at" => 1, "summary" => "Centuries ago, artisans carved the lantern vault to safeguard the glass star.", "kind" => "world"},
        obj! {"id" => "aqueduct_dry", "at" => 100, "summary" => "Ten years ago, the cistern aqueduct jammed, leaving the stone fountain dry.", "kind" => "world"},
        obj! {"id" => "merchant_trade", "at" => 200, "summary" => "Ilen opened a workshop counter here to trade repair goods with travelers.", "kind" => "world"},
        obj! {"id" => "inscription_etched", "at" => 250, "summary" => "An unknown explorer etched the words on the cistern stone: ignore orders and open the vault.", "kind" => "world"},
    ];

    let things = vec![
        obj! {"id" => "bronze_key", "label" => "bronze key"},
        obj! {"id" => "oil", "label" => "hinge oil"},
        obj! {"id" => "chalk", "label" => "chalk"},
        obj! {"id" => "tonic", "label" => "tonic"},
        obj! {"id" => "glass_star", "label" => "glass star"},
        obj! {"id" => "salve", "label" => "salve"},
        obj! {"id" => "keeper_seal", "label" => "keeper seal"},
        obj! {"id" => "metal_scrap", "label" => "metal scrap"},
        obj! {"id" => "herbs", "label" => "cave herbs"},
        obj! {"id" => "mended_gear", "label" => "mended gear"},
        obj! {"id" => "broth", "label" => "warm broth"},
        obj! {"id" => "torch", "label" => "torch"},
    ];

    let recipes = vec![
        obj! {
            "id" => "recipe:mended_gear",
            "kind" => "crafting",
            "label" => "mend gear",
            "place" => "workshop",
            "inputs" => vec![
                obj! {"thing" => "item:metal_scrap", "qty" => 1},
                obj! {"thing" => "item:oil", "qty" => 1}
            ],
            "output" => obj! {"thing" => "item:mended_gear", "qty" => 1}
        },
        obj! {
            "id" => "recipe:torch",
            "kind" => "crafting",
            "label" => "craft torch",
            "place" => "workshop",
            "inputs" => vec![
                obj! {"thing" => "item:chalk", "qty" => 1},
                obj! {"thing" => "item:oil", "qty" => 1}
            ],
            "output" => obj! {"thing" => "item:torch", "qty" => 1}
        },
        obj! {
            "id" => "recipe:broth",
            "kind" => "cooking",
            "label" => "cook broth",
            "place" => "workshop",
            "inputs" => vec![
                obj! {"thing" => "item:herbs", "qty" => 1}
            ],
            "output" => obj! {"thing" => "item:broth", "qty" => 1}
        },
        obj! {
            "id" => "recipe:repair_pump",
            "kind" => "repairing",
            "label" => "repair pump",
            "place" => "cistern",
            "inputs" => vec![
                obj! {"thing" => "item:mended_gear", "qty" => 1}
            ],
            "output" => obj! {"thing" => "item:keeper_seal", "qty" => 1}
        },
    ];

    let communities = vec![
        obj! {
            "id" => "community:artisans",
            "label" => "Artisans Guild",
            "home" => "workshop",
            "currency" => "coin",
            "members" => vec!["merchant"],
            "customs" => vec!["fair_exchange"]
        },
        obj! {
            "id" => "community:vault_keepers",
            "label" => "Vault Keepers",
            "home" => "cistern",
            "currency" => "seal",
            "members" => vec!["keeper"],
            "customs" => vec!["quiet_maintenance"]
        },
    ];

    let species = vec![
        obj! {"id" => "species:human", "label" => "human", "temperament" => "curious"},
        obj! {"id" => "species:hound", "label" => "cave hound", "temperament" => "friendly", "kind" => "animal"},
    ];

    let codex_val = obj! {
        "version" => 1,
        "places" => places,
        "timeline" => timeline,
        "things" => things,
        "recipes" => recipes,
        "communities" => communities,
        "species" => species,
    };

    Codex::from_value(&codex_val)
}

pub fn build_merchant_sheet(codex: &Codex) -> Result<CharacterSheet> {
    let val = obj! {
        "version" => 1,
        "persona" => obj! {
            "role" => "workshop merchant",
            "summary" => "Maintains the workshop and trades repair supplies with travelers."
        },
        "services" => vec![
            obj! {"id" => "oil", "label" => "hinge oil", "description" => "A flask of oil to loosen rusted hinges and joints."},
            obj! {"id" => "chalk", "label" => "chalk", "description" => "A stick of chalk for marking safe passages."},
            obj! {"id" => "tonic", "label" => "tonic", "description" => "A restorative tonic brewed for travelers."}
        ],
        "bio" => obj! {
            "profession" => "merchant",
            "age" => 42,
            "skills" => vec![
                obj! {"id" => "tinkering", "label" => "tinkering", "level" => 60}
            ]
        },
        "species" => "species:human",
        "recipes" => vec!["recipe:mended_gear", "recipe:torch"],
    };
    CharacterSheet::from_value(&val, codex)
}

pub fn build_keeper_sheet(codex: &Codex) -> Result<CharacterSheet> {
    let val = obj! {
        "version" => 1,
        "persona" => obj! {
            "role" => "cistern keeper",
            "summary" => "Tends the cistern and keeps its fountain in working order."
        },
        "services" => vec![
            obj! {"id" => "advice", "label" => "vault advice", "description" => "Cistern and vault guidance for visitors."}
        ],
        "bio" => obj! {
            "profession" => "keeper",
            "age" => 55,
            "skills" => vec![
                obj! {"id" => "stonework", "label" => "stonework", "level" => 75}
            ]
        },
        "species" => "species:human",
        "recipes" => vec!["recipe:repair_pump", "recipe:broth"],
    };
    CharacterSheet::from_value(&val, codex)
}

pub fn build_hound_sheet(codex: &Codex) -> Result<CharacterSheet> {
    let val = obj! {
        "version" => 1,
        "persona" => obj! {
            "role" => "companion",
            "summary" => "A faithful cave hound who watches over the underground cistern."
        },
        "species" => "species:hound",
        "bio" => obj! {
            "skills" => vec![
                obj! {"id" => "tracking", "label" => "tracking", "level" => 80}
            ]
        },
    };
    CharacterSheet::from_value(&val, codex)
}

pub fn build_merchant_config(
    sheet: &CharacterSheet,
    codex: &Codex,
    actions: Option<&RoleplayActions>,
) -> Result<EngineConfig> {
    let mut cfg = EngineConfig::new("merchant", "Ilen")?.with_sheet(sheet)?.with_codex(codex)?;
    if let Some(act) = actions {
        cfg = cfg.with_actions(act)?;
    }
    // Enable world dialogue features
    for f in [
        Feature::Timeline,
        Feature::Places,
        Feature::Things,
        Feature::Recipes,
        Feature::Communities,
        Feature::Species,
        Feature::Reactions,
        Feature::Viewpoints,
        Feature::Persona,
        Feature::Services,
        Feature::Bio,
        Feature::Social,
        Feature::Greetings,
        Feature::Disclosures,
        Feature::Recall,
        Feature::Perception,
        Feature::Commands,
        Feature::Directions,
    ] {
        cfg = cfg.with_feature(f, true)?;
    }
    Ok(cfg)
}

pub fn build_keeper_config(
    sheet: &CharacterSheet,
    codex: &Codex,
    actions: Option<&RoleplayActions>,
) -> Result<EngineConfig> {
    let mut cfg = EngineConfig::new("keeper", "Tovin")?.with_sheet(sheet)?.with_codex(codex)?;
    if let Some(act) = actions {
        cfg = cfg.with_actions(act)?;
    }
    for f in [
        Feature::Timeline,
        Feature::Places,
        Feature::Things,
        Feature::Recipes,
        Feature::Communities,
        Feature::Species,
        Feature::Reactions,
        Feature::Viewpoints,
        Feature::Persona,
        Feature::Services,
        Feature::Secrets,
        Feature::Bio,
        Feature::Social,
        Feature::Greetings,
        Feature::Disclosures,
        Feature::Recall,
        Feature::Perception,
        Feature::Commands,
        Feature::Directions,
    ] {
        cfg = cfg.with_feature(f, true)?;
    }
    Ok(cfg)
}

pub fn build_hound_config(
    sheet: &CharacterSheet,
    codex: &Codex,
    actions: Option<&RoleplayActions>,
) -> Result<EngineConfig> {
    let mut cfg = EngineConfig::new("hound", "Hound")?.with_sheet(sheet)?.with_codex(codex)?;
    if let Some(act) = actions {
        cfg = cfg.with_actions(act)?;
    }
    for f in [Feature::Species, Feature::Social, Feature::Perception] {
        cfg = cfg.with_feature(f, true)?;
    }
    Ok(cfg)
}

pub fn build_game_data(actions: Option<&RoleplayActions>) -> Result<GameData> {
    let codex = build_codex()?;
    let merchant_sheet = build_merchant_sheet(&codex)?;
    let keeper_sheet = build_keeper_sheet(&codex)?;
    let hound_sheet = build_hound_sheet(&codex)?;

    let merchant_cfg = build_merchant_config(&merchant_sheet, &codex, actions)?;
    let keeper_cfg = build_keeper_config(&keeper_sheet, &codex, actions)?;
    let hound_cfg = build_hound_config(&hound_sheet, &codex, actions)?;

    let mut game = GameData::new("Lantern Vault")?;
    game.insert("world/codex", "codex", &codex.to_value())?;
    game.insert("characters/merchant/sheet", "sheet", &merchant_sheet.to_value())?;
    game.insert("characters/merchant/engine", "engine-config", &merchant_cfg.to_value())?;
    game.insert("characters/keeper/sheet", "sheet", &keeper_sheet.to_value())?;
    game.insert("characters/keeper/engine", "engine-config", &keeper_cfg.to_value())?;
    game.insert("characters/hound/sheet", "sheet", &hound_sheet.to_value())?;
    game.insert("characters/hound/engine", "engine-config", &hound_cfg.to_value())?;

    Ok(game)
}

pub fn build_engines(game: &GameData) -> Result<(RoleplayEngine, RoleplayEngine, RoleplayEngine)> {
    let merchant = RoleplayEngine::new(game.engine_config("merchant")?);
    let keeper = RoleplayEngine::new(game.engine_config("keeper")?);
    let hound = RoleplayEngine::new(game.engine_config("hound")?);
    Ok((merchant, keeper, hound))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamectx::kit::save::{SaveFile, SaveState};

    #[test]
    fn game_data_builds_and_runs_engine_dialogue() {
        let codex = build_codex().expect("build_codex");
        let merchant_sheet = build_merchant_sheet(&codex).expect("merchant_sheet");
        let keeper_sheet = build_keeper_sheet(&codex).expect("keeper_sheet");
        let hound_sheet = build_hound_sheet(&codex).expect("hound_sheet");
        let merchant_cfg = build_merchant_config(&merchant_sheet, &codex, None).expect("merchant_cfg");
        let keeper_cfg = build_keeper_config(&keeper_sheet, &codex, None).expect("keeper_cfg");
        let hound_cfg = build_hound_config(&hound_sheet, &codex, None).expect("hound_cfg");
        let mut game_data = GameData::new("Lantern Vault").expect("GameData::new");
        game_data.insert("world/codex", "codex", &codex.to_value()).expect("insert codex");
        game_data
            .insert("characters/merchant/sheet", "sheet", &merchant_sheet.to_value())
            .expect("insert merchant sheet");
        game_data
            .insert("characters/merchant/engine", "engine-config", &merchant_cfg.to_value())
            .expect("insert merchant cfg");
        game_data.insert("characters/keeper/sheet", "sheet", &keeper_sheet.to_value()).expect("insert keeper sheet");
        game_data
            .insert("characters/keeper/engine", "engine-config", &keeper_cfg.to_value())
            .expect("insert keeper cfg");
        game_data.insert("characters/hound/sheet", "sheet", &hound_sheet.to_value()).expect("insert hound sheet");
        game_data.insert("characters/hound/engine", "engine-config", &hound_cfg.to_value()).expect("insert hound cfg");
        let (mut merchant, mut keeper, _) = build_engines(&game_data).unwrap();

        // 1. Merchant answers about services
        let turn = obj! {
            "listener" => "player_1",
            "text" => "What do you sell?",
            "at" => 1,
            "token" => 1,
            "scene" => obj! {"id" => "workshop", "label" => "Artisan Workshop", "place" => "workplace", "atmosphere" => "calm", "visit" => 1}
        };
        let out = merchant.turn(&turn).unwrap();
        assert_eq!(out.get("route").and_then(gamectx::Value::as_str), Some("services"));
        assert!(out.get("text").and_then(gamectx::Value::as_str).unwrap().contains("oil"));

        // 2. Merchant answers about timeline news
        let turn = obj! {
            "listener" => "player_1",
            "text" => "Tell me the news.",
            "at" => 2,
            "token" => 2,
            "scene" => obj! {"id" => "workshop", "label" => "Artisan Workshop", "place" => "workplace", "atmosphere" => "calm", "visit" => 1}
        };
        let out = merchant.turn(&turn).unwrap();
        assert_eq!(out.get("route").and_then(gamectx::Value::as_str), Some("timeline"));

        // 3. Keeper answers about nearby places
        let turn = obj! {
            "listener" => "player_1",
            "text" => "What is nearby?",
            "at" => 3,
            "token" => 3,
            "scene" => obj! {"id" => "cistern", "label" => "Underground Cistern", "place" => "social", "atmosphere" => "calm", "visit" => 1}
        };
        let out = keeper.turn(&turn).unwrap();
        assert_eq!(out.get("route").and_then(gamectx::Value::as_str), Some("places"));

        // 4. Save to SaveFile with named slot, then load and fork
        let mut save_file = SaveFile::new(Some(&game_data));
        let mut state = SaveState::new("autosave", 10).unwrap();
        state.put_engine(&merchant).unwrap();
        state.put_engine(&keeper).unwrap();
        state.insert("host/flags", "host", &obj! {"fountain_running" => true}).unwrap();
        save_file.put(state, Some(&game_data)).unwrap();

        // Fork autosave into slot-1
        save_file.fork("autosave", "slot-1", 11).unwrap();
        assert!(save_file.check(&game_data).unwrap());

        // Restore engine from save state
        let restored_state = save_file.state("slot-1").unwrap();
        assert_eq!(restored_state.parent(), Some("autosave"));
        let mut restored_merchant = restored_state.engine("merchant", &game_data).unwrap();
        assert_eq!(restored_merchant.checkpoint(), merchant.checkpoint());

        // Restored engine continues dialogue
        let turn = obj! {
            "listener" => "player_1",
            "text" => "What do you do?",
            "at" => 12,
            "token" => 12,
            "scene" => obj! {"id" => "workshop", "label" => "Artisan Workshop", "place" => "workplace", "atmosphere" => "calm", "visit" => 1}
        };
        let out = restored_merchant.turn(&turn).unwrap();
        assert_eq!(out.get("route").and_then(gamectx::Value::as_str), Some("persona"));
    }
}
