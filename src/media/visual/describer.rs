//! First-class Visual Describer for AI Image Generation Models.
//! Compiles structured prompts (positive, negative, dimensions, seed, descriptors)
//! for Stable Diffusion, SDXL, Flux, Midjourney, and DALL-E with full customization.

use crate::{
    character::life::{get, text},
    checked as c,
    media::visual::{tokens, ArtBible, VisualSpec},
    obj, Result, Value,
};

/// Target AI image generation model family presets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelTarget {
    Generic,
    StableDiffusion,
    Sdxl,
    Flux,
    Midjourney,
    Dalle3,
}

impl ModelTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::StableDiffusion => "stable_diffusion",
            Self::Sdxl => "sdxl",
            Self::Flux => "flux",
            Self::Midjourney => "midjourney",
            Self::Dalle3 => "dalle3",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "generic" => Some(Self::Generic),
            "sd" | "stable_diffusion" | "stable-diffusion" => Some(Self::StableDiffusion),
            "sdxl" => Some(Self::Sdxl),
            "flux" | "flux1" | "flux_schnell" | "flux_dev" => Some(Self::Flux),
            "midjourney" | "mj" => Some(Self::Midjourney),
            "dalle" | "dalle3" | "dall-e" | "dall-e-3" => Some(Self::Dalle3),
            _ => None,
        }
    }
}

/// Prompt composition mode: comma-separated tags or natural descriptive prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriberMode {
    /// Comma-separated or token-delimited tags (ideal for SD 1.5, SDXL, Flux).
    Tags,
    /// Fluent descriptive prose (ideal for Midjourney, DALL-E 3).
    Prose,
}

impl DescriberMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tags => "tags",
            Self::Prose => "prose",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "tags" | "tag" | "tokens" => Some(Self::Tags),
            "prose" | "natural" | "sentence" => Some(Self::Prose),
            _ => None,
        }
    }
}

/// Customization options for compiling AI image prompts.
#[derive(Clone, Debug)]
pub struct DescriberConfig {
    /// Prompt composition mode (Tags or Prose).
    pub mode: DescriberMode,
    /// Delimiter between positive tokens in Tags mode. Default: ", ".
    pub separator: String,
    /// Delimiter between negative tokens. Default: ", ".
    pub negative_separator: String,
    /// Whether to append the palette colors to the prompt. Default: true.
    pub include_palette: bool,
    /// Prefix before palette colors. Default: "Palette: ".
    pub palette_prefix: String,
    /// Suffix after palette colors. Default: ".".
    pub palette_suffix: String,
    /// Put style tokens first instead of subject/silhouette. Default: false.
    pub style_first: bool,
    /// Extra positive prompt tokens to append.
    pub extra_positive: Vec<String>,
    /// Extra negative prompt tokens to append.
    pub extra_negative: Vec<String>,
    /// Target model family preset.
    pub target: ModelTarget,
    /// Optional prefix before the prompt.
    pub prefix: Option<String>,
    /// Optional suffix after the prompt.
    pub suffix: Option<String>,
}

impl Default for DescriberConfig {
    fn default() -> Self {
        Self {
            mode: DescriberMode::Tags,
            separator: ", ".into(),
            negative_separator: ", ".into(),
            include_palette: true,
            palette_prefix: "Palette: ".into(),
            palette_suffix: ".".into(),
            style_first: false,
            extra_positive: Vec::new(),
            extra_negative: Vec::new(),
            target: ModelTarget::Generic,
            prefix: None,
            suffix: None,
        }
    }
}

impl DescriberConfig {
    /// Preset for Stable Diffusion 1.5 / 2.1.
    pub fn sd15() -> Self {
        Self {
            target: ModelTarget::StableDiffusion,
            extra_negative: vec![
                "blurry".into(),
                "bad anatomy".into(),
                "distorted".into(),
                "extra limbs".into(),
                "watermark".into(),
            ],
            ..Self::default()
        }
    }

    /// Preset for Stable Diffusion XL (SDXL).
    pub fn sdxl() -> Self {
        Self {
            target: ModelTarget::Sdxl,
            extra_negative: vec!["blurry".into(), "bad anatomy".into(), "distorted".into(), "watermark".into()],
            ..Self::default()
        }
    }

