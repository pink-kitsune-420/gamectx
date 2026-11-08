//! Offline host example: `cargo run --example adaptive-brain`. No model or game engine.
use gamectx::{obj, runtime::brain::Brain, Value};

fn example(danger: f64, need: f64) -> Value {
    // These labels are supplied by the host's policy/designer, not inferred world facts.
    obj! {"inputs" => obj! {"danger" => danger, "need" => need},
    "targets" => obj! {"withdraw" => danger, "assist" => need * (1.0 - danger)}}
}

fn main() -> gamectx::Result<()> {
    let mut brain = Brain::dense(&["danger", "need"], &["withdraw", "assist"], 4, 3)?;
    let report = brain.train(&obj! {
        "id" => "first_training", "expected_revision" => brain.revision(),
        "examples" => vec![example(0.1,0.2),example(0.2,0.8),example(0.8,0.1),example(0.9,0.7)],
        "validation" => vec![example(0.15,0.75),example(0.75,0.15)],
        "epochs" => 128, "rate" => 0.1
    })?;
    assert_eq!(report.get("accepted"), Some(&Value::Bool(true)));
    // Supply current observations and the currently legal reactions from native game code.
    let prediction = brain.run(&obj! {"danger" => 0.8, "need" => 0.2}, &["withdraw", "assist"])?;
    assert_eq!(prediction.get("choice").and_then(Value::as_str), Some("withdraw"));
    let saved = brain.to_value().to_json();
    let restored = Brain::restore_json(&saved)?;
    assert_eq!(restored.to_value(), brain.to_value());
    // The host may persist `saved` in its own NPC/player/world storage. Action execution
    // still uses its normal validation. A null choice means use a host-defined fallback.
    println!("{}", (obj! {"training" => report, "prediction" => prediction, "saved_bytes" => saved.len()}).to_json());
    brain.undo(1)?;
    assert_eq!(brain.revision(), 2);
    Ok(())
}
