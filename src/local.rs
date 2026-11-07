//! Stateless reference/CLI adapter for additive local deployment contracts. The byte
//! counter used here is a synthetic fixture estimator, never an actual model tokenizer.
use crate::{
    checked as c, obj,
    request::{Count, CountMethod, RequestBudget},
    Result, Value,
};
fn usize_arg(v: &Value, k: &str, max: u64) -> Result<usize> {
    Ok(c::uint(c::get(v, k)?, 0, max)? as usize)
}
pub fn process(v: &Value) -> Result<Value> {
    crate::request::bounded(v, 1_572_864)?;
    c::fields(v, &["kind", "args"])?;
    let a = c::get(v, "args")?;
    let counter = |request: &Value| Ok(Count { input: request.to_json().len(), method: CountMethod::Estimate });
    match c::text(c::get(v, "kind")?)? {
        "studio-roleplay-actions" => crate::studio::roleplay_actions_fixture(a),
        "studio-message-chunks" => crate::studio::message_chunks(a),
        "studio-wording" => crate::studio::validate_wording(a),
        "studio-expression" => crate::studio::expression_fixture(a),
        "studio-deliberation" => crate::studio::deliberation_fixture(a),
        "studio-greetings" => crate::studio::greetings_fixture(a),
        "studio-conversation" => crate::studio::conversation_route(a),
        "studio-reactions" => crate::studio::reactions_fixture(a),
        "studio-viewpoints" => crate::studio::viewpoints_fixture(a),
        "studio-voice-archive" => crate::studio::archive_fixture(a),
        "studio-speech-buffer" => crate::studio::speech_buffer(a),
        "studio-native-audio" => crate::studio::native_audio_plan(a),
        "model-manager" => crate::studio::manager_fixture(a),
        "life-speech-v2" => crate::life::speech_fixture(a),
        "sentence-boundary" => {
            c::fields(a, &["text", "max_chars"])?;
            Ok(crate::life::assess_sentence(
                c::text(c::get(a, "text")?)?,
                c::uint(c::get(a, "max_chars")?, 1, 1024)? as usize,
            )?
            .to_value())
        }
        "source-speech" | "required-speech" => {
            let allow_unknown = c::text(c::get(v, "kind")?)? == "source-speech";
            c::fields(a, &["answers", "calls"])?;
            let answers = crate::dialogue::Answers::from_value(c::get(a, "answers")?)?;
            let schema = answers.speech_schema_with_unknown(allow_unknown)?;
            let calls = c::list(c::get(a, "calls")?)?;
            if calls.len() > 32 {
                return Err(c::bad());
            }
            let mut results = Vec::new();
            for call in calls {
                c::fields(call, &["text", "context", "available"])?;
                let available = c::list(c::get(call, "available")?)?;
                if available.len() > 32 {
                    return Err(c::bad());
                }
                results.push(
                    match answers.resolve_speech_with_unknown(
                        c::text(c::get(call, "text")?)?,
                        c::get(call, "context")?,
                        allow_unknown,
                        |s| available.iter().any(|x| x.same(s)),
                    ) {
                        Ok(v) => obj! {"ok"=>true,"result"=>v},
                        Err(_) => obj! {"ok"=>false},
                    },
                );
            }
            Ok(obj! {"schema"=>schema,"results"=>results})
        }
        "character-context" => crate::life::context_fixture(a),
        "archetype" => crate::archetype::Blueprint::from_value(a).map(|b| b.to_value()),
        "studio-command" => crate::studio::resolve_command(a),
        "studio-routing" => crate::studio::routing_workflow(a),
        "studio-voice" => crate::studio::voice_workflow(a),
        "visual-spec" => crate::visual::compile_fixture(a).map(|s| s.to_value()),
        "visual-assets" => crate::visual::assets_workflow(a),
        "cast-pack" => crate::cast::CastPack::from_value(a).map(|p| p.summary()),
        "living-world" => crate::living::workflow(a),
        "speech-tickets" => crate::living::speech_workflow(a),
        "living-compile" => crate::living::compile_fixture(a),
        "attitude-gates" => crate::party::select_gates_fixture(a),
        "party" => crate::party::workflow(a),
        "party-compile" => crate::party::compile_fixture(a),
        "initiative" => crate::life::initiative_fixture(a),
        "episodes" => crate::life::episodes_fixture(a),
        "continuity" => crate::life::continuity_fixture(a),
        "life" => crate::life::workflow(a),
        "interactions" => crate::dialogue::interaction_workflow(a),
        "info" => crate::info::workflow(a),
        "inventory" => crate::inventory::workflow(a),
        "character-plan" => crate::character::plan_fixture(a),
        "world" => crate::world::process(a),
        "location" => crate::location::process(a),
        "quote-phrase" => {
            c::fields(a, &["quote"])?;
            Ok(Value::str(crate::dialogue::render_quote(c::text(c::get(a, "quote")?)?)?))
        }
        "lore" => {
            c::fields(a, &["world", "select"])?;
            crate::lore::Lore::from_value(c::get(a, "world")?)?.select(c::get(a, "select")?)
        }
        "answers" => {
            c::fields(a, &["definition", "calls"])?;
            let answers = crate::dialogue::Answers::from_value(c::get(a, "definition")?)?;
            let calls = c::list(c::get(a, "calls")?)?;
            if calls.len() > 8 {
                return Err(c::bad());
            }
            let mut results = Vec::new();
            for call in calls {
                c::fields(call, &["text", "context", "available"])?;
                let available = c::list(c::get(call, "available")?)?;
                if available.len() > 32 {
                    return Err(c::bad());
                }
                let result = answers.resolve(c::text(c::get(call, "text")?)?, c::get(call, "context")?, |s| {
                    available.iter().any(|v| v.same(s))
                });
                results.push(match result {
                    Ok(v) => obj! {"ok"=>true,"result"=>v},
                    Err(_) => obj! {"ok"=>false},
                });
            }
            Ok(obj! {"offer"=>answers.offer(),"schema"=>answers.schema(),"results"=>results})
        }
        "conversation" => crate::dialogue::conversation_workflow(a),
        "references" => crate::dialogue::resolve_references(a),
        "confirmation" => crate::decision::confirmation_workflow(a),
        "workflow" => {
            c::fields(a, &["size", "task", "name", "identity"])?;
            crate::deployment::ModelSize::parse(c::text(c::get(a, "size")?)?)?.workflow_profile(
                crate::deployment::Workload::parse(c::text(c::get(a, "task")?)?)?,
                c::text(c::get(a, "name")?)?,
                &crate::deployment::Identity::from_value(c::get(a, "identity")?)?,
            )
        }
        "render-direct" => crate::dialogue::render_direct(a),
        "tune" => crate::tune::compare(a),
        "choice-codec" => {
            c::fields(a, &["choices", "names", "calls"])?;
            let choices = crate::decision::Choices::from_value(c::get(a, "choices")?)?;
            let codec = crate::decision::ChoiceCodec::new(&choices, c::get(a, "names")?)?;
            let calls = c::list(c::get(a, "calls")?)?;
            if calls.len() > 3 {
                return Err(c::bad());
            }
            let mut session = codec.session();
            let mut results = Vec::new();
            for call in calls {
                c::fields(call, &["text", "revision", "available"])?;
                let available = c::list(c::get(call, "available")?)?;
                if available.len() > 32 {
                    return Err(c::bad());
                }
                results.push(session.apply(
                    c::text(c::get(call, "text")?)?,
                    c::uint(c::get(call, "revision")?, 0, u32::MAX as u64)? as u32,
                    |p| available.iter().any(|q| p.same(q)),
                )?);
            }
            Ok(
                obj! {"offer"=>codec.offer(),"schema"=>codec.schema(),"results"=>results,"finished"=>session.is_finished()},
            )
        }
        "route" => {
            c::fields(a, &["choices", "revision", "candidates", "available"])?;
            let choices = crate::decision::Choices::from_value(c::get(a, "choices")?)?;
            let available = c::list(c::get(a, "available")?)?;
            if available.len() > 32 {
                return Err(c::bad());
            }
            crate::decision::route(
                &choices,
                c::uint(c::get(a, "revision")?, 0, u32::MAX as u64)? as u32,
                c::get(a, "candidates")?,
                |p| available.iter().any(|q| p.same(q)),
            )
        }
        "compose" => {
            c::fields(a, &["definition", "observations"])?;
            let mut session = crate::request::Composition::from_value(c::get(a, "definition")?)?;
            let observations = c::list(c::get(a, "observations")?)?;
            if observations.len() > 129 {
                return Err(c::bad());
            }
            for observation in observations {
                session.observe(observation)?;
            }
            Ok(session.state())
        }
        "evidence" => {
            c::fields(a, &["records", "max_bytes"])?;
            crate::selection::Evidence::from_value(c::get(a, "records")?)?.select(usize_arg(a, "max_bytes", 262144)?)
        }
        "query-template" => {
            c::fields(a, &["template", "arguments", "rows"])?;
            let template = crate::query::Template::from_value(c::get(a, "template")?)?;
            let result = template.run(c::get(a, "arguments")?, c::list(c::get(a, "rows")?)?)?;
            Ok(obj! {"tool"=>template.tool(),"result"=>result})
        }
        "intent" => {
            c::fields(a, &["choices", "intent", "calls"])?;
            let choices = crate::decision::Choices::from_value(c::get(a, "choices")?)?;
            let mut session = choices.with_intent(c::get(a, "intent")?)?;
            let schema = session.schema();
            let calls = c::list(c::get(a, "calls")?)?;
            if calls.len() > 3 {
                return Err(c::bad());
            }
            let mut results = Vec::new();
            for call in calls {
                c::fields(call, &["text", "revision", "available"])?;
                let available = c::list(c::get(call, "available")?)?;
                if available.len() > 32 {
                    return Err(c::bad());
                }
                results.push(session.apply(
                    c::text(c::get(call, "text")?)?,
                    c::uint(c::get(call, "revision")?, 0, u32::MAX as u64)? as u32,
                    |p| available.iter().any(|q| p.same(q)),
                )?);
            }
            Ok(obj! {"schema"=>schema,"results"=>results,"finished"=>session.is_finished()})
        }
        "budget" => {
            c::fields(a, &["budget", "count"])?;
            Ok(RequestBudget::from_value(c::get(a, "budget")?)?
                .assess(Count::from_value(c::get(a, "count")?)?)?
                .to_value())
        }
        "dialogue" => {
            c::fields(a, &["thread", "current", "protect", "maximum", "tokens"])?;
            Ok(crate::dialogue::Thread::from_value(c::get(a, "thread")?)?
                .select(
                    c::text(c::get(a, "current")?)?,
                    usize_arg(a, "protect", 3)?,
                    usize_arg(a, "maximum", 128)?,
                    usize_arg(a, "tokens", 1_048_576)?,
                    counter,
                )?
                .to_value())
        }
        "scene" => {
            c::fields(a, &["scene", "protect", "maximum", "dialogue_tokens", "budget"])?;
            Ok(crate::dialogue::Scene::from_value(c::get(a, "scene")?)?
                .assemble(
                    &RequestBudget::from_value(c::get(a, "budget")?)?,
                    usize_arg(a, "protect", 3)?,
                    usize_arg(a, "maximum", 128)?,
                    usize_arg(a, "dialogue_tokens", 1_048_576)?,
                    crate::dialogue::render_chat,
                    counter,
                )?
                .to_value())
        }
        "cancel" => {
            c::fields(a, &["choices", "available"])?;
            let choices = crate::decision::Choices::from_value(c::get(a, "choices")?)?;
            let available = c::list(c::get(a, "available")?)?;
            if available.len() > 32 {
                return Err(c::bad());
            }
            choices.session().cancel(|v| available.iter().any(|p| p.same(v)))
        }
        "decision" => {
            c::fields(a, &["choices", "calls"])?;
            let choices = crate::decision::Choices::from_value(c::get(a, "choices")?)?;
            let calls = c::list(c::get(a, "calls")?)?;
            if calls.len() > 3 {
                return Err(c::bad());
            }
            let mut session = choices.session();
            let mut results = Vec::new();
            for call in calls {
                c::fields(call, &["text", "revision", "available"])?;
                let revision = c::uint(c::get(call, "revision")?, 0, u32::MAX as u64)? as u32;
                let available = c::list(c::get(call, "available")?)?;
                if available.len() > 32 {
                    return Err(c::bad());
                }
                results.push(
                    session
                        .apply(c::text(c::get(call, "text")?)?, revision, |v| available.iter().any(|p| p.same(v)))?,
                );
            }
            Ok(obj! {"schema"=>choices.schema(),"results"=>results,"finished"=>session.is_finished()})
        }
        "preset" => {
            c::fields(a, &["size", "name", "identity"])?;
            crate::deployment::ModelSize::parse(c::text(c::get(a, "size")?)?)?.starting_profile(
                c::text(c::get(a, "name")?)?,
                &crate::deployment::Identity::from_value(c::get(a, "identity")?)?,
            )
        }
        "profile" => {
            c::fields(a, &["profile", "actual_identity"])?;
            crate::deployment::Profile::from_value(c::get(a, "profile")?)?
                .describe(&crate::deployment::Identity::from_value(c::get(a, "actual_identity")?)?)
        }
        "time-assessment" => crate::observation::assessment_fixture(a),
        "routine-detail" => crate::routine::detail_fixture(a),
        "routine-fixture" => crate::routine::fixture(a),
        "report" => crate::turn::report(a),
        _ => Err(c::bad()),
    }
}
