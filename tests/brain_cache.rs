use gamectx::{
    data::json::parse,
    obj,
    runtime::brain::{Brain, CachedBrain},
    Value,
};

fn brain() -> Brain {
    Brain::dense(&["x", "y"], &["left", "right"], 4, 3).unwrap()
}

#[test]
fn normalized_keys_include_masks_thresholds_and_unknowns() {
    let base = brain();
    let mut cache = CachedBrain::new(base.clone(), 16).unwrap();
    let first = parse(r#"{"x":0.500000,"y":0.25}"#, 1024).unwrap();
    let alias = parse(r#"{"y":0.25,"x":5e-1}"#, 1024).unwrap();
    assert_eq!(cache.run(&first, &["left", "right"]).unwrap(), base.run(&first, &["left", "right"]).unwrap());
    assert_eq!(cache.run(&alias, &["right", "left"]).unwrap(), base.run(&first, &["left", "right"]).unwrap());
    assert_eq!(cache.stats().hits, 1);
    for (input, allowed, low, gap) in [
        (obj! {"x" => 0,"y" => 0.25}, vec!["left", "right"], 0.0, 0.0),
        (obj! {"x" => Value::Null,"y" => 0.25}, vec!["left", "right"], 0.0, 0.0),
        (alias.clone(), vec!["left"], 0.0, 0.0),
        (alias.clone(), vec!["left", "right"], 0.9, 0.0),
        (alias.clone(), vec!["left", "right"], 0.0, 0.9),
        (alias.clone(), vec![], 0.0, 0.0),
    ] {
        assert_eq!(
            cache.run_with(&input, &allowed, low, gap).unwrap(),
            base.run_with(&input, &allowed, low, gap).unwrap()
        );
    }
    let before = cache.stats();
    assert!(cache.run(&obj! {"x" => 1.000001,"y" => 0.25}, &["left", "right"]).is_err());
    assert!(cache.run(&alias, &["left", "left"]).is_err());
    assert!(cache.run_with(&alias, &["left"], f64::NAN, 0.0).is_err());
    assert_eq!(cache.stats(), before);
}

#[test]
fn fifo_is_bounded_disabled_mode_has_no_entries_and_results_are_owned() {
    let mut cache = CachedBrain::new(brain(), 2).unwrap();
    let input = |x| obj! {"x" => x,"y" => 0.5};
    let expected = cache.brain().run(&input(0.1), &["left", "right"]).unwrap();
    let mut result = cache.run(&input(0.1), &["left", "right"]).unwrap();
    if let Value::Object(entries) = &mut result {
        entries.clear();
    }
    assert_eq!(cache.run(&input(0.1), &["left", "right"]).unwrap(), expected);
    cache.run(&input(0.2), &["left", "right"]).unwrap();
    cache.run(&input(0.3), &["left", "right"]).unwrap();
    assert_eq!(cache.stats().entries, 2);
    assert_eq!(cache.stats().misses, 3);
    cache.run(&input(0.1), &["left", "right"]).unwrap();
    assert_eq!(cache.stats().misses, 4, "FIFO hits do not refresh eviction order");
    let mut disabled = CachedBrain::new(brain(), 0).unwrap();
    disabled.run(&input(0.1), &["left"]).unwrap();
    disabled.run(&input(0.1), &["left"]).unwrap();
    assert_eq!(disabled.stats().entries, 0);
    assert_eq!(disabled.stats().misses, 2);
    assert!(CachedBrain::new(brain(), 257).is_err());
}

#[test]
fn mutation_undo_and_same_revision_replacement_invalidate_predictions() {
    let workflow = parse(include_str!("../conformance/brain/teach-accept-undo/input.json"), 8192).unwrap();
    let base = Brain::from_value(workflow.get("definition").unwrap()).unwrap();
    let mut cache = CachedBrain::new(base.clone(), 4).unwrap();
    let input = obj! {"danger" => 0.8,"need" => 0.5};
    let before = cache.run(&input, &["withdraw", "assist"]).unwrap();
    let args = workflow.get("operations").unwrap().as_array().unwrap()[0].get("args").unwrap();
    cache.brain_mut().teach(args).unwrap();
    assert_eq!(cache.stats().entries, 0);
    let after = cache.run(&input, &["withdraw", "assist"]).unwrap();
    assert_ne!(before, after);
    cache.brain_mut().undo(1).unwrap();
    assert_eq!(
        cache.run(&input, &["withdraw", "assist"]).unwrap(),
        cache.brain().run(&input, &["withdraw", "assist"]).unwrap()
    );
    *cache.brain_mut() = base;
    assert_eq!(cache.run(&input, &["withdraw", "assist"]).unwrap(), before);
    let other = Brain::dense(&["danger", "need"], &["withdraw", "assist"], 0, 99).unwrap();
    let expected = other.run(&input, &["withdraw", "assist"]).unwrap();
    *cache.brain_mut() = other;
    assert_eq!(cache.run(&input, &["withdraw", "assist"]).unwrap(), expected);
    let saved = cache.brain().to_value();
    assert_eq!(Brain::restore(&saved).unwrap().to_value(), saved);
    assert_eq!(cache.into_brain().to_value(), saved, "ephemeral cache never changes saved learning");
}
