//! Explicit host-selected settings tied to the complete deployment identity. No automatic
//! model-size rule, template rewriting, model loading or server configuration changes.
use crate::request::{bounded, RequestBudget};
use crate::{checked as c, obj, Error, Result, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct Identity {
    value: Value,
}
impl Identity {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 4096)?;
        let keys = ["model_digest", "quantization", "template_hash", "tokenizer", "runner"];
        c::fields(v, &keys)?;
        for key in keys {
            let s = c::text(c::get(v, key)?)?;
            if s.is_empty() || s.chars().count() > 128 || s.chars().any(char::is_control) {
                return Err(c::bad());
            }
        }
        Ok(Self {
            value: Value::Object(
                keys.iter().map(|key| (key.to_string(), v.get(key).expect("identity field").clone())).collect(),
            ),
        })
    }
    pub fn to_value(&self) -> Value {
        self.value.clone()
    }
    /// Fingerprint for calibration indexing, not security or proof of identity. Profile
    /// matching below compares every complete field, not just this noncryptographic hash.
    pub fn scope(&self) -> u64 {
        crate::fingerprint_u64(&self.value.to_json())
    }
}
#[derive(Clone, Debug)]
pub struct Profile {
    identity: Identity,
    budget: RequestBudget,
    threads: usize,
    transport: String,
    guidance: String,
    temperature: usize,
}
impl Profile {
    pub fn from_value(v: &Value) -> Result<Self> {
        bounded(v, 8192)?;
        c::fields(
            v,
            &["version", "name", "identity", "budget", "threads", "transport", "guidance", "temperature_milli"],
        )?;
        c::uint(c::get(v, "version")?, 1, 1)?;
        c::scope(c::text(c::get(v, "name")?)?)?;
        let identity = Identity::from_value(c::get(v, "identity")?)?;
        let budget = RequestBudget::from_value(c::get(v, "budget")?)?;
        let threads = c::uint(c::get(v, "threads")?, 1, 64)? as usize;
        let transport = c::text(c::get(v, "transport")?)?;
        if !["json", "schema", "tools", "signatures"].contains(&transport) {
            return Err(c::bad());
        }
        let guidance = c::text(c::get(v, "guidance")?)?;
        guidance_text(guidance)?;
        let temperature = c::uint(c::get(v, "temperature_milli")?, 0, 2000)? as usize;
        Ok(Self { identity, budget, threads, transport: transport.into(), guidance: guidance.into(), temperature })
    }
    pub fn require_identity(&self, actual: &Identity) -> Result<()> {
        if self.identity != *actual {
            return Err(Error::new("deployment identity changed; profile must be revalidated"));
        }
        Ok(())
    }
    pub fn budget(&self) -> RequestBudget {
        self.budget
    }
    pub fn guidance(&self) -> &'static str {
        guidance_text(&self.guidance).expect("validated guidance")
    }
    pub fn transport(&self) -> &str {
        &self.transport
    }
    /// Explicit per-call options, not applied to any running service by this library.
    pub fn ollama_options(&self) -> Value {
        let t = self.temperature;
        let temperature = if t % 1000 == 0 {
            Value::from(t / 1000)
        } else {
            Value::Number(format!("{}.{:03}", t / 1000, t % 1000).trim_end_matches('0').to_owned())
        };
        obj! {"num_ctx"=>self.budget.to_value().get("context").expect("context").clone(),"num_predict"=>self.budget.to_value().get("output").expect("output").clone(),"num_thread"=>self.threads,"temperature"=>temperature}
    }
    pub fn describe(&self, actual: &Identity) -> Result<Value> {
        self.require_identity(actual)?;
        Ok(
            obj! {"identity"=>self.identity.to_value(),"guidance"=>self.guidance(),"transport"=>self.transport.as_str(),"options"=>self.ollama_options()},
        )
    }
}
pub fn guidance_text(profile: &str) -> Result<&'static str> {
    match profile {
        "standard"=>Ok(""),
        "concise"=>Ok("Choose only from the supplied legal action/target pairs. Use current facts and the latest player request. Return exactly one JSON object with action and target; no explanation."),
        "example"=>Ok("Return one supplied action/target pair as JSON. Example shape: {\"action\":\"inspect\",\"target\":\"grove:gate\"}. The example is a shape only; use a pair from the current choices, never copy this target."),
        "explicit-selection"=>Ok(crate::query::Guidance::ExplicitSelection.text()),
        _=>Err(Error::new("unknown deployment guidance profile")),
    }
}

