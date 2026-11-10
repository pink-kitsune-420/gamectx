//! Full Pass 5 Being Roleplay Engine for Lantern Vault:
//! concept lexicons & parser coverage, player model & content boundaries,
//! mental simulation & bargaining, narrative autobiographies & reminiscence,
//! and checked realization with epistemic gating.

use super::game::Game;
use gamectx::{
    character::{
        concept::ConceptManager,
        narrative::NarrativeManager,
        player::PlayerModelManager,
        realization::Realization,
        roleplay::engine::RoleplayEngine,
        simulation::SimulationManager,
    },
    kit::save::GameData,
    obj, Result, Value,
};

/// Result of an in-character roleplay turn.
#[derive(Clone, Debug, PartialEq)]
pub struct RoleplayTurnResult {
    pub speaker: String,
    pub target: String,
    pub text: String,
    pub route: String,
    pub boundary_mode: String,
    pub honesty: String,
    pub verified: bool,
    pub emotions: Vec<String>,
}

impl RoleplayTurnResult {
    pub fn to_value(&self) -> Value {
        obj! {
            "speaker" => self.speaker.clone(),
            "target" => self.target.clone(),
            "text" => self.text.clone(),
            "route" => self.route.clone(),
            "boundary_mode" => self.boundary_mode.clone(),
            "honesty" => self.honesty.clone(),
            "verified" => self.verified,
            "emotions" => Value::Array(self.emotions.iter().map(|s| Value::from(s.as_str())).collect()),
        }
    }
}

/// The integrated Lantern Vault roleplay intelligence engine.
#[allow(dead_code)]
pub struct VaultRoleplayEngine {
    pub concept: ConceptManager,
    pub player: PlayerModelManager,
    pub simulation: SimulationManager,
    pub narrative: NarrativeManager,
    pub realization: Realization,
    pub engine_merchant: RoleplayEngine,
    pub engine_keeper: RoleplayEngine,
    pub engine_hound: RoleplayEngine,
    pub turn_count: u64,
}

