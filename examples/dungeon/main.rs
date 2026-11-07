//! Lantern Vault: a deterministic playable game and JSON integration testbed.
mod conversation_policy;
mod game;
mod host;
mod intent;
mod knowledge;
mod natural;
mod place;
mod routines;
mod social;
mod trade;
use game::{fields, get, text, uint, GOAL};
use gamectx::{json, obj, Error, Result, Value};
use std::io::{self, BufRead, Read, Write};

fn line(reader: &mut impl BufRead) -> Result<Option<String>> {
    let mut bytes = Vec::new();
    let n = reader.take(16385).read_until(b'\n', &mut bytes).map_err(|e| Error::new(e.to_string()))?;
    if n == 0 {
        return Ok(None);
    }
    if n > 16384 {
        return Err(Error::new("input line exceeds 16384 bytes; session closed"));
    }
    String::from_utf8(bytes).map(Some).map_err(|_| Error::new("input must be UTF-8"))
}
fn protocol(host: &mut host::Host, input: &str) -> Value {
    let mut id = Value::Null;
    let result = (|| {
        let v = json::parse(input, 16384)?;
        fields(&v, &["version", "id", "op", "args"])?;
        if uint(get(&v, "version")?, 1)? != 1 {
            return Err(Error::new("expected version 1"));
        }
        let key = text(&v, "id")?;
        if key.is_empty() || key.len() > 64 || key.chars().any(char::is_control) {
            return Err(Error::new("invalid request ID"));
        }
        id = Value::str(key);
        host.request(&obj! {"op"=>get(&v,"op")?.clone(),"args"=>get(&v,"args")?.clone()})
    })();
    match result {
        Ok(value) => obj! {"version"=>1,"id"=>id,"ok"=>true,"result"=>value},
        Err(e) => obj! {"version"=>1,"id"=>id,"ok"=>false,"error"=>e.to_string()},
    }
}
fn show(game: &game::Game) {
    println!("\n{} | health {}/6 | coins {} | revision {}", game.room, game.hp, game.coins, game.revision);
    println!("{}", game.current_description());
    for item in game.visible_items() {
        println!(
            "  {} [{}; owner {}; price {}]",
            item.get("label").and_then(Value::as_str).unwrap(),
            item.get("id").and_then(Value::as_str).unwrap(),
            item.get("owner").and_then(Value::as_str).unwrap(),
            item.get("price").unwrap().to_json()
        );
    }
    let actions = game
        .choices()
        .iter()
        .map(|p| {
            let a = p.get("action").and_then(Value::as_str).unwrap();
            match p.get("target").and_then(Value::as_str) {
                Some(t) => format!("{a} {}", t.split(':').next_back().unwrap()),
                None => a.into(),
            }
        })
        .collect::<Vec<_>>();
    println!("Actions: {}", actions.join(" | "));
}
fn play(host: &mut host::Host) -> Result<()> {
    println!(
        "LANTERN VAULT\n{GOAL}\nUse the listed actions. Other commands: help, state, packet [raw|scene|named], quit."
    );
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    show(&host.game);
    loop {
        if host.game.won {
            println!("You escaped with the glass star!");
            return Ok(());
        }
        if host.game.choices().is_empty() {
            println!("This run has ended.");
            return Ok(());
        }
        print!("> ");
        io::stdout().flush().map_err(|e| Error::new(e.to_string()))?;
        let Some(input) = line(&mut reader)? else { return Ok(()) };
        let input = input.trim();
        if input == "quit" {
            return Ok(());
        }
        if input == "help" {
            println!("{GOAL}\nThe shop sells hinge oil. Find a bronze key, unlock the gate, defeat the sentinel, take the star, and go to the exit. Inspecting never unlocks a door.");
            continue;
        }
        if input == "state" {
            println!("{}", host.game.public().to_json());
            continue;
        }
        if input == "packet" || input.starts_with("packet ") {
            let mode = input.split_whitespace().nth(1).unwrap_or("scene");
            match host.observe(GOAL, mode, 2048) {
                Ok(v) => println!("{}", v.to_json()),
                Err(e) => println!("Packet failed: {e}"),
            };
            continue;
        }
        let words = input.split_whitespace().collect::<Vec<_>>();
        let choice = host.game.choices().into_iter().find(|p| {
            if words.first().copied() != p.get("action").and_then(Value::as_str) {
                return false;
            }
            match p.get("target").and_then(Value::as_str) {
                None => words.len() == 1,
                Some(t) => {
                    words.len() == 2 && (words[1] == t || words[1] == t.split(':').next_back().unwrap())
                        || words.len() == 1 && words[0] == "look"
                }
            }
        });
        if let Some(decision) = choice {
            let result = host.act(&obj! {"revision"=>host.game.revision,"decision"=>decision})?;
            println!("{}", result.get("message").and_then(Value::as_str).unwrap_or("Action rejected."));
            show(&host.game);
        } else {
            println!("Choose one of the listed actions, or type help.");
        }
    }
}
fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut mode = "play";
    let mut seed = 7;
    let mut replay_path = None;
    let mut model = "arzaan789/qwen2.5-3b-uncensored:latest".to_owned();
    let mut context = None::<u32>;
    let mut size = "3".to_owned();
    let mut count_cache = 0u8;
    let mut guidance = "focused".to_owned();
    let mut world_path = None::<String>;
    let mut routine_path = None::<String>;
    let mut routine_config = None::<String>;
    let mut dialogue_mode = "sources".to_owned();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" => {
                println!("Lantern Vault\n  --play [--seed N]       interactive game (default)\n  --serve [--seed N]      bounded version-1 JSON-lines host\n  --replay FILE|-         deterministic JSON replay\n  --ai [--model NAME]     human player with local AI characters\n  --size CLASS           editable task hints: 1, 1.5, 3 (default), 8, 30, 70\n  --guidance PROFILE     focused (default), standard, grounded\n  --count-cache N        opt-in exact-request token-count reuse, 0..64 entries\n  --context TOKENS       override size-class context starting budget\n  --world FILE           validated authored lore file\n  --dialogue MODE        sources (default), natural (experimental), or legacy\n  --social-replay FILE|-  conversation regression replay\n  --routines-demo         offline optional surface-annex routine/sensor demo\n  --routine-file FILE     editable demo routine\n  --routine-config FILE   editable demo defaults/overrides\nSeeds: 0..65535. Basic play needs no model or network.");
                return Ok(());
            }
            "--routines-demo" => mode = "routines-demo",
            "--routine-file" | "--routine-config" => {
                let option = args[i].clone();
                i += 1;
                let path = args.get(i).ok_or_else(|| Error::new("missing routine file"))?.clone();
                if option == "--routine-file" {
                    routine_path = Some(path);
                } else {
                    routine_config = Some(path);
                }
            }
            "--play" => mode = "play",
            "--ai" => mode = "ai",
            "--world" => {
                i += 1;
                world_path = Some(args.get(i).ok_or_else(|| Error::new("missing world file"))?.clone());
            }
            "--dialogue" => {
                i += 1;
                dialogue_mode = args.get(i).ok_or_else(|| Error::new("missing dialogue mode"))?.clone();
            }
            "--count-cache" => {
                i += 1;
                count_cache = args
                    .get(i)
                    .ok_or_else(|| Error::new("missing count cache capacity"))?
                    .parse()
                    .map_err(|_| Error::new("invalid count cache capacity"))?;
                if count_cache > 64 {
                    return Err(Error::new("count cache capacity must be 0..64"));
                }
            }
            "--model" => {
                i += 1;
                model = args.get(i).ok_or_else(|| Error::new("missing model"))?.clone();
            }
            "--size" => {
                i += 1;
                size = args.get(i).ok_or_else(|| Error::new("missing size"))?.clone();
            }
            "--guidance" => {
                i += 1;
                guidance = args.get(i).ok_or_else(|| Error::new("missing guidance"))?.clone();
            }
            "--context" => {
                i += 1;
                context = Some(
                    args.get(i)
                        .ok_or_else(|| Error::new("missing context"))?
                        .parse()
                        .map_err(|_| Error::new("invalid context"))?,
                );
            }
            "--serve" => mode = "serve",
            "--seed" => {
                i += 1;
                seed = args
                    .get(i)
                    .ok_or_else(|| Error::new("missing seed"))?
                    .parse()
                    .map_err(|_| Error::new("invalid seed"))?;
            }
            "--replay" | "--social-replay" => {
                i += 1;
                replay_path = Some(args.get(i).ok_or_else(|| Error::new("missing replay file"))?.clone());
                mode = if args[i - 1] == "--social-replay" { "social-replay" } else { "replay" };
            }
            _ => return Err(Error::new("unknown option; use --help")),
        }
        i += 1;
    }
    if mode == "routines-demo" {
        println!("{}", routines::demo(routine_path.as_deref(), routine_config.as_deref())?.to_json());
        return Ok(());
    }
    if routine_path.is_some() || routine_config.is_some() {
        return Err(Error::new("routine files require --routines-demo"));
    }
    if mode == "ai" {
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/dungeon/play_ai.py");
        let mut command = std::process::Command::new("python3");
        command
            .arg(script)
            .arg("--cap")
            .arg("0")
            .arg("--binary")
            .arg(std::env::current_exe().map_err(|e| Error::new(e.to_string()))?)
            .arg("--model")
            .arg(model)
            .arg("--seed")
            .arg(seed.to_string())
            .arg("--size")
            .arg(size)
            .arg("--count-cache")
            .arg(count_cache.to_string())
            .arg("--guidance")
            .arg(guidance);
        command.arg("--dialogue").arg(dialogue_mode);
        if let Some(path) = world_path {
            command.arg("--world").arg(path);
        }
        if let Some(context) = context {
            command.arg("--context").arg(context.to_string());
        }
        let status = command.status().map_err(|e| Error::new(e.to_string()))?;
        return if status.success() {
            Ok(())
        } else {
            Err(Error::new("AI player interface stopped; check Ollama/model settings above"))
        };
    }
    if mode == "replay" || mode == "social-replay" {
        let path = replay_path.ok_or_else(|| Error::new("missing replay file"))?;
        let reader: Box<dyn Read> = if path == "-" {
            Box::new(io::stdin())
        } else {
            Box::new(std::fs::File::open(path).map_err(|e| Error::new(e.to_string()))?)
        };
        let mut input = String::new();
        reader.take(262145).read_to_string(&mut input).map_err(|e| Error::new(e.to_string()))?;
        let v = json::parse(&input, 262144)?;
        println!("{}", if mode == "social-replay" { social::replay(&v)? } else { game::replay(&v)? }.to_json());
        return Ok(());
    }
    let mut host = if let Some(path) = world_path {
        host::Host::with_world(seed, gamectx::lore::Lore::read(path)?)?
    } else {
        host::Host::new(seed)?
    };
    if mode == "serve" {
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        while let Some(input) = line(&mut reader)? {
            writeln!(writer, "{}", protocol(&mut host, &input).to_json()).map_err(|e| Error::new(e.to_string()))?;
            writer.flush().map_err(|e| Error::new(e.to_string()))?;
        }
        Ok(())
    } else {
        play(&mut host)
    }
}
fn main() {
    if let Err(e) = run() {
        eprintln!("dungeon: {e}");
        std::process::exit(1);
    }
}
