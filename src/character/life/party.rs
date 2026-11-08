use super::*;
impl Life {
    pub(crate) fn gate_attitudes(&self, frame: &Value) -> Result<Value> {
        self.active()?;
        bounded(frame, 8192)?;
        c::fields(frame, &["revision", "speaker", "listeners"])?;
        let listeners = names(get(frame, "listeners")?, 16)?;
        if num(frame, "revision")? != self.revision
            || !self.participants.iter().any(|p| p == text(frame, "speaker").unwrap_or(""))
            || listeners.is_empty()
            || !subset(&listeners, &self.participants)
        {
            return Err(c::bad());
        }
        Ok(self.attitudes.iter().map(|a|{let d=get(a,"definition").expect("definition");obj!{"kind"=>get(d,"kind").expect("kind").clone(),"subject"=>get(d,"subject").expect("subject").clone(),"level"=>get(a,"level").expect("level").clone()}}).collect::<Vec<_>>().into())
    }
    /// Party-aware selection before attention caps. Private facets still require their
    /// authored audience to cover the fixed Life scope. Attitudes follow subject order,
    /// so addressing players precede optional participants. Inspection is host-only.
    pub fn party_context(
        &self,
        selection: &Value,
        caps: ContextCaps,
        turn: &SpeechTurn,
        gates: &crate::character::party::AttitudeGates,
        frame: &Value,
    ) -> Result<Value> {
        ContextCaps::new(caps.facts, caps.facets, caps.attitudes, caps.episodes, caps.exchanges)?;
        bounded(selection, 8192)?;
        c::fields(selection, &["tags", "subjects"])?;
        let tags = get(selection, "tags")?;
        let subjects = get(selection, "subjects")?;
        self.gate_attitudes(frame)?;
        let subjects = names(subjects, 16)?;
        if !subset(&subjects, &self.participants) {
            return Err(c::bad());
        }
        let tags = names(tags, 8)?;
        let mut empty = caps;
        empty.facets = 0;
        empty.attitudes = 0;
        let mut context = self.context_for_turn(&Value::strings(&tags), empty, turn)?;
        let visible = |v: &Value| -> bool {
            names(get(v, "audience").expect("audience"), 16).is_ok_and(|a| subset(&self.participants, &a))
                && (tags.is_empty()
                    || c::list(get(v, "tags").expect("tags"))
                        .expect("tags")
                        .iter()
                        .any(|t| tags.iter().any(|x| t.as_str() == Some(x))))
        };
        let facets =
            c::list(get(&self.definition.value, "facets")?)?.iter().filter(|f| visible(f)).cloned().collect::<Vec<_>>();
        let selected = gates.select(self, frame, "facet", &facets.into())?;
        let facets = c::list(get(&selected, "packet")?)?.iter().take(caps.facets).cloned().collect::<Vec<_>>();
        let attitudes = subjects
            .iter()
            .flat_map(|subject| {
                self.attitudes.iter().filter(|a| {
                    let d = get(a, "definition").expect("definition");
                    d.get("subject").and_then(Value::as_str) == Some(subject)
                        && visible(d)
                        && (self.config.mistaken || d.get("kind").and_then(Value::as_str) != Some("misconception"))
                })
            })
            .take(caps.attitudes)
            .cloned()
            .collect::<Vec<_>>();
        super::continuity::set(&mut context, "facets", facets.into());
        super::continuity::set(&mut context, "attitudes", attitudes.into());
        Ok(obj! {"context"=>context,"withheld"=>get(&selected,"withheld")?.clone()})
    }
}
