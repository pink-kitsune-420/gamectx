//! Bounded opt-in multi-sentence envelopes and long player input preferences.
use super::*;
use gamectx::obj;
pub struct Options {
    pub value: Value,
}
impl Options {
    pub fn new(dir: &std::path::Path) -> Result<Self> {
        let p = dir.join("conversation.json");
        let value = if p.exists() {
            parse(&read_file(&p, 1024)?)?
        } else {
            obj! {"version"=>1,"max_sentences"=>1,"max_player_chars"=>2048,"summarize_long"=>false}
        };
        validate(&value)?;
        Ok(Self { value })
    }
    pub fn configure(&mut self, v: &Value, dir: &std::path::Path) -> Result<()> {
        validate(v)?;
        atomic(&dir.join("conversation.json"), v.to_json().as_bytes())?;
        self.value = v.clone();
        Ok(())
    }
    pub fn decorate(&self, payload: &mut Value, binding: &mut Value) -> Result<()> {
        let cap = if binding.get("wording").is_some() || binding.get("answers").is_some_and(|v| *v != Value::Null) {
            1
        } else {
            num(&self.value, "max_sentences", 3)?
        };
        let summary = self.value.get("summarize_long") == Some(&Value::Bool(true))
            && payload.get("long_player_message") == Some(&true.into());
        if cap == 1 && !summary {
            return Ok(());
        }
        let mut schema=payload.get("speech_schema").cloned().unwrap_or(obj!{"type"=>"object","properties"=>obj!{"speech"=>obj!{"type"=>"string","maxLength"=>240}},"required"=>vec!["speech"],"additionalProperties"=>false});
        let mut props = get(&schema, "properties")?.clone();
        if cap > 1 {
            let item = get(&props, "speech")?.clone();
            set(
                &mut props,
                "speech",
                obj! {"type"=>"array","minItems"=>if binding.get("expression")==Some(&Value::str("fiction")){2}else{1},"maxItems"=>cap,"items"=>item},
            );
            set(binding, "sentence_chunks", cap.into());
        }
        if summary {
            set(&mut props, "player_summary", obj! {"type"=>"string","minLength"=>1,"maxLength"=>240});
            let mut required = get(&schema, "required")?.as_array().ok_or_else(err)?.to_vec();
            required.insert(0, "player_summary".into());
            set(&mut schema, "required", required.into());
            set(binding, "player_summary", true.into());
            let mut ordered = vec![("player_summary".into(), get(&props, "player_summary")?.clone())];
            ordered.extend(props.as_object().ok_or_else(err)?.iter().filter(|(k, _)| k != "player_summary").cloned());
            props = Value::Object(ordered);
        }
        set(&mut schema, "properties", props);
        set(payload, "speech_schema", schema);
        let mut messages = get(payload, "messages")?.as_array().ok_or_else(err)?.to_vec();
        messages.insert(messages.len()-1,obj!{"role"=>"system","content"=>format!("Return speech JSON. Speech {}. Each item must be one complete sentence under 180 characters, independently grounded in supplied facts or the requested personal stance. No repeated or rephrased duplicates; one sentence is sufficient when there is nothing more to add. {}",if cap>1{format!("is an array of 1 to {cap} sentences")}else{"is a string".into()},if summary{"First produce player_summary, a brief account of what the PLAYER SAID, preserving names, corrections, negations and unresolved questions. It is an unverified aid, not a fact or action; then answer the original message."}else{""})});
        if binding.get("expression") == Some(&Value::str("fiction")) && cap > 1 {
            messages.insert(messages.len()-1,obj!{"role"=>"system","content"=>"For the fable return exactly two short complete sentences: Imagine starts the problem, In this tale, starts its resolution. Follow the supplied plot seed. This is invented, not your biography or the current room."});
        }
        set(payload, "messages", messages.into());
        let tokens = num(payload, "max_tokens", 4096)?;
        set(payload, "max_tokens", tokens.max(if summary { 512 } else { (cap * 96).min(512) }).into());
        Ok(())
    }
}
fn validate(v: &Value) -> Result<()> {
    if !v.is_valid(1024) {
        return Err(err());
    }
    let mut fields_expected = vec!["version", "max_sentences", "max_player_chars", "summarize_long"];
    if let Some(flag) = v.get("ai_wording") {
        fields_expected.push("ai_wording");
        flag.as_bool().ok_or_else(err)?;
    }
    fields(v, &fields_expected)?;
    if num(v, "version", 1)? != 1
        || num(v, "max_sentences", 3)? < 1
        || num(v, "max_player_chars", 4096)? < 512
        || get(v, "summarize_long")?.as_bool().is_none()
    {
        return Err(err());
    }
    Ok(())
}

/// Bounded exact excerpt of an explicit name correction. This remains a player
/// claim, not a verified biography; a generated summary cannot replace it.
pub fn protected_correction(input: &str) -> Value {
    for (at, _) in input.char_indices().filter(|(_, c)| matches!(c, 'm' | 'M')) {
        let tail = &input[at..];
        if tail.get(..11).is_some_and(|s| s.eq_ignore_ascii_case("my name is ")) {
            let end = tail.find(['.', '!', '?']).map_or(tail.len(), |n| n + 1);
            let excerpt = tail[..end].chars().take(140).collect::<String>();
            return obj! {"kind"=>"player_claim","text"=>excerpt};
        }
    }
    Value::Null
}
