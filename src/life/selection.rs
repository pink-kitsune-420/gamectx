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

impl Life {
    /// Opt-in original-event recall across bounded consolidations and the working set.
    /// Host-selected subjects/required anchors are authoritative, not inferred from prose.
    /// Required causal groups must fit in full; irrelevant optional episodes are omitted.
    pub fn recall_selected(&self, q: &Value) -> Result<Value> {
        self.active()?;
        bounded(q, 8192)?;
        c::fields(q, &["input", "tags", "subjects", "required", "count", "archive"])?;
        let input = text(q, "input")?;
        if input.len() > 2048 {
            return Err(c::bad());
        }
        let tags = names(get(q, "tags")?, 8)?;
        let subjects = names(get(q, "subjects")?, 16)?;
        if subjects.is_empty() || !subset(&subjects, &self.definition.entities) {
            return Err(c::bad());
        }
        let count = c::uint(get(q, "count")?, 0, 16)? as usize;
        let archived = c::boolean(get(q, "archive")?)?;
        let required = c::list(get(q, "required")?)?;
        if required.len() > 16 {
            return Err(c::bad());
        }
        for (i, n) in required.iter().enumerate() {
            c::uint(n, 1, self.watermark)?;
            if required[..i].contains(n) {
                return Err(c::bad());
            }
        }
        let mut pool = self.events.iter().collect::<Vec<_>>();
        if archived {
            for summary in &self.summaries {
                for event in c::list(get(summary, "events")?)? {
                    if !pool.iter().any(|e| e.get("token") == event.get("token")) {
                        pool.push(event);
                    }
                }
            }
        }
        pool.sort_by_key(|e| num(e, "token").expect("token"));
        let mut chosen = vec![false; pool.len()];
        for token in required {
            let i = pool.iter().position(|e| e.get("token") == Some(token)).ok_or_else(c::bad)?;
            chosen[i] = true;
        }
        close(&pool, &mut chosen, true)?;
        let allowed = |selected: &[bool]| {
            selected.iter().filter(|b| **b).count() <= count
                && pool.iter().zip(selected).all(|(e, yes)| {
                    !yes || subjects.iter().any(|s| e.get("subject").and_then(Value::as_str) == Some(s))
                })
        };
        if !allowed(&chosen) {
            return Err(c::bad());
        }
        if chosen.iter().filter(|b| **b).count() == count {
            return Ok(pool
                .iter()
                .zip(chosen)
                .filter(|(_, yes)| *yes)
                .map(|(e, _)| (*e).clone())
                .collect::<Vec<_>>()
                .into());
        }
        let wanted = terms(&format!("{input} {}", tags.join(" ")));
        let mut ranked = pool
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let labels = terms(&format!("{} {}", text(e, "text").expect("event"), text(e, "tag").expect("tag")));
                let score = wanted.intersection(&labels).count();
                (
                    i,
                    score,
                    get(e, "retain").expect("retain") == &Value::Bool(true),
                    num(e, "at").expect("at"),
                    num(e, "token").expect("token"),
                )
            })
            .filter(|(_, score, _, _, _)| *score > 0)
            .collect::<Vec<_>>();
        ranked.sort_by_key(|(i, score, retain, at, token)| {
            (
                std::cmp::Reverse(*score),
                std::cmp::Reverse(*retain),
                std::cmp::Reverse(*at),
                std::cmp::Reverse(*token),
                *i,
            )
        });
        for (i, _, _, _, _) in ranked {
            if chosen.iter().filter(|b| **b).count() == count {
                break;
            }
            if chosen[i] {
                continue;
            }
            let mut trial = chosen.clone();
            trial[i] = true;
            if close(&pool, &mut trial, true).is_ok() && allowed(&trial) {
                chosen = trial;
            }
        }
        Ok(pool.iter().zip(chosen).filter(|(_, yes)| *yes).map(|(e, _)| (*e).clone()).collect::<Vec<_>>().into())
    }
}
fn terms(s: &str) -> std::collections::BTreeSet<String> {
    let mut terms = std::collections::BTreeSet::new();
    let lower = s.to_lowercase();
    for word in lower.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        if word.chars().count() > 3 && word.ends_with('s') {
            terms.insert(word[..word.len() - 1].to_owned());
        }
        terms.insert(word.to_owned());
    }
    terms
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
    #[test]
    fn archival_erasure_includes_causal_descendants_in_tombstones() {
        let mut l = life();
        l.experience(&event(1, true, vec![])).unwrap();
        l.experience(&event(2, true, vec![1])).unwrap();
        l.consolidate(1, 2).unwrap();
        l.acknowledge_archive(2).unwrap();
        l.evict(2).unwrap();
        let result = l.forget_episodes(&vec![1].into(), l.revision()).unwrap();
        assert_eq!(result.get("forgotten"), Some(&vec![1, 2].into()));
        assert!(l.recall_selected(&obj!{"input"=>"chatter","tags"=>Vec::<Value>::new(),"subjects"=>vec!["player"],"required"=>vec![2],"count"=>2,"archive"=>true}).is_err());
    }
}