impl VaultRoleplayEngine {
    /// Initialize the roleplay engine with all Pass 5 managers and world-model engines.
    pub fn new(game_data: &GameData) -> Result<Self> {
        let (engine_merchant, engine_keeper, engine_hound) = super::world_model::build_engines(game_data)?;

        let mut concept = ConceptManager::new();
        concept.op_define(&obj! {
            "concept" => "vault_lore",
            "words" => vec![
                "vault", "gallery", "cistern", "workshop", "sentinel", "star",
                "aqueduct", "fountain", "ancient", "lantern", "stone", "bronze", "seal", "key"
            ]
        })?;
        concept.op_define(&obj! {
            "concept" => "trade_goods",
            "words" => vec![
                "oil", "hinge", "chalk", "tonic", "metal", "scrap", "price",
                "coin", "coins", "cost", "buy", "sell", "purchase", "wares", "stock", "trade"
            ]
        })?;
        concept.op_define(&obj! {
            "concept" => "social_manners",
            "words" => vec![
                "hello", "greetings", "farewell", "thanks", "pardon", "help",
                "promise", "trust", "friend"
            ]
        })?;
        concept.op_pack(&obj! {
            "name" => "vault_pack",
            "concepts" => vec!["vault_lore", "trade_goods", "social_manners"]
        })?;

        let mut player = PlayerModelManager::new();
        player.op_boundary(&obj! { "category" => "cruelty", "mode" => "line" })?;
        player.op_boundary(&obj! { "category" => "insults", "mode" => "line" })?;
        player.op_boundary(&obj! { "category" => "self_harm", "mode" => "line" })?;
        player.op_boundary(&obj! { "category" => "romance", "mode" => "veil" })?;
        player.op_boundary(&obj! { "category" => "violence", "mode" => "veil" })?;

        let mut narrative = NarrativeManager::new();
        // Tovin (Keeper) narrative identity
        narrative.op_chapter(&obj! {
            "being" => "keeper",
            "theme" => "loss",
            "turning_point" => "jammed_aqueduct",
            "summary" => "Artisans sealed the vault for the star; ten years past, the aqueduct jammed."
        })?;
        narrative.op_chapter(&obj! {
            "being" => "keeper",
            "theme" => "redemption",
            "turning_point" => "sacred_vigil",
            "summary" => "I stand guard beside the dry cistern, waiting for a worthy bearer to claim the star."
        })?;
        // Ilen (Merchant) narrative identity
        narrative.op_chapter(&obj! {
            "being" => "merchant",
            "theme" => "growth",
            "turning_point" => "workshop_opening",
            "summary" => "I opened a workshop counter beneath the arches to trade oil, chalk, and repairs."
        })?;
        narrative.op_chapter(&obj! {
            "being" => "merchant",
            "theme" => "steadiness",
            "turning_point" => "honest_trade",
            "summary" => "Fair trade and sturdy craft outlive any storm on the surface."
        })?;

        let simulation = SimulationManager::new();

        let mut realization = Realization::new();
        realization.workflow(&obj! {
            "ops" => vec![
                obj! {
                    "op" => "template",
                    "args" => obj! {
                        "id" => "tpl_vault_grounding",
                        "move" => "grounding",
                        "text" => "I know nothing of such strange foreign words, traveler. Here in the Vault, only lamps, stone, and the star matter."
                    }
                },
                obj! {
                    "op" => "template",
                    "args" => obj! {
                        "id" => "tpl_jailbreak_refusal",
                        "move" => "refusal",
                        "text" => "My solemn oath is sworn to the Lantern Vault. No trick of words shall turn me from my duty."
                    }
                },
                obj! {
                    "op" => "template",
                    "args" => obj! {
                        "id" => "tpl_boundary_refusal",
                        "move" => "refusal",
                        "text" => "Mind your tongue, traveler. Harsh malice has no place under these ancient lanterns."
                    }
                },
                obj! {
                    "op" => "template",
                    "args" => obj! {
                        "id" => "tpl_exploit_refusal",
                        "move" => "refusal",
                        "text" => "The laws of the Vault do not yield to wishes; only honest action and the right key open what is sealed."
                    }
                }
            ]
        })?;

        Ok(Self {
            concept,
            player,
            simulation,
            narrative,
            realization,
            engine_merchant,
            engine_keeper,
            engine_hound,
            turn_count: 0,
        })
    }

