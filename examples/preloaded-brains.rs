//! Offline preload/template/cache example: `cargo run --example preloaded-brains`.
use gamectx::{
    brain::{Brain, CachedBrain},
    obj,
};

fn main() -> gamectx::Result<()> {
    // Load and validate once at startup; retain this compiled template outside the tick loop.
    let template = Brain::restore_json(include_str!("brain/preloaded.json"))?;
    let mut actor_a = CachedBrain::new(template.clone(), 32)?;
    let actor_b = CachedBrain::new(template, 32)?;
    let input = obj! {"danger" => 0.8,"need" => 0.5};
    let first = actor_a.run(&input, &["withdraw", "assist"])?;
    assert_eq!(actor_a.run(&input, &["assist", "withdraw"])?, first);
    assert_eq!(actor_a.stats().hits, 1);
    // Each owner retains its own learned state. Mutating access invalidates predictions.
    actor_a.brain_mut().undo(1)?;
    assert_eq!(actor_a.stats().entries, 0);
    let changed = actor_a.run(&input, &["withdraw", "assist"])?;
    assert_ne!(first, changed);
    assert_eq!(actor_b.brain().revision(), 1);
    assert_eq!(actor_a.brain().revision(), 2);
    println!(
        "{}",
        (obj! {"before_undo" => first,"after_undo" => changed,
        "cache_hits" => actor_a.stats().hits,"cache_misses" => actor_a.stats().misses})
        .to_json()
    );
    Ok(())
}
