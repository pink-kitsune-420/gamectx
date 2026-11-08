//! Source-bound surface wording. Lexical anchors are guardrails, not entailment proof.
use crate::{checked as c, obj, Result, Value};
const GLUE:&str="i m you your me my we our us a an the this that these those it its is are was be been am to of and or for from with at on in here there can could would will do does did have has s re ve ll d t so just still now only all some any what which how as if when then than about into please hello hi welcome thank thanks glad pleased nice good well certainly course yes right alright also quite very tell hear see say share ask answer help something anything one no not never cannot don won haven know told shared name use yet listen feel afraid ah sharing";
fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}
fn glue(s: &str) -> bool {
    GLUE.split_whitespace().any(|w| stem(w) == stem(s))
}
fn stem(w: &str) -> &str {
    if w.ends_with("ing") && w.len() > 5 {
        &w[..w.len() - 3]
    } else if w.ends_with('s') && w.len() > 3 {
        &w[..w.len() - 1]
    } else {
        w
    }
}
fn negative(s: &str) -> bool {
    let low = s.to_lowercase();
    words(s).iter().any(|w| ["not", "no", "never", "cannot"].contains(&w.as_str()))
        || low.contains("n't")
        || low.contains("n’t")
}
fn pairs(ws: &[String]) -> Vec<(String, Vec<String>)> {
    ws.iter()
        .enumerate()
        .filter(|(_, w)| w.chars().any(char::is_numeric))
        .map(|(i, w)| {
            let mut p =
                ws[..i].iter().rev().filter(|w| !glue(w)).take(2).map(|w| stem(w).to_owned()).collect::<Vec<_>>();
            p.reverse();
            (w.clone(), p)
        })
        .collect()
}
/// Required lexical source terms for prompt assembly. Inflected courtesy words
/// share the validator's normalization; these hints grant no world authority.
pub fn wording_anchors(canonical: &str) -> Result<Value> {
    if canonical.is_empty()
        || canonical.len() > 1024
        || canonical.chars().count() > 240
        || canonical.chars().any(char::is_control)
    {
        return Err(c::bad());
    }
    Ok(words(canonical).into_iter().filter(|w| !glue(w)).collect::<Vec<_>>().into())
}
/// Checks source vocabulary, required content anchors, numeric associations and
/// negation before a model's paraphrase can replace an authoritative NPC line.
pub fn validate_wording(v: &Value) -> Result<Value> {
    crate::packet::request::bounded(v, 4096)?;
    c::fields(v, &["canonical", "speech"])?;
    let canonical = c::text(c::get(v, "canonical")?)?;
    let speech = c::text(c::get(v, "speech")?)?;
    for s in [canonical, speech] {
        if s.is_empty() || s.len() > 1024 || s.chars().count() > 240 || s.chars().any(char::is_control) {
            return Err(c::bad());
        }
    }
    let source = words(canonical);
    let got = words(speech);
    let allowed = source.iter().map(|w| stem(w)).chain(GLUE.split_whitespace().map(stem)).collect::<Vec<_>>();
    if got.is_empty()
        || got.iter().any(|w| !allowed.contains(&stem(w)))
        || source.iter().filter(|w| !glue(w)).any(|w| !got.iter().any(|x| stem(x) == stem(w)))
        || negative(canonical) != negative(speech)
        || pairs(&source) != pairs(&got)
        || !speech.ends_with(['.', '!', '?'])
    {
        return Err(c::bad());
    }
    let mut runs = 0;
    let mut last = false;
    for ch in speech.chars() {
        let end = matches!(ch, '.' | '!' | '?');
        if end && !last {
            runs += 1;
        }
        last = end;
    }
    if runs != 1 {
        return Err(c::bad());
    }
    Ok(obj! {"valid"=>true,"world_authority"=>false})
}
