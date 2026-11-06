use super::*;
/// One narrow model job. Size alone does not establish task capability.
#[derive(Clone, Copy, Debug)]
pub enum Workload {
    Interpret,
    Dialogue,
    Narration,
    Query,
}
impl Workload {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "interpret" => Ok(Self::Interpret),
            "dialogue" => Ok(Self::Dialogue),
            "narration" => Ok(Self::Narration),
            "query" => Ok(Self::Query),
            _ => Err(Error::new("unknown workload")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Interpret => "interpret",
            Self::Dialogue => "dialogue",
            Self::Narration => "narration",
            Self::Query => "query",
        }
    }
}
/// Advisory counts. Required facts/dialogue must still preserve or fail.
#[derive(Clone, Copy, Debug)]
pub struct WorkloadHints {
    pub output_tokens: usize,
    pub exchanges: usize,
    pub recall: usize,
}
impl ModelSize {
    pub fn workload_hints(self, task: Workload) -> WorkloadHints {
        let i = match self {
            Self::B1 => 0,
            Self::B1_5 => 1,
            Self::B3 => 2,
            Self::B8 => 3,
            Self::B30 => 4,
            Self::B70 => 5,
        };
        WorkloadHints {
            output_tokens: match task {
                Workload::Interpret => 32,
                Workload::Dialogue => [64, 96, 128, 160, 192, 192][i],
                Workload::Narration => [64, 96, 128, 192, 256, 256][i],
                Workload::Query => [64, 96, 128, 160, 192, 256][i],
            },
            exchanges: if matches!(task, Workload::Dialogue) { [1, 1, 1, 2, 3, 3][i] } else { 0 },
            recall: if matches!(task, Workload::Dialogue) { [1, 1, 2, 3, 4, 6][i] } else { 0 },
        }
    }
    /// Editable size/task template. Every generic tier is unmeasured. No model is
    /// loaded, no configuration is applied, and no content is silently truncated.
    pub fn workflow_profile(self, task: Workload, name: &str, identity: &Identity) -> Result<Value> {
        let hints = self.workload_hints(task);
        let mut value = self.starting_profile(name, identity)?;
        if let Value::Object(root) = &mut value {
            if let Some((_, Value::Object(profile))) = root.iter_mut().find(|(k, _)| k == "profile") {
                if let Some((_, Value::Object(budget))) = profile.iter_mut().find(|(k, _)| k == "budget") {
                    if let Some((_, v)) = budget.iter_mut().find(|(k, _)| k == "output") {
                        *v = Value::from(hints.output_tokens);
                    }
                }
            }
            root.push(("workflow".into(),obj! {"version"=>1,"task"=>task.name(),"size_class"=>self.settings().0,"measured"=>false,"renderer"=>"direct",
                "response"=>match task {Workload::Interpret=>"choice",Workload::Query=>"sql",_=>"speech"},"exchanges_hint"=>hints.exchanges,"recall_hint"=>hints.recall,
                "choices_hint"=>self.settings().6,"include_quest_goal"=>false,"required_policy"=>"preserve_or_fail","hint_policy"=>"editable_not_capability_limits"}));
        }
        Ok(value)
    }
}
