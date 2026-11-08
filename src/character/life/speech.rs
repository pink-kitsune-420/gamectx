//! Versioned turn-aware speech context. Subject/status declarations come from the host;
//! compact directives and punctuation checks do not establish semantic grounding.
use super::*;
pub(crate) const GROUNDED: &str = "State only the supplied facts, attitude, and unknown status in one short sentence; do not offer unlisted stock or speculate about unknown weather/history.";
pub(crate) const EXPRESSIVE: &str = "Opinions, humor and imagined hypotheticals may be expressed. Separate facts, reports, beliefs, inference and proposals. Answer known parts and relevant uncertainty; invented flavor is not lore or experience.";
/// Host turn policy and explicitly subject-scoped factual declarations. Models cannot
/// choose this object through a tool. Unknown is a status, not an unbound null attribute.
#[derive(Clone, Debug)]
pub struct SpeechTurn {
    pub(super) value: Value,
}
impl SpeechTurn {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 65536)?;
        c::fields(v, &["profile", "knowledge", "creative", "speaker", "question_subject", "facts"])?;
        if !matches!(text(v, "profile")?, "focused" | "expanded")
            || !matches!(text(v, "knowledge")?, "known" | "partial" | "unknown")
        {
            return Err(c::bad());
        }
        c::boolean(get(v, "creative")?)?;
        for k in ["speaker", "question_subject"] {
            c::scope(text(v, k)?)?;
        }
        let facts = c::list(get(v, "facts")?)?;
        if facts.len() > 32 {
            return Err(c::bad());
        }
        let (mut known, mut unknown) = (0, 0);
        for (i, f) in facts.iter().enumerate() {
            c::fields(f, &["key", "subject", "status", "value"])?;
            let key = text(f, "key")?;
            c::scope(key)?;
            c::scope(text(f, "subject")?)?;
            if matches!(key, "question_subject" | "utterance_speaker" | "fact_subjects")
                || facts[..i].iter().any(|o| o.get("key") == f.get("key"))
            {
                return Err(c::bad());
            }
            bounded(get(f, "value")?, 1024)?;
            match text(f, "status")? {
                "known" if get(f, "value")? != &Value::Null => known += 1,
                "unknown" if get(f, "value")? == &Value::Null => unknown += 1,
                _ => return Err(c::bad()),
            }
        }
        match text(v, "knowledge")? {
            "known" if unknown > 0 => return Err(c::bad()),
            "partial" if unknown == 0 || known == 0 => return Err(c::bad()),
            "unknown" if unknown == 0 => return Err(c::bad()),
            _ => {}
        }
        Ok(Self { value: v.clone() })
    }
    pub fn from_hints(
        hints: crate::deploy::deployment::WorkloadHints,
        knowledge: &str,
        creative: bool,
        speaker: &str,
        question_subject: &str,
        facts: &Value,
    ) -> Result<Self> {
        Self::from_value(
            &obj! {"profile"=>if hints.exchanges<=1{"focused"}else{"expanded"},"knowledge"=>knowledge,"creative"=>creative,"speaker"=>speaker,"question_subject"=>question_subject,"facts"=>facts.clone()},
        )
    }
    pub(super) fn validate_scope(&self, life: &Life, caps: ContextCaps) -> Result<()> {
        if !life.participants.iter().any(|p| Some(p.as_str()) == self.value.get("speaker").and_then(Value::as_str))
            || !life
                .definition
                .entities
                .iter()
                .any(|e| Some(e.as_str()) == self.value.get("question_subject").and_then(Value::as_str))
        {
            return Err(c::bad());
        }
        let facts = c::list(get(&self.value, "facts")?)?;
        if facts.len() > caps.facts {
            return Err(c::bad());
        }
        if facts.iter().any(|f| {
            !life.definition.entities.iter().any(|e| Some(e.as_str()) == f.get("subject").and_then(Value::as_str))
        }) {
            return Err(c::bad());
        }
        Ok(())
    }
    pub(super) fn expressive(&self) -> Result<bool> {
        Ok(text(&self.value, "knowledge")? == "known"
            && (text(&self.value, "profile")? == "expanded" || c::boolean(get(&self.value, "creative")?)?))
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    pub(crate) fn scoped_facts(&self, actor: &str) -> Result<Value> {
        let rows = c::list(get(&self.value, "facts")?)?;
        let subjects = Value::Object(
            rows.iter()
                .map(|f| (text(f, "key").expect("key").into(), get(f, "subject").expect("subject").clone()))
                .collect(),
        );
        let mut facts = vec![
            ("question_subject".into(), get(&self.value, "question_subject")?.clone()),
            ("utterance_speaker".into(), get(&self.value, "speaker")?.clone()),
            ("fact_subjects".into(), subjects),
        ];
        for f in rows {
            facts.push((
                text(f, "key")?.into(),
                if text(f, "status")? == "unknown" {
                    Value::str(format!("unknown_to_{actor}"))
                } else {
                    get(f, "value")?.clone()
                },
            ));
        }
        Ok(Value::Object(facts))
    }
}
/// Mechanical punctuation/length diagnostics. This is not grammatical parsing,
/// entailment checking or proof of a completed thought; raw text is never rewritten.
#[derive(Clone, Debug)]
pub struct SentenceAssessment {
    value: Value,
}
impl SentenceAssessment {
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
}
pub fn assess_sentence(speech: &str, max_chars: usize) -> Result<SentenceAssessment> {
    if !(1..=1024).contains(&max_chars) || speech.len() > 4096 {
        return Err(c::bad());
    }
    let characters = speech.chars().count();
    if characters > 1024 {
        return Err(c::bad());
    }
    let body = speech.trim_end().trim_end_matches(['"', '\'', '”', '’', ')', ']']).trim_end();
    let terminal = |ch: char| matches!(ch, '.' | '!' | '?' | '。' | '！' | '？');
    let complete = body.chars().last().is_some_and(terminal) && !body.ends_with("...") && !body.ends_with('…');
    let mut previous = None;
    let mut iter = speech.chars().peekable();
    let (mut count, mut in_run) = (0, false);
    while let Some(ch) = iter.next() {
        let end = terminal(ch)
            && !(ch == '.'
                && previous.is_some_and(|c: char| c.is_ascii_digit())
                && iter.peek().is_some_and(char::is_ascii_digit));
        if end && !in_run {
            count += 1;
        }
        in_run = end;
        previous = Some(ch);
    }
    Ok(SentenceAssessment {
        value: obj! {"characters"=>characters,"boundary_complete"=>complete,"single_sentence"=>complete&&count==1,"at_schema_wall"=>characters==max_chars,"over_limit"=>characters>max_chars,"suspected_cutoff"=>characters==max_chars&&!complete},
    })
}
/// Additive offline prototype/parity adapter. No model calls or tool exposure.
pub fn fixture(a: &Value) -> Result<Value> {
    c::fields(a, &["life", "request"])?;
    let l = get(a, "life")?;
    c::fields(l, &["setting", "definition", "defaults", "overrides", "scope", "actor", "participants", "operations"])?;
    let d = Arc::new(Definition::draft(get(l, "definition")?, &Setting::from_value(get(l, "setting")?)?)?);
    let mut life = Life::accept(
        text(l, "scope")?,
        text(l, "actor")?,
        get(l, "participants")?,
        d,
        Config::resolve(get(l, "defaults")?, get(l, "overrides")?)?,
    )?;
    let ops = c::list(get(l, "operations")?)?;
    if ops.len() > 128 {
        return Err(c::bad());
    }
    for op in ops {
        c::fields(op, &["op", "args"])?;
        let v = get(op, "args")?;
        match text(op, "op")? {
            "experience" => life.experience(v)?,
            "appraise" => {
                c::fields(v, &["rule", "event", "revision"])?;
                life.appraise(text(v, "rule")?, num(v, "event")?, num(v, "revision")?)?;
            }
            _ => return Err(c::bad()),
        }
    }
    let r = get(a, "request")?;
    let turn = SpeechTurn::from_value(get(r, "turn")?)?;
    let character = life.context_for_turn(get(r, "tags")?, ContextCaps::from_value(get(r, "caps")?)?, &turn)?;
    Ok(obj! {"character"=>character,"request"=>life.compile_speech(r)?})
}
