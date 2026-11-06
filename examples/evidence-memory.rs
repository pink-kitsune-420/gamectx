//! Generic native evidence and model-proposed hypotheses: `cargo run --example evidence-memory`.
use gamectx::{memory::EvidenceMemory, obj, Value};
pub fn main() -> gamectx::Result<()> {
    let mut memory = EvidenceMemory::new("actor_a", "game_ticks", 8, 4)?;
    let observation = |turn, key, text| obj! {"turn"=>turn,"entity"=>"actor_a","key"=>key,"text"=>text,"tags"=>vec!["activity"],"importance"=>5,"source"=>"observed","expires"=>30};
    memory.observe(1, &observation(1, "approach", "The actor approached cover."), &["position"])?;
    memory.observe(2, &observation(2, "departure", "The actor then moved away."), &[])?;
    let view = memory.snapshot(2, 8, 4000, &[])?;
    // Supply view.to_value() to a model. Keep the sealed host object for validating its reply.
    let request = obj! {"id"=>"seeking_shelter","statement"=>"The actor may be seeking shelter.","confidence"=>0.6,"supports"=>vec![1],"contradicts"=>vec![2],"ttl"=>100};
    memory.propose(&view, 2, &request, &[])?;
    let public = memory.snapshot(3, 8, 4000, &[])?.to_value();
    assert_eq!(public.get("hypotheses").unwrap().as_array().unwrap()[0].get("status"), Some(&Value::str("contested")));
    assert_eq!(public.get("hypotheses").unwrap().as_array().unwrap()[0].get("expires"), Some(&Value::from(30)));
    let saved = memory.to_value();
    let mut restored = EvidenceMemory::restore_json(&saved.to_json(), "actor_a", "game_ticks")?;
    assert_eq!(restored.to_value(), saved);
    // Consent changes filter snapshots immediately; physical forgetting cascades dependencies.
    let denied = restored.snapshot(3, 8, 4000, &["position"])?.to_value();
    assert!(denied.get("hypotheses").unwrap().as_array().unwrap().is_empty());
    restored.forget_privacy(&["position"])?;
    assert!(restored.to_value().get("hypotheses").unwrap().as_array().unwrap().is_empty());
    println!("{}", public.to_json());
    Ok(())
}
