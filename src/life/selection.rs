//! Opt-in bounded working memory maintenance and explicitly scoped archival recall.
use super::*;
impl Life {
    /// Keep retained, appraisal-supporting and causally connected events plus the newest
    /// complete transient groups. No retained evidence is silently archived or discarded.
    /// If mandatory groups exceed the requested capacity, fail without changing state.
    pub fn trim_transient(&mut self, keep: usize) -> Result<usize> {
        self.active()?;
        if keep > 128 {
            return Err(c::bad());
        }
        let mut chosen = vec![false; self.events.len()];
        for (i, e) in self.events.iter().enumerate() {
            let token = get(e, "token")?;
            chosen[i] = get(e, "retain")? == &Value::Bool(true)
                || self.changes.iter().any(|x| x.get("event") == Some(token))
                || self.attitudes.iter().any(|a| {
                    ["supports", "contradicts"]
                        .iter()
                        .any(|k| c::list(get(a, k).expect("attitude")).expect("tokens").contains(token))
                });
        }
        close(&self.events.iter().collect::<Vec<_>>(), &mut chosen, false)?;
        if chosen.iter().filter(|b| **b).count() > keep {
            return Err(c::bad());
        }
        let pool = self.events.iter().collect::<Vec<_>>();
        for i in (0..pool.len()).rev() {
            if chosen[i] {
                continue;
            }
            let mut trial = chosen.clone();
            trial[i] = true;
            close(&pool, &mut trial, false)?;
            if trial.iter().filter(|b| **b).count() <= keep {
                chosen = trial;
            }
        }
        let removed = chosen.iter().filter(|b| !**b).count();
        if removed > 0 {
            self.bump()?;
            let mut i = 0;
            self.events.retain(|_| {
                let keep = chosen[i];
                i += 1;
                keep
            });
        }
        Ok(removed)
    }
}
/// Close both directions, preserving corrections and the original event they correct.
fn close(pool: &[&Value], chosen: &mut [bool], strict: bool) -> Result<()> {
    loop {
        let mut changed = false;
        for (i, e) in pool.iter().enumerate() {
            for cause in c::list(get(e, "causes")?)? {
                let found = pool.iter().position(|other| other.get("token") == Some(cause));
                if let Some(j) = found {
                    if chosen[i] || chosen[j] {
                        if !chosen[i] || !chosen[j] {
                            changed = true;
                        }
                        chosen[i] = true;
                        chosen[j] = true;
                    }
                } else if chosen[i] && strict {
                    return Err(c::bad());
                }
            }
        }
        if !changed {
            return Ok(());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn life() -> Life {
        let d = Arc::new(
            Definition::draft(
                &crate::json::parse(include_str!("../../examples/characters/merchant-life.json"), MAX_BYTES).unwrap(),
                &Setting::from_value(
                    &crate::json::parse(include_str!("../../examples/characters/setting.json"), MAX_BYTES).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        );
        Life::accept(
            "deep",
            "merchant",
            &vec!["merchant", "player"].into(),
            d,
            Config::resolve(&obj! {"enabled"=>true}, &obj! {}).unwrap(),
        )
        .unwrap()
    }
    fn event(token: usize, retain: bool, causes: Vec<usize>) -> Value {
        obj! {"token"=>token,"at"=>token,"kind"=>"direct","tag"=>"chatter","subject"=>"player","source"=>obj!{"id"=>format!("receipt_{token}"),"speaker"=>"merchant","text"=>"Confirmed speech."},"audience"=>vec!["merchant","player"],"text"=>"Routine chatter.","causes"=>causes,"retain"=>retain}
    }
    #[test]
    fn compaction_keeps_retained_groups_and_fails_atomically() {
        let mut l = life();
        l.experience(&event(1, false, vec![])).unwrap();
        l.experience(&event(2, true, vec![1])).unwrap();
        l.experience(&event(3, false, vec![])).unwrap();
        let before = l.inspect();
        assert!(l.trim_transient(1).is_err());
        assert_eq!(l.inspect(), before);
        assert_eq!(l.trim_transient(2).unwrap(), 1);
        assert_eq!(l.events.len(), 2);
        assert_eq!(l.events[0].get("token"), Some(&1.into()));
    }
    #[test]
    fn thousand_transients_remain_bounded_with_retained_anchor() {
        let mut l = life();
        l.experience(&event(1, true, vec![])).unwrap();
        for n in 2..=1000 {
            if l.events.len() == 128 {
                l.trim_transient(64).unwrap();
            }
            l.experience(&event(n, false, vec![])).unwrap();
        }
        assert_eq!(l.events.len(), 104);
        assert_eq!(l.events[0].get("token"), Some(&1.into()));
    }
}
