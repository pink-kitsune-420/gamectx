//! Offline example: `cargo run --example runtime-loop`.
use gamectx::{
    brain::{Brain, Prediction},
    json::parse,
    obj,
    schedule::Scheduler,
    sync::{Publisher, Replica},
    Context, Value,
};

pub fn main() -> gamectx::Result<()> {
    let workflow = parse(include_str!("brain/selection.json"), 262144)?;
    let mut source = workflow.as_object().unwrap().to_vec();
    source.retain(|(key, _)| key != "operations");
    source.push(("operations".into(), Value::Array(vec![])));
    let initial = gamectx::brain::process(&Value::Object(source))?;
    let brain = Brain::restore(initial.get("state").unwrap())?;
    let request = workflow.get("operations").unwrap().as_array().unwrap()[0].get("args").unwrap();
    let mut brain = brain;
    let mut job = Some(brain.start_train_select(request)?);
    let mut prediction = Prediction::default();
    let mut scheduler = Scheduler::new(1, 1)?;
    scheduler.register(0, 1, 1, 10, 0)?;
    let mut context = Context::new().budget_bytes(1024);
    context.prompt("rules", "The host supplies current observations and validates each action.")?;
    let mut publisher = Publisher::new("encounter_1")?;
    let mut receiver = Replica::new("encounter_1")?;
    let mut evaluations = 0;
    for tick in 0..40 {
        // Advance candidate work separately. Training never changes active predictions mid-slice.
        if let Some(active) = &mut job {
            active.advance(4096)?;
        }
        if job.as_ref().is_some_and(|active| active.is_finished()) {
            let report = job.take().unwrap().finish(&mut brain)?;
            scheduler.changed(0, true)?;
            assert_eq!(report.get("accepted"), Some(&Value::Bool(true)));
        }
        if tick % 5 == 0 {
            scheduler.changed(0, false)?;
        }
        for _ in scheduler.poll(tick)? {
            let input = [Some(800_000), Some(200_000)];
            brain.run_into(&input, 3, 0, 0, &mut prediction)?;
            evaluations += 1;
        }
        let mut turn = context.turn();
        turn.var("tick", tick)?.var("reaction_index", prediction.choice())?;
        let packet = turn.build()?;
        let message = publisher.publish(&packet.value, Some(receiver.revision()))?;
        assert_eq!(receiver.apply(&message)?.to_json(), packet.text);
        // Give the complete reconstructed packet to a model; the sync envelope is for the host.
    }
    // Separate host-policy metric: eligibility and thresholds travel with validation labels.
    let mut fields = request.as_object().unwrap().to_vec();
    fields.retain(|(key, _)| key != "validation" && key != "expected_revision" && key != "id");
    let validation: Vec<_> = request
        .get("validation")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let mut row = row.as_object().unwrap().to_vec();
            row.extend([
                ("allowed".into(), Value::strings(brain.outputs())),
                ("min_score".into(), Value::from(0)),
                ("min_margin".into(), Value::from(0)),
            ]);
            Value::Object(row)
        })
        .collect();
    fields.extend([
        ("id".into(), Value::str("host_metric")),
        ("expected_revision".into(), Value::from(brain.revision())),
        ("validation".into(), Value::Array(validation)),
    ]);
    let report = brain.train_select_host(&Value::Object(fields))?;
    println!(
        "{}",
        (obj! {"frames"=>40,"evaluations"=>evaluations,"revision"=>brain.revision(),"host_report"=>report}).to_json()
    );
    Ok(())
}
