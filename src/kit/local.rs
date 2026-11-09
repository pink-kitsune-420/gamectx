//! Stateless reference/CLI adapter for additive local deployment contracts. The byte
//! counter used here is a synthetic fixture estimator, never an actual model tokenizer.
use crate::{
    checked as c, obj,
    packet::request::{Count, CountMethod, RequestBudget},
    Result, Value,
};
fn usize_arg(v: &Value, k: &str, max: u64) -> Result<usize> {
    Ok(c::uint(c::get(v, k)?, 0, max)? as usize)
}
pub fn process(v: &Value) -> Result<Value> {
    crate::packet::request::bounded(v, 1_572_864)?;
    c::fields(v, &["kind", "args"])?;
    let a = c::get(v, "args")?;
    let counter = |request: &Value| Ok(Count { input: request.to_json().len(), method: CountMethod::Estimate });
    match c::text(c::get(v, "kind")?)? {
        "studio-roleplay-actions" => crate::character::roleplay::roleplay_actions_fixture(a),
        "studio-message-chunks" => crate::character::roleplay::message_chunks(a),
        "studio-wording" => crate::character::roleplay::validate_wording(a),
        "studio-expression" => crate::character::roleplay::expression_fixture(a),
        "studio-deliberation" => crate::character::roleplay::deliberation_fixture(a),
        "studio-greetings" => crate::character::roleplay::greetings_fixture(a),
        "studio-conversation" => crate::character::roleplay::conversation_route(a),
        "studio-reactions" => crate::character::roleplay::reactions_fixture(a),
        "studio-viewpoints" => crate::character::roleplay::viewpoints_fixture(a),
        "studio-voice-archive" => crate::media::voice::archive_fixture(a),
        "studio-speech-buffer" => crate::media::voice::speech_buffer(a),
        "studio-native-audio" => crate::media::voice::native_audio_plan(a),
        "model-manager" => crate::deploy::model_policy::fixture(a),
        "life-speech-v2" => crate::character::life::speech_fixture(a),
        "sentence-boundary" => {
            c::fields(a, &["text", "max_chars"])?;
            Ok(crate::character::life::assess_sentence(
                c::text(c::get(a, "text")?)?,
                c::uint(c::get(a, "max_chars")?, 1, 1024)? as usize,
            )?
            .to_value())
        }
        "source-speech" | "required-speech" => {
            let allow_unknown = c::text(c::get(v, "kind")?)? == "source-speech";
            c::fields(a, &["answers", "calls"])?;
            let answers = crate::character::dialogue::Answers::from_value(c::get(a, "answers")?)?;
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
        "character-context" => crate::character::life::context_fixture(a),
        "archetype" => crate::character::archetype::Blueprint::from_value(a).map(|b| b.to_value()),
        "studio-command" => crate::character::roleplay::resolve_command(a),
        "roleplay-engine" => crate::character::roleplay::engine_fixture(a),
        "save-file" => crate::kit::save::workflow(a),
        "studio-routing" => crate::media::providers::routing_workflow(a),
        "studio-voice" => crate::media::voice::voice_workflow(a),
        "visual-spec" => crate::media::visual::compile_fixture(a).map(|s| s.to_value()),
        "visual-describe" => crate::media::visual::describe_fixture(a),
        "visual-assets" => crate::media::visual::assets_workflow(a),
        "visual-director" => crate::media::visual::director_workflow(a),
        "cast-pack" => crate::character::cast::CastPack::from_value(a).map(|p| p.summary()),
        "living-world" => crate::character::living::workflow(a),
        "speech-tickets" => crate::character::living::speech_workflow(a),
        "living-compile" => crate::character::living::compile_fixture(a),
        "attitude-gates" => crate::character::party::select_gates_fixture(a),
        "party" => crate::character::party::workflow(a),
        "party-compile" => crate::character::party::compile_fixture(a),
        "initiative" => crate::character::life::initiative_fixture(a),
        "episodes" => crate::character::life::episodes_fixture(a),
        "continuity" => crate::character::life::continuity_fixture(a),
        "life" => crate::character::life::workflow(a),
        "witness" => crate::character::life::witness::workflow(a),
        "interactions" => crate::character::dialogue::interaction_workflow(a),
        "info" => crate::knowledge::info::workflow(a),
        "inventory" => crate::knowledge::inventory::workflow(a),
        "timeline" => crate::knowledge::timeline::workflow(a),
        "actions" => crate::knowledge::action_catalog::workflow(a),
        "things" => crate::knowledge::things::workflow(a),
        "recipes" => crate::knowledge::recipes::workflow(a),
        "species" => crate::knowledge::species::workflow(a),
        "communities" => crate::knowledge::communities::workflow(a),
        "fitness" => crate::character::fitness::workflow(a),
        "body" => crate::character::body::workflow(a),
        "motive" => crate::character::motive::workflow(a),
        "task" => crate::character::task::workflow(a),
        "affect" => crate::character::affect::workflow(a),
        "skills" => crate::character::skills::workflow(a),
        "mind" => crate::character::mind::workflow(a),
        "deception" => crate::character::deception::workflow(a),
        "relationship" => crate::character::relationship::workflow(a),
        "offscreen" => crate::character::offscreen::workflow(a),
        "curiosity" => crate::character::curiosity::workflow(a),
        "explain" => crate::character::explain::workflow(a),
        "stability" => crate::character::stability::workflow(a),
        "beliefs" => crate::character::beliefs::workflow(a),
        "speech_act" => crate::character::speech_act::workflow(a),
        "diet" => crate::character::diet::workflow(a),
        "creed" => crate::character::creed::workflow(a),
        "expectation" => crate::character::expectation::workflow(a),
        "attention" => crate::character::attention::workflow(a),
        "aspiration" => crate::character::aspiration::workflow(a),
        "habit" => crate::character::habit::workflow(a),
        "cooperation" => crate::character::cooperation::workflow(a),
        "cognition" => crate::character::cognition::workflow(a),
        "chatter" => crate::character::chatter::workflow(a),
        "survival" => crate::character::survival::workflow(a),
        "detail" => crate::character::detail::workflow(a),
        "retention" => crate::character::retention::workflow(a),
        "roles" => crate::character::roles::workflow(a),
        "economy" => crate::character::economy::workflow(a),
        "space" => crate::character::space::workflow(a),
        "lesson" => crate::character::lesson::workflow(a),
        "places" => crate::knowledge::places::workflow(a),
        "character-plan" => crate::character::tools::plan_fixture(a),
        "world" => crate::knowledge::world::process(a),
        "location" => crate::knowledge::location::process(a),
        "quote-phrase" => {
            c::fields(a, &["quote"])?;
            Ok(Value::str(crate::character::dialogue::render_quote(c::text(c::get(a, "quote")?)?)?))
        }
        "lore" => crate::knowledge::lore::workflow(a),
        "answers" => {
            c::fields(a, &["definition", "calls"])?;
            let answers = crate::character::dialogue::Answers::from_value(c::get(a, "definition")?)?;
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
        "conversation" => crate::character::dialogue::conversation_workflow(a),
        "references" => crate::character::dialogue::resolve_references(a),
        "confirmation" => crate::runtime::decision::confirmation_workflow(a),
        "workflow" => {
            c::fields(a, &["size", "task", "name", "identity"])?;
            crate::deploy::deployment::ModelSize::parse(c::text(c::get(a, "size")?)?)?.workflow_profile(
                crate::deploy::deployment::Workload::parse(c::text(c::get(a, "task")?)?)?,
                c::text(c::get(a, "name")?)?,
                &crate::deploy::deployment::Identity::from_value(c::get(a, "identity")?)?,
            )
        }
        "render-direct" => crate::character::dialogue::render_direct(a),
        "tune" => crate::deploy::tune::compare(a),
        "choice-codec" => {
            c::fields(a, &["choices", "names", "calls"])?;
            let choices = crate::runtime::decision::Choices::from_value(c::get(a, "choices")?)?;
            let codec = crate::runtime::decision::ChoiceCodec::new(&choices, c::get(a, "names")?)?;
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
            let choices = crate::runtime::decision::Choices::from_value(c::get(a, "choices")?)?;
            let available = c::list(c::get(a, "available")?)?;
            if available.len() > 32 {
                return Err(c::bad());
            }
            crate::runtime::decision::route(
                &choices,
                c::uint(c::get(a, "revision")?, 0, u32::MAX as u64)? as u32,
                c::get(a, "candidates")?,
                |p| available.iter().any(|q| p.same(q)),
            )
        }
        "compose" => {
            c::fields(a, &["definition", "observations"])?;
            let mut session = crate::packet::request::Composition::from_value(c::get(a, "definition")?)?;
            let observations = c::list(c::get(a, "observations")?)?;
            if observations.len() > 129 {
                return Err(c::bad());
            }
            for observation in observations {
                session.observe(observation)?;
            }
            Ok(session.state())
        }
        "evidence" => crate::knowledge::selection::workflow(a),
        "query-template" => {
            c::fields(a, &["template", "arguments", "rows"])?;
            let template = crate::data::query::Template::from_value(c::get(a, "template")?)?;
            let result = template.run(c::get(a, "arguments")?, c::list(c::get(a, "rows")?)?)?;
            Ok(obj! {"tool"=>template.tool(),"result"=>result})
        }
        "intent" => {
            c::fields(a, &["choices", "intent", "calls"])?;
            let choices = crate::runtime::decision::Choices::from_value(c::get(a, "choices")?)?;
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
            Ok(crate::character::dialogue::Thread::from_value(c::get(a, "thread")?)?
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
            Ok(crate::character::dialogue::Scene::from_value(c::get(a, "scene")?)?
                .assemble(
                    &RequestBudget::from_value(c::get(a, "budget")?)?,
                    usize_arg(a, "protect", 3)?,
                    usize_arg(a, "maximum", 128)?,
                    usize_arg(a, "dialogue_tokens", 1_048_576)?,
                    crate::character::dialogue::render_chat,
                    counter,
                )?
                .to_value())
        }
        "cancel" => {
            c::fields(a, &["choices", "available"])?;
            let choices = crate::runtime::decision::Choices::from_value(c::get(a, "choices")?)?;
            let available = c::list(c::get(a, "available")?)?;
            if available.len() > 32 {
                return Err(c::bad());
            }
            choices.session().cancel(|v| available.iter().any(|p| p.same(v)))
        }
        "decision" => {
            c::fields(a, &["choices", "calls"])?;
            let choices = crate::runtime::decision::Choices::from_value(c::get(a, "choices")?)?;
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
            crate::deploy::deployment::ModelSize::parse(c::text(c::get(a, "size")?)?)?.starting_profile(
                c::text(c::get(a, "name")?)?,
                &crate::deploy::deployment::Identity::from_value(c::get(a, "identity")?)?,
            )
        }
        "profile" => {
            c::fields(a, &["profile", "actual_identity"])?;
            crate::deploy::deployment::Profile::from_value(c::get(a, "profile")?)?
                .describe(&crate::deploy::deployment::Identity::from_value(c::get(a, "actual_identity")?)?)
        }
        "time-assessment" => crate::knowledge::observation::assessment_fixture(a),
        "routine-detail" => crate::character::routine::detail_fixture(a),
        "routine-fixture" => crate::character::routine::fixture(a),
        "report" => crate::deploy::turn::report(a),
        _ => Err(c::bad()),
    }
}
