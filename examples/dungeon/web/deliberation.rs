//! Host-only opt-in controls. Notes are ephemeral diagnostics, never world state.
use super::*;
use gamectx::obj;
pub struct State {
    pub config: Value,
    pub latest: Value,
}
impl State {
    pub fn new(dir: &std::path::Path) -> Result<Self> {
        let file = dir.join("deliberation.json");
        let config = if file.exists() {
            parse(&read_file(&file, 8192)?)?
        } else {
            obj! {"version"=>1,"mode"=>"off","effort"=>"low","max_tokens"=>768,"hypotheses"=>false,"online"=>Vec::<Value>::new()}
        };
        gamectx::studio::validate_deliberation_config(&config)?;
        Ok(Self { config, latest: Value::Null })
    }
    pub fn configure(&mut self, v: &Value, dir: &std::path::Path) -> Result<()> {
        gamectx::studio::validate_deliberation_config(v)?;
        atomic(&dir.join("deliberation.json"), v.to_json().as_bytes())?;
        self.config = v.clone();
        self.latest = Value::Null;
        Ok(())
    }
    pub fn plan(
        &self,
        config: &providers::Config,
        backend: &str,
        report: &Value,
        task: &str,
        payload: &Value,
    ) -> Result<Value> {
        let mut model = super::setup::selected_model_for(config, backend);
        if model.is_empty() {
            model = "unconfigured".into();
        }
        let values = report
            .get("thinking_controls")
            .and_then(Value::as_array)
            .unwrap_or(&[])
            .iter()
            .find(|v| {
                v.get("model").and_then(Value::as_str) == Some(model.as_str())
                    && v.get("endpoint") == config.endpoint(backend).ok().and_then(|e| e.get("url"))
            })
            .and_then(|v| v.get("values"))
            .cloned()
            .unwrap_or(Value::Null);
        gamectx::studio::deliberation_plan(
            &obj! {"config"=>self.config.clone(),"backend"=>backend,"model"=>model,"values"=>values,"task"=>task,"expanded"=>gamectx::deployment::ModelSize::parse(text(&config.value,"size")?)?.workload_hints(gamectx::deployment::Workload::Dialogue).exchanges>1,"output_tokens"=>get(payload,"max_tokens")?.clone()},
        )
    }
}
/// Conservative finite classifier. No native reasoning for routine/social replies.
pub fn task(input: &str, conflict: bool) -> &'static str {
    if conflict {
        return "conflict";
    }
    let s = input.to_lowercase();
    if ["what could", "what might", "possible explanation", "hypothesis"].iter().any(|w| s.contains(w)) {
        "investigation"
    } else if ["weigh the", "compare our", "plan our", "make a plan", "which option"].iter().any(|w| s.contains(w)) {
        "planning"
    } else {
        "routine"
    }
}
pub fn apply(payload: &mut Value, plan: &Value, evidence: &Value) -> Result<()> {
    set(payload, "deliberation", plan.clone());
    set(payload, "max_tokens", get(plan, "max_tokens")?.clone());
    if plan.get("hypotheses") == Some(&Value::Bool(true)) {
        // The existing speech schema remains authoritative. Only its optional notes envelope grows.
        let mut schema = get(payload, "speech_schema")?.clone();
        let mut props = get(&schema, "properties")?.clone();
        let ids = array(get(evidence, "ids")?)?.to_vec();
        set(
            &mut props,
            "hypotheses",
            obj! {"type"=>"array","maxItems"=>3,"items"=>obj!{"type"=>"object","properties"=>obj!{"claim"=>obj!{"type"=>"string","minLength"=>1,"maxLength"=>180},"evidence"=>obj!{"type"=>"array","minItems"=>1,"maxItems"=>4,"items"=>obj!{"type"=>"string","enum"=>ids}},"check"=>obj!{"type"=>"string","minLength"=>1,"maxLength"=>180}},"required"=>vec!["claim","evidence","check"],"additionalProperties"=>false}},
        );
        set(&mut schema, "properties", props);
        let mut required = array(get(&schema, "required")?)?.to_vec();
        required.push("hypotheses".into());
        set(&mut schema, "required", required.into());
        set(payload, "speech_schema", schema);
        let mut messages = array(get(payload, "messages")?)?.to_vec();
        messages.insert(messages.len()-1,obj!{"role"=>"system","content"=>format!("Keep speech within its authorized schema. Separately return hypotheses: up to three short UNVERIFIED possibilities, each with claim, evidence IDs and a check. Empty is valid. Evidence: {}. These notes confer no knowledge or permission; do not put private reasoning in speech.",get(evidence,"facts")?.to_json())});
        set(payload, "messages", messages.into());
        let tokens = num(payload, "max_tokens", 4096)?;
        set(payload, "max_tokens", tokens.max(512).into());
    }
    Ok(())
}
fn array(v: &Value) -> Result<&[Value]> {
    v.as_array().ok_or_else(err)
}