/// Advisory workload classes, not architectural context limits or capability promises.
/// The exact model/fine-tune/quantization still needs its own evaluation and identity.
#[derive(Clone, Copy, Debug)]
pub enum ModelSize {
    B1,
    B1_5,
    B3,
    B8,
    B30,
    B70,
}
impl ModelSize {
    pub fn parse(value: &str) -> Result<Self> {
        match value.strip_suffix('b').or_else(|| value.strip_suffix('B')).unwrap_or(value) {
            "1" => Ok(Self::B1),
            "1.5" => Ok(Self::B1_5),
            "3" => Ok(Self::B3),
            "8" => Ok(Self::B8),
            "30" => Ok(Self::B30),
            "70" => Ok(Self::B70),
            _ => Err(Error::new("unknown guideline size; use an explicit profile for other sizes")),
        }
    }
    fn settings(self) -> (&'static str, usize, usize, usize, usize, usize, usize, &'static str) {
        match self {
            Self::B1 => ("1b", 1000, 400, 700, 2048, 64, 4, "Classify one intent or select a supplied legal pair."),
            Self::B1_5 => (
                "1.5b",
                1500,
                500,
                900,
                2048,
                96,
                6,
                "Extract one intent and entity, or select a few prefiltered legal choices.",
            ),
            Self::B3 => (
                "3b",
                3000,
                800,
                1400,
                2048,
                256,
                8,
                "Handle one scene and a bounded decision or short grounded narration.",
            ),
            Self::B8 => {
                ("8b", 8000, 1000, 2200, 4096, 256, 16, "Test richer single-scene reasoning with bounded tool use.")
            }
            Self::B30 => (
                "30b",
                30000,
                1200,
                3200,
                8192,
                384,
                32,
                "Test short explicit multi-step plans with host validation at each step.",
            ),
            Self::B70 => (
                "70b",
                70000,
                1500,
                4800,
                8192,
                512,
                32,
                "Test broader planning or nuanced dialogue while retaining explicit state and tool bounds.",
            ),
        }
    }
    /// Proposed experiment ranges for hydrated content. Tool/template/output accounting
    /// is separate. No tier is marked measured merely because its parameter count matches.
    pub fn guideline(self) -> Value {
        let (label, millions, low, high, context, output, choices, task) = self.settings();
        obj! {"size_class"=>label,"nominal_parameter_millions"=>millions,"task"=>task,
        "content_tokens_min_hint"=>low,"content_tokens_max_hint"=>high,"choices_hint"=>choices,
        "context_start"=>context,"output_start"=>output,"measured"=>false,
        "host_work"=>"Filter candidates, compute arithmetic, enforce permissions and resolve state changes locally."}
    }
    /// Editable v1 profile template. Size guidance adds no implicit model selection and
    /// no stronger prompt guidance; context/output/threads remain explicit configuration.
    pub fn starting_profile(self, name: &str, identity: &Identity) -> Result<Value> {
        c::scope(name)?;
        let (_, _, _, _, context, output, _, _) = self.settings();
        let profile = obj! {"version"=>1,"name"=>name,"identity"=>identity.to_value(),
        "budget"=>obj! {"context"=>context,"output"=>output,"safety"=>256},
        "threads"=>4,"transport"=>"schema","guidance"=>"standard","temperature_milli"=>0};
        Profile::from_value(&profile)?;
        Ok(obj! {"guideline"=>self.guideline(),"profile"=>profile})
    }
}
