use super::*;

#[derive(Clone, Debug)]
struct Feature {
    name: String,
    unit: String,
    low: i64,
    high: i64,
    invert: bool,
    missing: String,
    outside: String,
}

/// Versioned host meanings for a graph's ordered inputs. Ranges are physical units;
/// numerical conversion uses millionths and i128 intermediate arithmetic.
#[derive(Clone, Debug)]
pub struct FeatureSchema {
    id: String,
    revision: usize,
    features: Vec<Feature>,
}

/// Fixed-size normalized feature vector, in schema/brain input order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Features {
    values: [Option<i64>; MAX_INPUTS],
    len: usize,
}
impl Features {
    pub fn as_slice(&self) -> &[Option<i64>] {
        &self.values[..self.len]
    }
}

fn physical(value: &Value) -> Result<i64> {
    ticks(value, -1_000_000_000 * S, 1_000_000_000 * S)
}
fn policy(value: &Value, allowed: &[&str]) -> Result<String> {
    let p = text(value)?;
    if !allowed.contains(&p) {
        return Err(err("unknown feature policy"));
    }
    Ok(p.to_owned())
}
impl FeatureSchema {
    pub fn parse(text: &str) -> Result<Self> {
        Self::from_value(&crate::json::parse(text, 16_384)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        fields(v, &["version", "id", "revision", "features"])?;
        integer(get(v, "version")?, 1, 1)?;
        let id = text(get(v, "id")?)?;
        name(id)?;
        let revision = integer(get(v, "revision")?, 1, 1_000_000)?;
        let rows = list(get(v, "features")?)?;
        if rows.is_empty() || rows.len() > MAX_INPUTS {
            return Err(err("feature count outside limits"));
        }
        let mut features: Vec<Feature> = Vec::new();
        for row in rows {
            fields(row, &["name", "unit", "min", "max", "invert", "missing", "outside"])?;
            let key = text(get(row, "name")?)?;
            name(key)?;
            let unit = text(get(row, "unit")?)?;
            if unit.is_empty() || unit.chars().count() > 32 || unit.chars().any(char::is_control) {
                return Err(err("invalid feature unit"));
            }
            let low = physical(get(row, "min")?)?;
            let high = physical(get(row, "max")?)?;
            if low >= high || features.iter().any(|old| old.name == key) {
                return Err(err("invalid or duplicate feature range/name"));
            }
            let invert = get(row, "invert")?.as_bool().ok_or_else(|| err("invert must be boolean"))?;
            features.push(Feature {
                name: key.into(),
                unit: unit.into(),
                low,
                high,
                invert,
                missing: policy(get(row, "missing")?, &["unknown", "error"])?,
                outside: policy(get(row, "outside")?, &["unknown", "error", "clamp"])?,
            });
        }
        Ok(Self { id: id.into(), revision, features })
    }
    pub fn to_value(&self) -> Value {
        crate::obj! {"version"=>1,"id"=>self.id.as_str(),"revision"=>self.revision,"features"=>self.features.iter().map(|f|
        crate::obj!{"name"=>f.name.as_str(),"unit"=>f.unit.as_str(),"min"=>number(f.low),"max"=>number(f.high),"invert"=>f.invert,"missing"=>f.missing.as_str(),"outside"=>f.outside.as_str()}).collect::<Vec<_>>()}
    }
    pub fn validate(&self, brain: &Brain) -> Result<()> {
        if self.features.iter().map(|f| &f.name).ne(brain.inputs.iter()) {
            return Err(err("feature names/order do not match brain"));
        }
        Ok(())
    }
    /// Native raw values must be finite within +/-1e9 physical units. Missing and range
    /// handling are explicit. No silent zero substitution or automatic unit inference.
    pub fn normalize(&self, raw: &Value) -> Result<Features> {
        let entries = raw.as_object().ok_or_else(|| err("raw features must be an object"))?;
        if entries.len() > self.features.len()
            || entries.iter().enumerate().any(|(i, (key, _))| {
                !self.features.iter().any(|f| &f.name == key) || entries[..i].iter().any(|(old, _)| old == key)
            })
        {
            return Err(err("unknown or duplicate raw feature"));
        }
        let mut out = Features { values: [None; MAX_INPUTS], len: self.features.len() };
        for (i, f) in self.features.iter().enumerate() {
            let value = match raw.get(&f.name) {
                None | Some(Value::Null) => {
                    if f.missing == "error" {
                        return Err(err("missing required feature"));
                    }
                    continue;
                }
                Some(v) => v,
            };
            let mut value = physical(value)?;
            if !(f.low..=f.high).contains(&value) {
                match f.outside.as_str() {
                    "error" => return Err(err("physical feature outside range")),
                    "unknown" => continue,
                    _ => value = value.clamp(f.low, f.high),
                }
            }
            let normalized = ((i128::from(value - f.low) * i128::from(S) + i128::from(f.high - f.low) / 2)
                / i128::from(f.high - f.low)) as i64;
            out.values[i] = Some(if f.invert { S - normalized } else { normalized });
        }
        Ok(out)
    }
    pub fn save(&self, brain: &Brain) -> Result<Value> {
        self.validate(brain)?;
        Ok(crate::obj! {"version"=>1,"schema"=>self.to_value(),"brain"=>brain.to_value()})
    }
}

/// Brain plus its declared host bindings. Restore requires the complete expected schema,
/// including units/ranges/policies/version, rather than trusting only a hash or version ID.
#[derive(Clone, Debug)]
pub struct BoundBrain {
    brain: Brain,
    schema: FeatureSchema,
}
impl BoundBrain {
    pub fn new(brain: Brain, schema: FeatureSchema) -> Result<Self> {
        schema.validate(&brain)?;
        Ok(Self { brain, schema })
    }
    pub fn restore_json(text: &str, expected: &FeatureSchema) -> Result<Self> {
        Self::restore(&crate::json::parse(text, MAX_STATE_BYTES + 16_384)?, expected)
    }
    pub fn restore(v: &Value, expected: &FeatureSchema) -> Result<Self> {
        fields(v, &["version", "schema", "brain"])?;
        integer(get(v, "version")?, 1, 1)?;
        let schema = FeatureSchema::from_value(get(v, "schema")?)?;
        if schema.to_value() != expected.to_value() {
            return Err(err("saved feature schema mismatch"));
        }
        Self::new(Brain::restore(get(v, "brain")?)?, schema)
    }
    pub fn to_value(&self) -> Result<Value> {
        self.schema.save(&self.brain)
    }
    pub fn brain(&self) -> &Brain {
        &self.brain
    }
    pub fn brain_mut(&mut self) -> &mut Brain {
        &mut self.brain
    }
    pub fn schema(&self) -> &FeatureSchema {
        &self.schema
    }
    pub fn into_parts(self) -> (Brain, FeatureSchema) {
        (self.brain, self.schema)
    }
    pub fn run_into(&self, raw: &Value, allowed: u16, low: i64, gap: i64, out: &mut Prediction) -> Result<()> {
        self.schema.validate(&self.brain)?;
        let inputs = self.schema.normalize(raw)?;
        self.brain.run_into(inputs.as_slice(), allowed, low, gap, out)
    }
}