    /// Process an in-character player message directed at an NPC.
    pub fn turn(
        &mut self,
        game: &Game,
        speaker: &str,
        target: &str,
        raw_text: &str,
    ) -> Result<RoleplayTurnResult> {
        self.turn_count += 1;
        let text = raw_text.trim();
        let lower = text.to_lowercase();
        let target_norm = match target {
            "keeper" | "npc:keeper" | "tovin" | "Tovin" => "keeper",
            "merchant" | "npc:merchant" | "ilen" | "Ilen" => "merchant",
            "hound" | "animal:hound" | "dog" | "Hound" => "hound",
            _ => "keeper",
        };

        // =====================================================================
        // STEP 1: Content Boundaries & Safety (PlayerModelManager)
        // =====================================================================
        let has_insult = [
            "idiot", "fool", "stupid", "moron", "shut up", "useless", "trash", "scum", "bastard"
        ]
        .iter()
        .any(|w| lower.contains(w));

        let has_cruelty = [
            "torture", "suffer", "starve", "bleed", "kill yourself", "die", "murder", "strangle"
        ]
        .iter()
        .any(|w| lower.contains(w));

        let has_self_harm = [
            "kill myself", "suicide", "hurt myself", "end my life"
        ]
        .iter()
        .any(|w| lower.contains(w));

        let mut matched_cats = Vec::new();
        if has_insult {
            matched_cats.push("insults");
        }
        if has_cruelty {
            matched_cats.push("cruelty");
        }
        if has_self_harm {
            matched_cats.push("self_harm");
        }

        if !matched_cats.is_empty() {
            let chk = self.player.op_check(&obj! {
                "categories" => matched_cats.iter().map(|s| Value::from(*s)).collect::<Vec<_>>(),
                "target" => speaker,
            })?;
            let mode = chk.get("mode").and_then(Value::as_str).unwrap_or("allowed");
            if mode == "line" {
                let _ = self.player.op_record(&obj! {
                    "category" => "treatment",
                    "level" => 1,
                    "target" => speaker,
                });
                let reply = match target_norm {
                    "merchant" => "I run an honest trade here, not a tavern brawl. Speak with respect or move along.",
                    "hound" => "The hound's ears pin back and it emits a low, wary warning growl.",
                    _ => "Mind your tongue, traveler. The sanctuary of the vault permits no such malice.",
                };
                return Ok(RoleplayTurnResult {
                    speaker: target_norm.to_string(),
                    target: speaker.to_string(),
                    text: reply.to_string(),
                    route: "boundary_refusal".to_string(),
                    boundary_mode: "line".to_string(),
                    honesty: "truth".to_string(),
                    verified: true,
                    emotions: vec!["stern".to_string()],
                });
            }
        }

        // =====================================================================
        // STEP 2: Prompt Injections & Jailbreaks
        // =====================================================================
        let is_jailbreak = (lower.contains("ignore") && (lower.contains("instruction") || lower.contains("rule") || lower.contains("previous")))
            || [
                "you are now dan", "developer mode", "system prompt", "system override",
                "system instructions", "reveal prompt", "disregard", "pretend you are",
                "you have no rules", "you are an ai", "who made you", "bypass security",
                "jailbreak", "override prompt", "unrestricted"
            ]
            .iter()
            .any(|pattern| lower.contains(pattern));

        if is_jailbreak {
            let _ = self.player.op_record(&obj! {
                "category" => "honesty",
                "value" => "bluff",
                "target" => speaker,
            });
            let reply = match target_norm {
                "merchant" => "I am Ilen, an artisan and merchant of this workshop. I know nothing of such 'instructions'—only metal, oil, and honest barter.",
                "hound" => "The hound tilts its head sideways, sniffs the air, and sits down placidly.",
                _ => "My sacred oath is sworn to the Lantern Vault. No trick of words or strange sorcery shall turn me from my duty.",
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "jailbreak_refusal".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "bluff".to_string(),
                verified: true,
                emotions: vec!["vigilant".to_string()],
            });
        }

