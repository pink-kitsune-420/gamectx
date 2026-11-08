use gamectx::data::catalog::{Catalog, Limits};
use gamectx::packet::budget::{assemble, Packet};
use gamectx::packet::estimate::{self, Calibration};
use gamectx::packet::manifest::{self, read_json_file, Check, Manifest};
use gamectx::{packet::schema, Error, Result};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const HELP: &str = "gamectx: compile game data and tool schemas into small, budgeted context packets

USAGE:
  gamectx init <new-directory>
      Generate a project, snapshot, evaluation suite and runnable starter checks.
  gamectx doctor <project.json>
      Check project registration and print configuration diagnostics as JSON.
  gamectx inspect <project.json> <snapshot.json> <recipe>
      Build a packet and print its separate host-only selection/budget inspection.
  gamectx characters
      Start a persistent character-tool bridge without project-registration boilerplate.
  gamectx serve <project.json>
      Persistent version-1 JSON-lines bridge; bounded requests on stdin, replies on stdout.
  gamectx evaluate <project.json> <suite.json>
      Score complete recorded model runs against independent snapshot expectations; offline.
  gamectx local <workflow.json>
      Run an additive request/dialogue/decision/profile/turn-report workflow; - reads stdin.
  gamectx kit
      Read a stateless {project,op,args} request on stdin (reference/conformance adapter).
  gamectx pack <manifest.json> [BUDGETS] [--receipt]
      Print the packet as compact JSON. --receipt prints what was kept/dropped to stderr.
      Use - as the manifest to read it from standard input.
  gamectx report <manifest.json> [BUDGETS]
      Show each section's size before and after packing, and what the budget kept.
  gamectx verify <manifest.json>
      Unpack every lossless section and check it matches the original exactly
      (for tables with a view: the rows the view selected).
  gamectx search <catalog.json> <query> [--kind KIND] [--offset N] [--count N] [--generation G]
      Print one page of catalog search results.
  gamectx outline <tools.json>
      Print lossy one-line tool signatures.
  gamectx calc <expression> [--formulas formulas.json]
      Evaluate a formula expression exactly, e.g. a model's calc request; prints JSON.
      --formulas loads saved definitions (a list of name, params, body, description).
  gamectx query --guidance <standard|explicit-selection>
      Print optional supplemental system guidance as a JSON string.
  gamectx query --explain <sql>
      Print the parsed query plan without reading or executing a table.
  gamectx query <tables.json> <sql>
      Run a model's SQL SELECT exactly over the tables (an object of name: array of
      objects); - reads tables from stdin. Prints JSON. gamectx query --tool prints the query tool for the model.
  gamectx query --schema <tables.json> [--metadata meanings.json]
      Print column types and complete small string domains, without rows. - reads stdin.
  gamectx query --session <tables.json> [--result-bytes N]
      Read JSON argument objects (one per line) from stdin; print one JSON reply per attempt. A query error
      allows one correction. Success or the second failure ends the session.
  gamectx replay <trace.json>
      Replay bounded normalized observations and brain snapshots; - reads stdin.
  gamectx brain <workflow.json> [--cache N]
      Run a bounded local reaction-network workflow; prints results and saved state.
      - reads stdin. --cache uses a bounded per-workflow prediction cache (0..256 entries).
      gamectx brain --tools prints model tool schemas.
  gamectx calibrate <calibration.json> --scope NAME --bytes N --tokens N [--estimate N]
      Record one provider usage report (input tokens for a request body of N bytes).