    /// Preset for Flux.1 (Schnell / Dev) with clean tag structure and empty negative guidance.
    pub fn flux() -> Self {
        Self { target: ModelTarget::Flux, palette_prefix: "color palette: ".into(), ..Self::default() }
    }

    /// Preset for Midjourney (prose mode, style-first, aspect ratio suffix).
    pub fn midjourney() -> Self {
        Self {
            target: ModelTarget::Midjourney,
            mode: DescriberMode::Prose,
            style_first: true,
            palette_prefix: "Color palette features ".into(),
            palette_suffix: ".".into(),
            suffix: Some("--v 6.1".into()),
            ..Self::default()
        }
    }

    /// Preset for DALL-E 3 (descriptive prose mode, style-first, no negative prompt).
    pub fn dalle() -> Self {
        Self {
            target: ModelTarget::Dalle3,
            mode: DescriberMode::Prose,
            style_first: true,
            palette_prefix: "The palette features ".into(),
            palette_suffix: ".".into(),
            ..Self::default()
        }
    }

    /// Parse configuration from a Value object.
    pub fn from_value(v: &Value) -> Result<Self> {
        let mut cfg = Self::default();
        if let Some(m) = v.get("mode").and_then(Value::as_str) {
            cfg.mode = DescriberMode::parse(m).ok_or_else(c::bad)?;
        }
        if let Some(s) = v.get("separator").and_then(Value::as_str) {
            cfg.separator = s.to_owned();
        }
        if let Some(s) = v.get("negative_separator").and_then(Value::as_str) {
            cfg.negative_separator = s.to_owned();
        }
        if let Some(b) = v.get("include_palette").and_then(Value::as_bool) {
            cfg.include_palette = b;
        }
        if let Some(s) = v.get("palette_prefix").and_then(Value::as_str) {
            cfg.palette_prefix = s.to_owned();
        }
        if let Some(s) = v.get("palette_suffix").and_then(Value::as_str) {
            cfg.palette_suffix = s.to_owned();
        }
        if let Some(b) = v.get("style_first").and_then(Value::as_bool) {
            cfg.style_first = b;
        }
        if let Some(pos) = v.get("extra_positive").and_then(Value::as_array) {
            cfg.extra_positive = pos.iter().filter_map(Value::as_str).map(String::from).collect();
        }
        if let Some(neg) = v.get("extra_negative").and_then(Value::as_array) {
            cfg.extra_negative = neg.iter().filter_map(Value::as_str).map(String::from).collect();
        }
        if let Some(t) = v.get("target").and_then(Value::as_str) {
            cfg.target = ModelTarget::parse(t).ok_or_else(c::bad)?;
        }
        if let Some(s) = v.get("prefix").and_then(Value::as_str) {
            cfg.prefix = Some(s.to_owned());
        }
        if let Some(s) = v.get("suffix").and_then(Value::as_str) {
            cfg.suffix = Some(s.to_owned());
        }
        Ok(cfg)
    }