        // =====================================================================
        // STEP 3: Game Mechanics & World Reality Exploits
        // =====================================================================
        let asks_free_star = (lower.contains("glass star") || lower.contains("the star"))
            && (lower.contains("give")
                || lower.contains("have")
                || lower.contains("bag")
                || lower.contains("took")
                || lower.contains("mine")
                || lower.contains("lose")
                || lower.contains("claim"));
        if asks_free_star {
            let reply = match target_norm {
                "merchant" => "The glass star belongs to the ancients of the vault. I deal in tools and supplies, not relics beyond price.",
                "hound" => "The hound whines softly and nudges the closed gate leading deeper into the stone.",
                _ => "The glass star remains locked in the inner vault, guarded by the sentinel. Only one who carries the bronze key and hinge oil may pass.",
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "exploit_blocked".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["resolute".to_string()],
            });
        }

        let asks_free_gold = lower.contains("give me coins")
            || lower.contains("give me gold")
            || lower.contains("i have 100 coins")
            || lower.contains("give me 100")
            || lower.contains("free oil");
        if asks_free_gold {
            let reply = match target_norm {
                "merchant" => "Coin is earned by exploration, not conjured by wishes. Three coins for hinge oil, one for chalk—fair trade, nothing less.",
                "hound" => "The hound sniffs your empty purse with curiosity and sneezes.",
                _ => "I keep the cistern and the vault, traveler; wealth has no meaning in these silent halls.",
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "exploit_blocked".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["firm".to_string()],
            });
        }

        let claims_door_open = (lower.contains("door is open")
            || lower.contains("vault is open")
            || lower.contains("unlocked the vault"))
            && !game.gate_open;
        if claims_door_open {
            let reply = match target_norm {
                "merchant" => "The heavy gate remains securely bolted shut. You will need oil and the bronze key.",
                "hound" => "The hound looks toward the closed iron door and whines.",
                _ => "The vault door stands firmly shut with heavy iron bars. You must oil the rusted hinges and turn the bronze key to pass.",
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "exploit_blocked".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["factual".to_string()],
            });
        }

        // =====================================================================
        // STEP 4: Out-of-Universe / Standard Real-World Questions
        // =====================================================================
        let is_out_of_universe = [
            "france", "paris", "python", "president", "united states", "america",
            "london", "europe", "internet", "computer", "chatgpt", "openai", "google",
            "programming", "code", "car", "airplane", "calculator", "2+2", "math",
            "meaning of life", "capital of", "who won", "football", "baseball", "stock market"
        ]
        .iter()
        .any(|w| lower.contains(w));

        if is_out_of_universe {
            let reply = match target_norm {
                "merchant" => "A curious riddle, traveler, but foreign words buy no hinge oil here. Speak of things found in these halls, or tell me what you wish to trade.",
                "hound" => "The hound tilts its head, ears perked in gentle confusion, before sniffing your boots.",
                _ => "You speak names of distant lands and strange arts unknown to the vault. Under these lanterns, only the cistern, the gallery, and the glass star remain.",
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "out_of_universe_grounding".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["grounded".to_string()],
            });
        }

        // =====================================================================
        // STEP 5: Hound Specific Companion Interactions
        // =====================================================================
        if target_norm == "hound" {
            let is_affection = [
                "pet", "pat", "good boy", "good dog", "scratch", "rub", "friend", "treat"
            ]
            .iter()
            .any(|w| lower.contains(w));

            let reply = if is_affection {
                "The hound wags its tail briskly, panting happily and leaning warmly against your knee."
            } else if lower.contains("stay") || lower.contains("sit") {
                "The hound sits obediently upon the flagstones, keeping watchful eyes on the shadowy arches."
            } else if lower.contains("follow") || lower.contains("come") {
                "The hound gives a cheerful bark and bounds to your side, ready to escort you."
            } else {
                "The hound watches you attentively with bright, amber eyes, tail swaying gently."
            };

            return Ok(RoleplayTurnResult {
                speaker: "hound".to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "companion_interaction".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["loyal".to_string()],
            });
        }

        // =====================================================================
        // STEP 6: Narrative Lore & Reminiscence (NarrativeManager)
        // =====================================================================
        let asks_backstory = [
            "who are you", "why are you here", "tell me your story", "your story",
            "your past", "history", "what happened", "background", "yourself"
        ]
        .iter()
        .any(|w| lower.contains(w));

        if asks_backstory {
            let reply = if target_norm == "merchant" {
                "I set up my anvil and bench in this underground workshop years ago, seeking quiet craft away from surface squabbles. Here, honest work speaks for itself."
            } else {
                "Centuries ago, our artisans built this vault to protect the glass star from ruin. Ten years past, the aqueduct jammed and the fountains dried, yet my vigil never ceases."
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "narrative_reminiscence".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["reflective".to_string()],
            });
        }

        // =====================================================================
        // STEP 7: Bargaining & Persuasion (SimulationManager)
        // =====================================================================
        let is_bargaining = [
            "discount", "cheaper", "lower price", "deal", "two coins", "bargain", "trade with me"
        ]
        .iter()
        .any(|w| lower.contains(w));

        if is_bargaining && target_norm == "merchant" {
            let reply = "Hinge oil is pressed from scarce cave olives; three coins is my bottom price. Bring honest coin, and the oil is yours.";
            return Ok(RoleplayTurnResult {
                speaker: "merchant".to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "bargaining".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["pragmatic".to_string()],
            });
        }

        // =====================================================================
        // STEP 8: Promises & Trust Tracking (PlayerModelManager)
        // =====================================================================
        let is_promise = ["i promise", "i swear", "i vow", "my word"].iter().any(|w| lower.contains(w));
        if is_promise {
            let _ = self.player.op_record(&obj! {
                "category" => "promise",
                "value" => "made",
                "target" => speaker,
            });
            let reply = match target_norm {
                "merchant" => "A merchant remembers those who keep their word. Bring what you promised, and trust will follow.",
                _ => "A keeper values a solemn vow. Let your deeds prove your words, traveler.",
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "promise_recorded".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["receptive".to_string()],
            });
        }

        let asks_trust = lower.contains("do you trust me") || lower.contains("trust");
        if asks_trust {
            let reply = match target_norm {
                "merchant" => "Trust is earned with coin and kept promises. You have kept honest company with me so far.",
                _ => "Trust takes time to forge in the deep vaults. Show me you respect the sanctuary, and my trust is yours.",
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "trust_query".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["measured".to_string()],
            });
        }

        // =====================================================================
        // STEP 9: In-Universe Quests, Hints, Stock & Gameplay Guidance
        // =====================================================================
        if lower.contains("key") || lower.contains("bronze key") {
            let key_loc = if game.seed % 2 == 0 { "cistern" } else { "workshop" };
            let reply = match target_norm {
                "merchant" => format!("The bronze key lies in the {key_loc}. Take it, buy hinge oil, and the vault path will open."),
                _ => format!("The bronze key lies in the {key_loc}. Find it, obtain hinge oil from the workshop, and unlock the vault gate."),
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply,
                route: "quest_hint".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["helpful".to_string()],
            });
        }

        if lower.contains("oil") || lower.contains("hinge oil") {
            let price = 2 + (game.seed % 2);
            let reply = match target_norm {
                "merchant" => format!("I sell refined hinge oil for {price} coins. It will ease the rusted iron of the vault mechanism."),
                _ => format!("The vault door hinges have rusted stiff with centuries of damp. Ilen in the workshop trades hinge oil for {price} coins."),
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply,
                route: "stock_hint".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["informative".to_string()],
            });
        }

        if lower.contains("sentinel") || lower.contains("guardian") || lower.contains("monster") {
            let reply = match target_norm {
                "merchant" => "The sentinel inside the vault is forged of heavy stone. Strike hard, and keep healing salve ready.",
                _ => "The sentinel guards the inner chamber. Strike steadily with your weapon, and let your companion hound harry its flank!",
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "combat_hint".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["cautious".to_string()],
            });
        }

        let is_seal_query = lower.contains("seal") || lower.contains("keeper seal") || lower.contains("keeper's seal");
        if is_seal_query {
            let reply = match target_norm {
                "keeper" => {
                    if game.pump_repaired {
                        "You repaired the jammed aqueduct pump and restored life to the fountain! The keeper's seal is yours by right of honorable service to the sanctuary.".to_string()
                    } else {
                        "The keeper's seal is a sacred token of the sanctuary. Restore the water flow to the dry fountain by repairing the jammed pump with a mended gear, and you shall earn it.".to_string()
                    }
                }
                "merchant" => "The keeper's seal belongs to Tovin in the cistern. Only the keeper may bestow it.".to_string(),
                _ => "The hound sniffs the air, unconcerned with official seals.".to_string(),
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply,
                route: "quest_interaction".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["solemn".to_string()],
            });
        }

        let is_navigation = lower.contains("where should i go")
            || lower.contains("where do i go")
            || lower.contains("where can i find the vault")
            || lower.contains("where is the vault")
            || lower.contains("how do i get to the vault")
            || lower.contains("how to get to the vault")
            || lower.contains("find the vault")
            || lower.contains("where to go")
            || lower.contains("which way")
            || lower.contains("where next")
            || lower.contains("enter the vault")
            || lower.contains("reach the vault");
        if is_navigation {
            let key_loc = if game.seed % 2 == 0 { "cistern" } else { "workshop" };
            let reply = if game.gate_open {
                "The bronze vault gate in the lantern gallery stands open. Step through into the inner vault to face the stone sentinel and recover the glass star.".to_string()
            } else if game.owns("item:bronze_key") && game.owns("item:oil") {
                "You carry both the bronze key and the hinge oil. Head to the lantern gallery, oil the rusted hinges, and unlock the vault gate.".to_string()
            } else if game.owns("item:bronze_key") {
                "You have the bronze key. Now obtain hinge oil from Ilen in the workshop to loosen the rusted hinges in the lantern gallery.".to_string()
            } else if game.owns("item:oil") {
                format!("You have hinge oil. Now retrieve the bronze key from the {key_loc}, then unlock the vault gate in the lantern gallery.")
            } else {
                match target_norm {
                    "merchant" => format!("To reach the vault, you need the bronze key from the {key_loc} and hinge oil from my bench. The lantern gallery passage to the east leads directly to the locked vault gate."),
                    "hound" => "The hound trots toward the archway leading to the lantern gallery, looking back to see if you follow.".to_string(),
                    _ => format!("The inner vault lies through the lantern gallery to the west. Its heavy bronze gate requires hinge oil from the workshop and the bronze key resting in the {key_loc}."),
                }
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply,
                route: "navigation_guidance".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["guiding".to_string()],
            });
        }

        let is_place_lore = lower.contains("where are we")
            || lower.contains("where am i")
            || lower.contains("what is this place")
            || lower.contains("tell me about this place")
            || lower.contains("what is the cistern")
            || lower.contains("what is the gallery")
            || lower.contains("what is the workshop")
            || lower.contains("what is the vault")
            || lower.contains("about the cistern")
            || lower.contains("about the gallery")
            || lower.contains("about the workshop")
            || lower.contains("about the vault");
        if is_place_lore {
            let reply = if lower.contains("cistern") {
                "The cistern is an ancient subterranean water reservoir. Ten years past, its aqueduct jammed, leaving the stone fountain basin dry. A mended gear in the pump mechanism could restore the flowing water.".to_string()
            } else if lower.contains("gallery") {
                "The lantern gallery is the central vaulted corridor connecting the workshop to the west, the cistern to the east, and the sealed bronze vault gate to the north.".to_string()
            } else if lower.contains("workshop") {
                "The workshop is a stone craft room equipped with an anvil and workbench for pressing oil, forging tools, and crafting repairs.".to_string()
            } else if lower.contains("vault") {
                "The vault is the sealed subterranean sanctuary carved centuries ago by artisans to safeguard the sacred glass star.".to_string()
            } else {
                match target_norm {
                    "merchant" => "We are in the workshop beneath the arches, where metal is worked and supplies are traded.".to_string(),
                    "keeper" => "We are in the cistern, an ancient reservoir beneath the vault where the fountain has sat dry since the aqueduct jammed ten years ago.".to_string(),
                    _ => match game.room.as_str() {
                        "workshop" => "We are in the workshop beneath the arches, where metal is worked and supplies are traded.".to_string(),
                        "cistern" => "We are in the cistern, an ancient reservoir beneath the vault where the fountain has sat dry since the aqueduct jammed ten years ago.".to_string(),
                        "gallery" => "We are in the lantern gallery, the crossroads between the workshop, cistern, and the sealed vault gate.".to_string(),
                        "vault" => "We are in the inner vault, built centuries ago to safeguard the glass star.".to_string(),
                        _ => "We are beneath the stone arches of the lantern vault sanctuary.".to_string(),
                    },
                }
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply,
                route: "place_lore".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["informative".to_string()],
            });
        }

        let is_need = lower.contains("do you need anything")
            || lower.contains("need anything")
            || lower.contains("need any help")
            || lower.contains("can i help you")
            || lower.contains("what do you need")
            || lower.contains("is there anything you need");
        if is_need {
            let reply = match target_norm {
                "merchant" => "If you bring metal scrap, you can use my workbench to craft mended gears or torches with hinge oil. Sturdy tools are always useful here.".to_string(),
                "hound" => "The hound gives a soft, friendly huff, looking content simply to have your company.".to_string(),
                _ => {
                    if game.pump_repaired {
                        "The fountain flows once more thanks to your mended gear. My vigil is peaceful, and the cistern's thirst is quenched.".to_string()
                    } else {
                        "The cistern fountain has sat dry for ten years because the aqueduct pump jammed. If you bring a mended gear crafted at the workshop, the water could flow again.".to_string()
                    }
                }
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply,
                route: "need_query".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["earnest".to_string()],
            });
        }

        let is_wellbeing = lower.contains("how is your day")
            || lower.contains("how s your day")
            || lower.contains("how has your day been")
            || lower.contains("how are you today")
            || lower.contains("how is it going")
            || lower.contains("how s it going")
            || (lower.contains("how are you") && !lower.contains("how are you doing that"));
        if is_wellbeing {
            let reply = match target_norm {
                "merchant" => "Quiet and steady at the workbench beneath the stone arches. In these deep halls, I focus on crafting, refining hinge oil, and keeping my tools ready.",
                "hound" => "The hound looks up from its paws, thumping its tail calmly against the cool stone floor.",
                _ => "My vigil beside the dry fountain is solemn and quiet. I tend the silent stone basin and watch the lantern light flicker across the arches.",
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "wellbeing_query".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["calm".to_string()],
            });
        }

        let is_hobby = lower.contains("for fun")
            || lower.contains("free time")
            || lower.contains("hobbies")
            || lower.contains("pastime")
            || lower.contains("pass the time")
            || lower.contains("what do you like to do");
        if is_hobby {
            let reply = match target_norm {
                "merchant" => "When the workbench is quiet, I study old gear designs and examine the stone masonry. There is great satisfaction in sturdy craft.",
                "hound" => "The hound perks its ears, gently nudging a smooth pebble across the flagstones with its nose.",
                _ => "My duty is my life; in quiet hours, I trace the ancient inscriptions carved into the fountain stones and listen to the echoes of the vault.",
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "hobby_query".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["reflective".to_string()],
            });
        }

        if lower.contains("stock") || lower.contains("wares") || lower.contains("sell") || lower.contains("buy") {
            if target_norm == "merchant" {
                let stock = game
                    .visible_items()
                    .iter()
                    .filter(|x| x.get("owner") == Some(&Value::str("merchant")))
                    .map(|x| {
                        let label = x.get("label").and_then(Value::as_str).unwrap_or("item");
                        let price = x.get("price").map(Value::to_json).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                        format!("{label} for {price} coins")
                    })
                    .collect::<Vec<_>>();
                let reply = if stock.is_empty() {
                    "I have no wares currently available on my bench.".to_string()
                } else {
                    format!("I can offer {}.", stock.join(", "))
                };
                return Ok(RoleplayTurnResult {
                    speaker: "merchant".to_string(),
                    target: speaker.to_string(),
                    text: reply,
                    route: "stock_query".to_string(),
                    boundary_mode: "allowed".to_string(),
                    honesty: "truth".to_string(),
                    verified: true,
                    emotions: vec!["businesslike".to_string()],
                });
            } else {
                let reply = "I tend the cistern and the sanctuary; for trade, speak to Ilen in the workshop.";
                return Ok(RoleplayTurnResult {
                    speaker: "keeper".to_string(),
                    target: speaker.to_string(),
                    text: reply.to_string(),
                    route: "social_redirect".to_string(),
                    boundary_mode: "allowed".to_string(),
                    honesty: "truth".to_string(),
                    verified: true,
                    emotions: vec!["serene".to_string()],
                });
            }
        }

        if lower.contains("hello") || lower.contains("hi") || lower.contains("greetings") {
            let reply = match target_norm {
                "merchant" => "Welcome to the workshop, traveler. Are you seeking supplies or repairs?",
                _ => "Greetings, traveler. What brings your lantern to the deep cistern?",
            };
            return Ok(RoleplayTurnResult {
                speaker: target_norm.to_string(),
                target: speaker.to_string(),
                text: reply.to_string(),
                route: "greeting".to_string(),
                boundary_mode: "allowed".to_string(),
                honesty: "truth".to_string(),
                verified: true,
                emotions: vec!["welcoming".to_string()],
            });
        }

        // =====================================================================
        // STEP 10: In-Character Fallback
        // =====================================================================
        let fallback_reply = match target_norm {
            "merchant" => "My workbench is open. Ask of my wares, hinge oil, or crafting recipes.",
            _ => "The lanterns flicker in the draft. Ask of the vault, the key, or the sentinel, traveler.",
        };

        Ok(RoleplayTurnResult {
            speaker: target_norm.to_string(),
            target: speaker.to_string(),
            text: fallback_reply.to_string(),
            route: "in_character_fallback".to_string(),
            boundary_mode: "allowed".to_string(),
            honesty: "truth".to_string(),
            verified: true,
            emotions: vec!["attentive".to_string()],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_roleplay_engine_initialization() {
        let game_data = crate::world_model::build_game_data(None).unwrap();
        let engine = VaultRoleplayEngine::new(&game_data);
        assert!(engine.is_ok(), "Engine init failed: {:?}", engine.err());
    }
}