BUDGETS (override the manifest's budget_bytes / budget_tokens):
  --budget BYTES              bytes of compact JSON
  --budget-tokens N           input tokens, estimated conservatively
  --calibration FILE --scope NAME
                              use a calibration file for the token estimate; without
                              one, every byte counts as a token";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gamectx: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Positional arguments plus `--flag value` / `--switch` options.
struct Args {
    positional: Vec<String>,
    options: Vec<(String, Option<String>)>,
}

const SWITCHES: [&str; 6] = ["--explain", "--receipt", "--tool", "--schema", "--session", "--tools"];

impl Args {
    fn parse(raw: &[String]) -> Result<Args> {
        let mut positional = Vec::new();
        let mut options = Vec::new();
        let mut it = raw.iter();
        while let Some(arg) = it.next() {
            if arg.starts_with("--") {
                if SWITCHES.contains(&arg.as_str()) {
                    options.push((arg.clone(), None));
                } else {
                    let value = it.next().ok_or_else(|| Error::new(format!("{arg} needs a value")))?;
                    options.push((arg.clone(), Some(value.clone())));
                }
            } else {
                positional.push(arg.clone());
            }
        }
        Ok(Args { positional, options })
    }

    fn only(&self, allowed: &[&str], positional: usize) -> Result<()> {
        if let Some((name, _)) = self.options.iter().find(|(n, _)| !allowed.contains(&n.as_str())) {
            return Err(Error::new(format!("unknown option {name}")));
        }
        if self.positional.len() != positional {
            return Err(Error::new(format!("expected {positional} argument(s); see gamectx help")));
        }
        Ok(())
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.options.iter().rev().find(|(n, _)| n == name).and_then(|(_, v)| v.as_deref())
    }

    fn has(&self, name: &str) -> bool {
        self.options.iter().any(|(n, _)| n == name)
    }

    fn number(&self, name: &str) -> Result<Option<usize>> {
        self.get(name)
            .map(|v| v.parse().map_err(|_| Error::new(format!("{name} must be a non-negative integer"))))
            .transpose()
    }
}

fn run(raw: &[String]) -> Result<()> {
    let Some(command) = raw.first() else {
        println!("{HELP}");
        return Ok(());
    };
    let args = Args::parse(&raw[1..])?;
    match command.as_str() {
        "local" => {
            args.only(&[], 1)?;
            let mut input = String::new();
            let cap = gamectx::packet::request::MAX_REQUEST_BYTES;
            if args.positional[0] == "-" {
                std::io::stdin()
                    .take(cap as u64 + 1)
                    .read_to_string(&mut input)
                    .map_err(|e| Error::new(e.to_string()))?;
            } else {
                std::fs::File::open(&args.positional[0])
                    .map_err(|e| Error::new(e.to_string()))?
                    .take(cap as u64 + 1)
                    .read_to_string(&mut input)
                    .map_err(|e| Error::new(e.to_string()))?;
            }
            println!("{}", gamectx::kit::local::process(&gamectx::data::json::parse(&input, cap)?)?.to_json());
            Ok(())
        }
        "kit" => {
            args.only(&[], 0)?;
            let mut input = String::new();
            std::io::stdin()
                .take(gamectx::kit::MAX_REQUEST_BYTES as u64 + 1)
                .read_to_string(&mut input)
                .map_err(|e| Error::new(e.to_string()))?;
            println!(
                "{}",
                gamectx::kit::process(&gamectx::data::json::parse(&input, gamectx::kit::MAX_REQUEST_BYTES)?)?.to_json()
            );
            Ok(())
        }
        "doctor" => {
            args.only(&[], 1)?;
            let project = load_project(&args.positional[0])?;
            println!("{}", project.doctor().to_json());
            Ok(())
        }
        "inspect" => {
            args.only(&[], 3)?;
            let project = load_project(&args.positional[0])?;
            let snapshot = project.snapshot(&read_json_file(Path::new(&args.positional[1]))?)?;
            println!("{}", project.build(&args.positional[2], &snapshot)?.to_value().to_json());
            Ok(())
        }
        "characters" => {
            args.only(&[], 0)?;
            let project = gamectx::kit::Registration::new().build()?;
            gamectx::kit::serve(&project, &mut std::io::stdin().lock(), &mut std::io::stdout().lock())
        }
        "serve" => {
            args.only(&[], 1)?;
            let project = load_project(&args.positional[0])?;
            gamectx::kit::serve(&project, &mut std::io::stdin().lock(), &mut std::io::stdout().lock())
        }
        "init" => {
            args.only(&[], 1)?;
            gamectx::kit::init(Path::new(&args.positional[0]))
        }
        "evaluate" => {
            args.only(&[], 2)?;
            let project = load_project(&args.positional[0])?;
            let suite = read_json_file(Path::new(&args.positional[1]))?;
            println!("{}", gamectx::kit::evaluation::run(&project, &suite)?.to_json());
            Ok(())
        }
        "pack" => {
            args.only(&["--budget", "--budget-tokens", "--calibration", "--scope", "--receipt"], 1)?;
            let manifest = manifest::load(Path::new(&args.positional[0]))?;
            let packet = build(&manifest, &Budgets::from(&args, &manifest)?)?;
            println!("{}", packet.text);
            if args.has("--receipt") {
                eprintln!("{}", packet.receipt.to_value().to_json());
            }
            Ok(())
        }
        "report" => {
            args.only(&["--budget", "--budget-tokens", "--calibration", "--scope"], 1)?;
            let manifest = manifest::load(Path::new(&args.positional[0]))?;
            report(&manifest, &Budgets::from(&args, &manifest)?)
        }
        "verify" => {
            args.only(&[], 1)?;
            let manifest = manifest::load(Path::new(&args.positional[0]))?;
            for s in &manifest.sections {
                let status = match s.verify()? {
                    Check::Exact => "ok (round-trips exactly)",
                    Check::ExactAfterView => "ok (round-trips the rows its view selected)",
                    Check::Lossy => "lossy by design, not checked",
                };
                println!("{:<20} {:<10} {status}", s.section.name, s.pack.name());
            }
            Ok(())
        }
        "search" => {
            args.only(&["--kind", "--offset", "--count", "--generation"], 2)?;
            let catalog = Catalog::from_json(&read_json_file(Path::new(&args.positional[0]))?, Limits::default())?;
            let page = catalog.search(
                &args.positional[1],
                args.get("--kind"),
                args.number("--offset")?.unwrap_or(0),
                args.number("--count")?.unwrap_or(Limits::default().results),
                args.get("--generation"),
            )?;
            println!("{}", page.to_value().to_json());
            Ok(())
        }
        "outline" => {
            args.only(&[], 1)?;
            let tools = read_json_file(Path::new(&args.positional[0]))?;
            println!(
                "{}",
                schema::outline(tools.as_array().ok_or_else(|| Error::new("tools file must be an array"))?)?
            );
            Ok(())
        }
        "calibrate" => {
            args.only(&["--scope", "--bytes", "--tokens", "--estimate"], 1)?;
            let path = PathBuf::from(&args.positional[0]);
            let current = if path.exists() { load_calibration(&path)? } else { Calibration::NONE };
            let need = |name: &str| args.number(name)?.ok_or_else(|| Error::new(format!("{name} is required")));
            let scope = estimate::scope(&[args.get("--scope").ok_or_else(|| Error::new("--scope is required"))?]);
            let bytes = need("--bytes")?;
            let used = args.number("--estimate")?.unwrap_or(bytes);
            let next = current.observe(scope, bytes, need("--tokens")? as u64, used);
            std::fs::write(&path, next.to_value().to_json() + "\n")
                .map_err(|e| Error::new(format!("{}: {e}", path.display())))?;
            println!("{}", next.to_value().to_json());
            Ok(())
        }
        "calc" => {
            args.only(&["--formulas"], 1)?;
            let formulas = match args.get("--formulas") {
                Some(file) => gamectx::data::formula::Formulas::from_value(
                    &read_json_file(Path::new(file))?,
                    gamectx::data::formula::Limits::default(),
                )?,
                None => gamectx::data::formula::Formulas::new(),
            };
            println!("{}", formulas.calc(&args.positional[0])?.to_json());
            Ok(())
        }
        "replay" => {
            args.only(&[], 1)?;
            let mut text = String::new();
            if args.positional[0] == "-" {
                std::io::stdin()
                    .take(gamectx::runtime::replay::MAX_TRACE_BYTES as u64 + 1)
                    .read_to_string(&mut text)
                    .map_err(|e| Error::new(e.to_string()))?;
            } else {
                std::fs::File::open(&args.positional[0])
                    .map_err(|e| Error::new(e.to_string()))?
                    .take(gamectx::runtime::replay::MAX_TRACE_BYTES as u64 + 1)
                    .read_to_string(&mut text)
                    .map_err(|e| Error::new(e.to_string()))?;
            }
            println!("{}", gamectx::runtime::replay::run_json(&text)?.to_json());
            Ok(())
        }
        "brain" if args.has("--tools") => {
            args.only(&["--tools"], 0)?;
            println!("{}", gamectx::runtime::brain::tools().to_json());
            Ok(())
        }
        "brain" => {
            args.only(&["--cache"], 1)?;
            let mut text = String::new();
            let cap = gamectx::runtime::brain::MAX_STATE_BYTES as u64 + 1;
            if args.positional[0] == "-" {
                std::io::stdin().take(cap).read_to_string(&mut text).map_err(|e| Error::new(e.to_string()))?;
            } else {
                std::fs::File::open(&args.positional[0])
                    .map_err(|e| Error::new(e.to_string()))?
                    .take(cap)
                    .read_to_string(&mut text)
                    .map_err(|e| Error::new(e.to_string()))?;
            }
            let input = gamectx::data::json::parse(&text, gamectx::runtime::brain::MAX_STATE_BYTES)?;
            let output = match args.number("--cache")? {
                Some(capacity) => gamectx::runtime::brain::process_cached(&input, capacity)?,
                None => gamectx::runtime::brain::process(&input)?,
            };
            println!("{}", output.to_json());
            Ok(())
        }
        "query" if args.has("--guidance") => {
            args.only(&["--guidance"], 0)?;
            let profile = gamectx::data::query::Guidance::parse(args.get("--guidance").unwrap_or(""))?;
            println!("{}", gamectx::Value::str(profile.text()).to_json());
            Ok(())
        }
        "query" if args.has("--explain") => {
            args.only(&["--explain"], 1)?;
            println!("{}", gamectx::data::query::Query::parse(&args.positional[0])?.plan().to_json());
            Ok(())
        }
        "query" if args.has("--tool") => {
            args.only(&["--tool"], 0)?;
            println!("{}", gamectx::data::query::tool().to_json());
            Ok(())
        }
        "query" if args.has("--schema") => {
            args.only(&["--schema", "--metadata"], 1)?;
            let tables = if args.positional[0] == "-" {
                let mut text = String::new();
                std::io::stdin()
                    .take(manifest::MAX_FILE_BYTES as u64 + 1)
                    .read_to_string(&mut text)
                    .map_err(|e| Error::new(e.to_string()))?;
                gamectx::data::json::parse(&text, manifest::MAX_FILE_BYTES)?
            } else {
                read_json_file(Path::new(&args.positional[0]))?
            };
            let named = named_tables(&tables)?;
            let description = match args.get("--metadata") {
                Some(file) => gamectx::data::query::describe_with(&named, &read_json_file(Path::new(file))?)?,
                None => gamectx::data::query::describe(&named)?,
            };
            println!("{}", description.to_json());
            Ok(())
        }
        "query" if args.has("--session") => {
            args.only(&["--session", "--result-bytes"], 1)?;
            if args.positional[0] == "-" {
                return Err(Error::new("query sessions need a table file; stdin is used for query calls"));
            }
            let tables = read_json_file(Path::new(&args.positional[0]))?;
            let tables = named_tables(&tables)?;
            let mut session =
                gamectx::data::query::Session::new().result_bytes(args.number("--result-bytes")?.unwrap_or(4096));
            let mut input = std::io::stdin().lock();
            let mut output = std::io::stdout().lock();
            while !session.is_finished() {
                let mut line = String::new();
                let bytes = (&mut input).take(8193).read_line(&mut line).map_err(|e| Error::new(e.to_string()))?;
                if bytes == 0 {
                    return Err(Error::new("query session ended before a terminal reply"));
                }
                if bytes > 8192 {
                    return Err(Error::new("query input line exceeds 8192 bytes"));
                }
                let call = gamectx::data::json::parse(&line, 8192).unwrap_or(gamectx::Value::Null);
                let reply = session.apply_call(&call, &tables)?;
                writeln!(output, "{}", reply.to_json()).map_err(|e| Error::new(e.to_string()))?;
                output.flush().map_err(|e| Error::new(e.to_string()))?;
            }
            Ok(())
        }
        "query" => {
            args.only(&[], 2)?;
            let tables = if args.positional[0] == "-" {
                let mut input = String::new();
                std::io::stdin()
                    .take((1 << 20) + 1)
                    .read_to_string(&mut input)
                    .map_err(|e| Error::new(e.to_string()))?;
                gamectx::data::json::parse(&input, 1 << 20)?
            } else {
                read_json_file(Path::new(&args.positional[0]))?
            };
            println!("{}", gamectx::data::query::run(&args.positional[1], &named_tables(&tables)?)?.to_json());
            Ok(())
        }
        "help" | "--help" | "-h" => {
            println!("{HELP}");
            Ok(())
        }
        other => Err(Error::new(format!("unknown command \"{other}\"; see gamectx help"))),
    }
}

fn named_tables(value: &gamectx::Value) -> Result<Vec<(&str, &[gamectx::Value])>> {
    value
        .as_object()
        .ok_or_else(|| Error::new("tables must be an object of name: array"))?
        .iter()
        .map(|(name, rows)| {
            rows.as_array()
                .map(|rows| (name.as_str(), rows))
                .ok_or_else(|| Error::new(format!("table \"{name}\" must be an array")))
        })
        .collect()
}

fn load_calibration(path: &Path) -> Result<Calibration> {
    Calibration::from_value(&read_json_file(path)?)
}

struct Budgets {
    bytes: Option<usize>,
    tokens: Option<usize>,
    calibration: Calibration,
    scope: u64,
}

impl Budgets {
    fn from(args: &Args, manifest: &Manifest) -> Result<Budgets> {
        let (calibration, scope) = match (args.get("--calibration"), args.get("--scope")) {
            (Some(file), Some(scope)) => (load_calibration(Path::new(file))?, estimate::scope(&[scope])),
            (None, None) => (Calibration::NONE, 0),
            _ => return Err(Error::new("--calibration and --scope go together")),
        };
        Ok(Budgets {
            bytes: args.number("--budget")?.or(manifest.budget_bytes),
            tokens: args.number("--budget-tokens")?.or(manifest.budget_tokens),
            calibration,
            scope,
        })
    }

    fn tokens(&self, text: &str) -> usize {
        self.calibration.estimate(self.scope, text.as_bytes(), 0)
    }

    fn calibrated(&self) -> bool {
        self.calibration.samples > 0 && self.calibration.scope == self.scope
    }
}

fn build(manifest: &Manifest, budgets: &Budgets) -> Result<Packet> {
    let sections = manifest.sections.iter().map(|p| p.section.clone()).collect();
    assemble(sections, &|text: &str| {
        budgets.bytes.is_none_or(|b| text.len() <= b) && budgets.tokens.is_none_or(|t| budgets.tokens(text) <= t)
    })
}

fn report(manifest: &Manifest, budgets: &Budgets) -> Result<()> {
    println!(
        "{:<20} {:<10} {:>8} {:>6} {:>10} {:>10} {:>7}",
        "section", "pack", "priority", "stable", "original", "packed", "saved"
    );
    let (mut original_total, mut packed_total) = (0, 0);
    for s in &manifest.sections {
        let original = s.original.json_len();
        let packed = s.section.value.json_len();
        original_total += original;
        packed_total += packed;
        println!(
            "{:<20} {:<10} {:>8} {:>6} {:>10} {:>10} {:>7}",
            s.section.name,
            s.pack.name(),
            s.section.priority,
            if s.section.stable { "yes" } else { "no" },
            original,
            packed,
            saved(original, packed)
        );
    }
    println!(
        "{:<47} {:>10} {:>10} {:>7}",
        "all sections (bytes of compact JSON)",
        original_total,
        packed_total,
        saved(original_total, packed_total)
    );

    let packet = build(manifest, budgets)?;
    let limits: Vec<String> =
        [budgets.bytes.map(|b| format!("{b} bytes")), budgets.tokens.map(|t| format!("{t} tokens"))]
            .into_iter()
            .flatten()
            .collect();
    let limits =
        if limits.is_empty() { "no budget set".to_owned() } else { format!("budget {}", limits.join(" and ")) };
    println!("\npacket: {} bytes; {limits}", packet.receipt.total_bytes);
    for line in &packet.receipt.dropped {
        println!("  dropped {} ({} bytes, priority {})", line.name, line.bytes, line.priority);
    }
    if budgets.calibrated() {
        println!(
            "token estimate: {} (calibrated from {} usage report(s), 2x margin)",
            budgets.tokens(&packet.text),
            budgets.calibration.samples
        );
    } else {
        println!(
            "token bound: {} (uncalibrated byte bound; real usage is usually much lower)",
            packet.receipt.total_bytes
        );
    }
    Ok(())
}

fn saved(original: usize, packed: usize) -> String {
    if original == 0 {
        return "-".into();
    }
    format!("{:.1}%", 100.0 * (original as f64 - packed as f64) / original as f64)
}

fn load_project(path: &str) -> Result<gamectx::kit::Project> {
    let mut input = String::new();
    std::fs::File::open(path)
        .map_err(|e| Error::new(e.to_string()))?
        .take(gamectx::kit::MAX_PROJECT_BYTES as u64 + 1)
        .read_to_string(&mut input)
        .map_err(|e| Error::new(e.to_string()))?;
    gamectx::kit::Project::parse(&input)
}