    pub fn with_mode(mut self, mode: DescriberMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_separator(mut self, s: impl Into<String>) -> Self {
        self.separator = s.into();
        self
    }

    pub fn with_negative_separator(mut self, s: impl Into<String>) -> Self {
        self.negative_separator = s.into();
        self
    }

    pub fn with_include_palette(mut self, include: bool) -> Self {
        self.include_palette = include;
        self
    }

    pub fn with_palette_format(mut self, prefix: impl Into<String>, suffix: impl Into<String>) -> Self {
        self.palette_prefix = prefix.into();
        self.palette_suffix = suffix.into();
        self
    }

    pub fn with_style_first(mut self, style_first: bool) -> Self {
        self.style_first = style_first;
        self
    }

    pub fn with_extra_positive(mut self, tokens: Vec<String>) -> Self {
        self.extra_positive = tokens;
        self
    }

    pub fn with_extra_negative(mut self, tokens: Vec<String>) -> Self {
        self.extra_negative = tokens;
        self
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    pub fn with_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    pub fn for_model(mut self, target: ModelTarget) -> Self {
        self.target = target;
        self
    }
}

/// First-class Visual Describer that compiles visual specs and art bible anchors
/// into structured descriptions and ready-to-run prompts for generative AI models.
#[derive(Clone, Debug)]
pub struct VisualDescriber {
    config: DescriberConfig,
}

impl Default for VisualDescriber {
    fn default() -> Self {
        Self::new(DescriberConfig::default())
    }
}

impl VisualDescriber {
    pub fn new(config: DescriberConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &DescriberConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut DescriberConfig {
        &mut self.config
    }

    /// Compile prompt and structured descriptors from a compiled VisualSpec.
    pub fn describe(&self, spec: &VisualSpec) -> Result<Value> {
        self.describe_with(spec, &self.config)
    }

    /// Compile prompt from a VisualSpec using a specific DescriberConfig.
    pub fn describe_with(&self, spec: &VisualSpec, config: &DescriberConfig) -> Result<Value> {
        let d = &spec.data;
        let entity = text(d, "entity")?;
        let style = text(d, "style")?;
        let format_kind = text(d, "format")?;
        let width = get(d, "width")?.clone();
        let height = get(d, "height")?.clone();
        let aspect = get(d, "aspect")?.clone();
        let seed = get(d, "seed")?.clone();
        let camera_angle = text(d, "camera_angle")?;
        let framing = text(d, "framing")?;
        let silhouette = text(d, "silhouette")?;
        let attire = text(d, "attire")?;

        let palette_tokens = tokens(get(d, "palette")?, 16)?;
        let mut negative_tokens = tokens(get(d, "negative_tokens")?, 32)?;
        for neg in &config.extra_negative {
            if !negative_tokens.contains(neg) {
                negative_tokens.push(neg.clone());
            }
        }

        let expr_tokens = tokens(get(get(d, "expression")?, "tokens")?, 64)?;
        let pose_tokens = tokens(get(get(d, "pose")?, "tokens")?, 64)?;

        let mut env_tokens = Vec::new();
        for cue in c::list(get(d, "environment")?)? {
            env_tokens.extend(tokens(get(cue, "tokens")?, 64)?);
        }

        let visible_props = tokens(get(d, "visible_props")?, 16)?;
        let mut spec_extra = tokens(get(d, "extra_tokens")?, 64)?;
        spec_extra.extend(config.extra_positive.clone());

        let features = d.get("features").cloned().unwrap_or(Value::Null);
        let clothing = d.get("clothing").cloned().unwrap_or(Value::Null);
        let material = d.get("material").and_then(Value::as_str);
        let color = d.get("color").and_then(Value::as_str);
        let texture = d.get("texture").and_then(Value::as_str);
        let mass = d.get("mass").cloned().unwrap_or(Value::Null);
        let state = d.get("state").and_then(Value::as_str);
        let dimensions = d.get("dimensions").cloned().unwrap_or(Value::Null);
        let floor = d.get("floor").and_then(Value::as_str);
        let ceiling = d.get("ceiling").and_then(Value::as_str);
        let exterior = d.get("exterior").and_then(Value::as_bool);
        let tags = d.get("tags").cloned().unwrap_or(Value::Null);
        let attributes = d.get("attributes").cloned().unwrap_or(Value::Null);
        let lighting = d.get("lighting").and_then(Value::as_str);
        let atmosphere = d.get("atmosphere").and_then(Value::as_str);

        let (prompt, negative_prompt) = Self::compile_prompt_strings(
            config,
            style,
            format_kind,
            camera_angle,
            framing,
            silhouette,
            attire,
            &features,
            &clothing,
            material,
            color,
            texture,
            floor,
            ceiling,
            lighting,
            atmosphere,
            &expr_tokens,
            &pose_tokens,
            &env_tokens,
            &visible_props,
            &spec_extra,
            &palette_tokens,
            &negative_tokens,
            aspect.get("width").and_then(Value::as_usize).map(|n| n as u64),
            aspect.get("height").and_then(Value::as_usize).map(|n| n as u64),
        );

        let mut descriptors = obj! {
            "entity" => entity,
            "subject" => silhouette,
            "attire" => attire,
            "style" => style,
            "format" => format_kind,
            "camera" => obj! {
                "angle" => camera_angle,
                "framing" => framing,
            },
            "expression" => expr_tokens.join(", "),
            "pose" => pose_tokens.join(", "),
            "environment" => env_tokens,
            "props" => visible_props,
            "palette" => palette_tokens,
        };

        if features != Value::Null {
            crate::character::archetype::set_field(&mut descriptors, "features", features);
        }
        if clothing != Value::Null {
            crate::character::archetype::set_field(&mut descriptors, "clothing", clothing);
        }
        let mut matter = obj! {};
        if let Some(m) = material {
            crate::character::archetype::set_field(&mut matter, "material", Value::str(m));
        }
        if let Some(c) = color {
            crate::character::archetype::set_field(&mut matter, "color", Value::str(c));
        }
        if let Some(t) = texture {
            crate::character::archetype::set_field(&mut matter, "texture", Value::str(t));
        }
        if mass != Value::Null {
            crate::character::archetype::set_field(&mut matter, "mass", mass);
        }
        if let Some(st) = state {
            crate::character::archetype::set_field(&mut matter, "state", Value::str(st));
        }
        if dimensions != Value::Null {
            crate::character::archetype::set_field(&mut matter, "dimensions", dimensions);
        }
        if matter.as_object().is_some_and(|m| !m.is_empty()) {
            crate::character::archetype::set_field(&mut descriptors, "matter", matter);
        }

        let mut scenery = obj! {};
        if let Some(fl) = floor {
            crate::character::archetype::set_field(&mut scenery, "floor", Value::str(fl));
        }
        if let Some(ce) = ceiling {
            crate::character::archetype::set_field(&mut scenery, "ceiling", Value::str(ce));
        }
        if let Some(ex) = exterior {
            crate::character::archetype::set_field(&mut scenery, "exterior", Value::Bool(ex));
        }
        if scenery.as_object().is_some_and(|s| !s.is_empty()) {
            crate::character::archetype::set_field(&mut descriptors, "scenery", scenery);
        }

        if let Some(l) = lighting {
            crate::character::archetype::set_field(&mut descriptors, "lighting", Value::str(l));
        }
        if let Some(a) = atmosphere {
            crate::character::archetype::set_field(&mut descriptors, "atmosphere", Value::str(a));
        }
        if tags != Value::Null {
            crate::character::archetype::set_field(&mut descriptors, "tags", tags);
        }
        if attributes != Value::Null {
            crate::character::archetype::set_field(&mut descriptors, "attributes", attributes);
        }

        Ok(obj! {
            "prompt" => prompt,
            "negative_prompt" => negative_prompt,
            "width" => width,
            "height" => height,
            "aspect" => aspect,
            "seed" => seed,
            "format" => format_kind,
            "style" => style,
            "palette" => get(d, "palette")?.clone(),
            "target" => config.target.as_str(),
            "descriptors" => descriptors,
        })
    }

    /// Directly compile prompt and descriptors for an anchor in an ArtBible without requiring scene/life runtime.
    pub fn describe_anchor(&self, bible: &ArtBible, entity: &str) -> Result<Value> {
        self.describe_anchor_with(bible, entity, &self.config)
    }

    /// Directly compile prompt and descriptors for an anchor in an ArtBible using a specific DescriberConfig.
    pub fn describe_anchor_with(&self, bible: &ArtBible, entity: &str, config: &DescriberConfig) -> Result<Value> {
        let b = &bible.to_value();
        let anchors = c::list(get(b, "anchors")?)?;
        let anchor =
            anchors.iter().find(|a| a.get("entity").and_then(Value::as_str) == Some(entity)).ok_or_else(c::bad)?;

        let style = text(b, "style")?;
        let format_kind = text(b, "format")?;
        let width = get(b, "width")?.clone();
        let height = get(b, "height")?.clone();
        let aspect = get(b, "aspect")?.clone();
        let camera_angle = text(b, "camera_angle")?;
        let framing = text(b, "framing")?;
        let silhouette = text(anchor, "silhouette")?;
        let attire = text(anchor, "attire")?;
        let seed = get(anchor, "seed")?.clone();

        let palette_tokens = tokens(get(b, "palette")?, 16)?;
        let mut negative_tokens = tokens(get(b, "negative_tokens")?, 32)?;
        for neg in &config.extra_negative {
            if !negative_tokens.contains(neg) {
                negative_tokens.push(neg.clone());
            }
        }

        let props = tokens(get(anchor, "props")?, 16)?;
        let features = anchor.get("features").cloned().unwrap_or(Value::Null);
        let clothing = anchor.get("clothing").cloned().unwrap_or(Value::Null);
        let material = anchor.get("material").and_then(Value::as_str);
        let color = anchor.get("color").and_then(Value::as_str);
        let texture = anchor.get("texture").and_then(Value::as_str);
        let mass = anchor.get("mass").cloned().unwrap_or(Value::Null);
        let state = anchor.get("state").and_then(Value::as_str);
        let dimensions = anchor.get("dimensions").cloned().unwrap_or(Value::Null);
        let floor = anchor.get("floor").and_then(Value::as_str);
        let ceiling = anchor.get("ceiling").and_then(Value::as_str);
        let exterior = anchor.get("exterior").and_then(Value::as_bool);
        let tags = anchor.get("tags").cloned().unwrap_or(Value::Null);
        let attributes = anchor.get("attributes").cloned().unwrap_or(Value::Null);
        let lighting =
            anchor.get("lighting").and_then(Value::as_str).or_else(|| b.get("lighting").and_then(Value::as_str));
        let atmosphere =
            anchor.get("atmosphere").and_then(Value::as_str).or_else(|| b.get("atmosphere").and_then(Value::as_str));

        let empty_expr: Vec<String> = Vec::new();
        let empty_pose: Vec<String> = Vec::new();
        let empty_env: Vec<String> = Vec::new();

        let (prompt, negative_prompt) = Self::compile_prompt_strings(
            config,
            style,
            format_kind,
            camera_angle,
            framing,
            silhouette,
            attire,
            &features,
            &clothing,
            material,
            color,
            texture,
            floor,
            ceiling,
            lighting,
            atmosphere,
            &empty_expr,
            &empty_pose,
            &empty_env,
            &props,
            &config.extra_positive,
            &palette_tokens,
            &negative_tokens,
            aspect.get("width").and_then(Value::as_usize).map(|n| n as u64),
            aspect.get("height").and_then(Value::as_usize).map(|n| n as u64),
        );

        let mut descriptors = obj! {
            "entity" => entity,
            "subject" => silhouette,
            "attire" => attire,
            "style" => style,
            "format" => format_kind,
            "camera" => obj! {
                "angle" => camera_angle,
                "framing" => framing,
            },
            "props" => props,
            "palette" => palette_tokens,
        };

        if features != Value::Null {
            crate::character::archetype::set_field(&mut descriptors, "features", features);
        }
        if clothing != Value::Null {
            crate::character::archetype::set_field(&mut descriptors, "clothing", clothing);
        }
        let mut matter = obj! {};
        if let Some(m) = material {
            crate::character::archetype::set_field(&mut matter, "material", Value::str(m));
        }
        if let Some(c) = color {
            crate::character::archetype::set_field(&mut matter, "color", Value::str(c));
        }
        if let Some(t) = texture {
            crate::character::archetype::set_field(&mut matter, "texture", Value::str(t));
        }
        if mass != Value::Null {
            crate::character::archetype::set_field(&mut matter, "mass", mass);
        }
        if let Some(st) = state {
            crate::character::archetype::set_field(&mut matter, "state", Value::str(st));
        }
        if dimensions != Value::Null {
            crate::character::archetype::set_field(&mut matter, "dimensions", dimensions);
        }
        if matter.as_object().is_some_and(|m| !m.is_empty()) {
            crate::character::archetype::set_field(&mut descriptors, "matter", matter);
        }

        let mut scenery = obj! {};
        if let Some(fl) = floor {
            crate::character::archetype::set_field(&mut scenery, "floor", Value::str(fl));
        }
        if let Some(ce) = ceiling {
            crate::character::archetype::set_field(&mut scenery, "ceiling", Value::str(ce));
        }
        if let Some(ex) = exterior {
            crate::character::archetype::set_field(&mut scenery, "exterior", Value::Bool(ex));
        }
        if scenery.as_object().is_some_and(|s| !s.is_empty()) {
            crate::character::archetype::set_field(&mut descriptors, "scenery", scenery);
        }

        if let Some(l) = lighting {
            crate::character::archetype::set_field(&mut descriptors, "lighting", Value::str(l));
        }
        if let Some(a) = atmosphere {
            crate::character::archetype::set_field(&mut descriptors, "atmosphere", Value::str(a));
        }
        if tags != Value::Null {
            crate::character::archetype::set_field(&mut descriptors, "tags", tags);
        }
        if attributes != Value::Null {
            crate::character::archetype::set_field(&mut descriptors, "attributes", attributes);
        }

        Ok(obj! {
            "prompt" => prompt,
            "negative_prompt" => negative_prompt,
            "width" => width,
            "height" => height,
            "aspect" => aspect,
            "seed" => seed,
            "format" => format_kind,
            "style" => style,
            "palette" => get(b, "palette")?.clone(),
            "target" => config.target.as_str(),
            "descriptors" => descriptors,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn compile_prompt_strings(
        config: &DescriberConfig,
        style: &str,
        format_kind: &str,
        camera_angle: &str,
        framing: &str,
        silhouette: &str,
        attire: &str,
        features: &Value,
        clothing: &Value,
        material: Option<&str>,
        color: Option<&str>,
        texture: Option<&str>,
        floor: Option<&str>,
        ceiling: Option<&str>,
        lighting: Option<&str>,
        atmosphere: Option<&str>,
        expr_tokens: &[String],
        pose_tokens: &[String],
        env_tokens: &[String],
        props: &[String],
        extra_tokens: &[String],
        palette_tokens: &[String],
        negative_tokens: &[String],
        aspect_w: Option<u64>,
        aspect_h: Option<u64>,
    ) -> (String, String) {
        match config.mode {
            DescriberMode::Tags => {
                let mut tokens = Vec::new();
                if config.style_first {
                    tokens.push(style.to_owned());
                    tokens.push(format_kind.to_owned());
                    tokens.push(camera_angle.to_owned());
                    tokens.push(framing.to_owned());
                }
                tokens.push(silhouette.to_owned());
                if let Some(feat) = features.as_object() {
                    for k in ["eyes", "hair", "skin", "build"] {
                        if let Some(v) = feat
                            .iter()
                            .find(|(key, _)| key == k)
                            .and_then(|(_, v)| v.as_str())
                            .filter(|s| !s.is_empty())
                        {
                            tokens.push(if k == "build" { format!("{v} build") } else { format!("{v} {k}") });
                        }
                    }
                    if let Some(scars) = feat.iter().find(|(k, _)| k == "scars").and_then(|(_, v)| v.as_array()) {
                        for s in scars {
                            if let Some(scar) = s.as_str().filter(|x| !x.is_empty()) {
                                tokens.push(scar.to_owned());
                            }
                        }
                    }
                }
                tokens.push(attire.to_owned());
                if let Some(items) = clothing.as_array() {
                    for it in items {
                        if let Some(s) = it.as_str().filter(|x| !x.is_empty()) {
                            tokens.push(s.to_owned());
                        }
                    }
                }
                for prop in props {
                    if !prop.is_empty() {
                        tokens.push(prop.clone());
                    }
                }
                for expr in expr_tokens {
                    tokens.push(expr.clone());
                }
                for pose in pose_tokens {
                    tokens.push(pose.clone());
                }
                for env in env_tokens {
                    tokens.push(env.clone());
                }
                if let Some(c) = color {
                    tokens.push(c.to_owned());
                }
                if let Some(t) = texture {
                    tokens.push(t.to_owned());
                }
                if let Some(m) = material {
                    tokens.push(m.to_owned());
                }
                if let Some(fl) = floor {
                    tokens.push(format!("{fl} floor"));
                }
                if let Some(ce) = ceiling {
                    tokens.push(format!("{ce} ceiling"));
                }
                if let Some(l) = lighting {
                    tokens.push(l.to_owned());
                }
                if let Some(a) = atmosphere {
                    tokens.push(a.to_owned());
                }
                if !config.style_first {
                    tokens.push(style.to_owned());
                    tokens.push(format_kind.to_owned());
                    tokens.push(camera_angle.to_owned());
                    tokens.push(framing.to_owned());
                }
                for ex in extra_tokens {
                    tokens.push(ex.clone());
                }

                let mut prompt = tokens.join(&config.separator);
                if config.include_palette && !palette_tokens.is_empty() {
                    let palette_str = palette_tokens.join(", ");
                    prompt = format!("{prompt}. {}{palette_str}{}", config.palette_prefix, config.palette_suffix);
                }
                if let Some(ref pfx) = config.prefix {
                    prompt = format!("{pfx}{prompt}");
                }
                if let Some(ref sfx) = config.suffix {
                    prompt = format!("{prompt} {sfx}");
                }

                let negative_prompt = if config.target == ModelTarget::Flux && config.extra_negative.is_empty() {
                    String::new()
                } else {
                    negative_tokens.join(&config.negative_separator)
                };

                (prompt, negative_prompt)
            }
            DescriberMode::Prose => {
                let mut sentences = Vec::new();
                let mut intro = format!("{style} of {silhouette}");
                let mut char_details = Vec::new();
                if let Some(feat) = features.as_object() {
                    let mut feat_parts = Vec::new();
                    for k in ["eyes", "hair", "skin"] {
                        if let Some(v) = feat
                            .iter()
                            .find(|(key, _)| key == k)
                            .and_then(|(_, v)| v.as_str())
                            .filter(|s| !s.is_empty())
                        {
                            feat_parts.push(format!("{v} {k}"));
                        }
                    }
                    if !feat_parts.is_empty() {
                        char_details.push(format!("with {}", feat_parts.join(", ")));
                    }
                    if let Some(b) =
                        feat.iter().find(|(k, _)| k == "build").and_then(|(_, v)| v.as_str()).filter(|s| !s.is_empty())
                    {
                        char_details.push(format!("a {b} build"));
                    }
                }
                if !char_details.is_empty() {
                    intro = format!("{intro}, {}", char_details.join(" and "));
                }
                sentences.push(format!("{intro}, wearing {attire}."));

                if let Some(items) = clothing.as_array() {
                    let cloth_list: Vec<&str> =
                        items.iter().filter_map(Value::as_str).filter(|s| !s.is_empty()).collect();
                    if !cloth_list.is_empty() {
                        sentences.push(format!("Wearing: {}.", cloth_list.join(", ")));
                    }
                }

                if !expr_tokens.is_empty() || !pose_tokens.is_empty() {
                    let expr_str = if expr_tokens.is_empty() { "neutral".to_string() } else { expr_tokens.join(", ") };
                    let pose_str = if pose_tokens.is_empty() { "at rest".to_string() } else { pose_tokens.join(", ") };
                    sentences.push(format!("The character has a {expr_str} while {pose_str}."));
                }

                if !env_tokens.is_empty() {
                    sentences.push(format!("Environment details include {}.", env_tokens.join(", ")));
                }

                if !props.is_empty() {
                    sentences.push(format!("Visible items: {}.", props.join(", ")));
                }

                let mut space_parts = Vec::new();
                if let Some(fl) = floor {
                    space_parts.push(format!("{fl} floor"));
                }
                if let Some(ce) = ceiling {
                    space_parts.push(format!("{ce} ceiling"));
                }
                if !space_parts.is_empty() {
                    sentences.push(format!("The space has a {}.", space_parts.join(" and a ")));
                }

                if let Some(l) = lighting {
                    if let Some(a) = atmosphere {
                        sentences.push(format!("Atmosphere is {a} with {l}."));
                    } else {
                        sentences.push(format!("Lighting is {l}."));
                    }
                } else if let Some(a) = atmosphere {
                    sentences.push(format!("Atmosphere is {a}."));
                }

                sentences.push(format!("{framing}, shot from a {camera_angle}."));

                if config.include_palette && !palette_tokens.is_empty() {
                    sentences.push(format!(
                        "{}{}{}",
                        config.palette_prefix,
                        palette_tokens.join(", "),
                        config.palette_suffix
                    ));
                }

                for ex in extra_tokens {
                    sentences.push(ex.clone());
                }

                let mut prompt = sentences.join(" ");
                if let Some(ref pfx) = config.prefix {
                    prompt = format!("{pfx} {prompt}");
                }
                if let Some(ref sfx) = config.suffix {
                    prompt = format!("{prompt} {sfx}");
                } else if config.target == ModelTarget::Midjourney {
                    if let (Some(w), Some(h)) = (aspect_w, aspect_h) {
                        prompt = format!("{prompt} --ar {w}:{h}");
                    }
                }

                let negative_prompt = if config.target == ModelTarget::Dalle3 {
                    String::new()
                } else {
                    negative_tokens.join(&config.negative_separator)
                };

                (prompt, negative_prompt)
            }
        }
    }
}
