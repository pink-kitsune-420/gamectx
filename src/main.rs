use gamectx::budget::{assemble, Packet};
use gamectx::catalog::{Catalog, Limits};
use gamectx::estimate::{self, Calibration};
use gamectx::manifest::{self, read_json_file, Check, Manifest};
use gamectx::{schema, Error, Result};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const HELP: &str = "gamectx: compile game data and tool schemas into small, budgeted context packets

USAGE:
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

const SWITCHES: [&str; 1] = ["--receipt"];

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
        "help" | "--help" | "-h" => {
            println!("{HELP}");
            Ok(())
        }
        other => Err(Error::new(format!("unknown command \"{other}\"; see gamectx help"))),
    }
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
