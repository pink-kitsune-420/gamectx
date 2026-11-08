//! Packed game data files and multi-state save files.
//!
//! Two documents share one container shape, so a game ships one organized file of authored
//! definitions and keeps any number of player saves beside it:
//!
//! - [`GameData`] (`"format":"gamectx-data"`): codex, props, lore, cast packs, character
//!   sheets, roleplay engine configs and host data, one section per path.
//! - [`SaveFile`] (`"format":"gamectx-save"`): any number of named [`SaveState`]s (slots,
//!   autosaves, forks). Each state is a complete snapshot of runtime sections (roleplay
//!   engine memory, long-term memory, journals, facts, brains, inventories, host data) and
//!   records its parent, the host's `saved_at` value and its own digest. The save links
//!   to the digest of the game data it was made against.
//!
//! Section paths are 2–4 identifier segments under `world/`, `characters/<id>/` or `host/`;
//! per-character kinds (`sheet`, `engine-config`, `engine`) live at exactly
//! `characters/<id>/<name>`. Sections stay sorted by path, so packing is canonical: the same
//! content always gives the same bytes. Every section is validated by its own parser when
//! it is added and again when a file is loaded.
//!
//! Digests are 64-bit FNV-1a ([`crate::fingerprint`]) of compact JSON. They notice edits,
//! damage and mismatched game data; they are **not** authentication. Saves are player
//! files: treat a loaded save like any other untrusted input (it is fully revalidated).
//!
//! ```
//! use gamectx::character::roleplay::engine::{EngineConfig, RoleplayEngine};
//! use gamectx::kit::save::{GameData, SaveFile, SaveState};
//! use gamectx::Value;
//!
//! let config = r#"{"version":1,"character":{"id":"baker","name":"Mira"}}"#;
//! let mut game = GameData::new("Ember Bakery")?;
//! game.insert("characters/baker/engine", "engine-config", &gamectx::data::json::parse(config, 4096)?)?;
//!
//! let engine = RoleplayEngine::new(EngineConfig::parse(config)?);
//! let mut slot = SaveState::new("slot-1", 40)?;
//! slot.insert("characters/baker/engine", "engine", &engine.checkpoint())?;
//! slot.insert("world/flags", "host", &gamectx::obj! {"ring_lost"=>true})?;
//!
//! let mut save = SaveFile::new(Some(&game));
//! save.put(slot, Some(&game))?;
//! save.fork("slot-1", "before-the-cellar", 41)?;
//!
//! let text = save.to_json();
//! let loaded = SaveFile::parse(&text)?;
//! assert_eq!(loaded.check(&game)?, true); // made against this exact game data
//! let state = loaded.state("before-the-cellar").unwrap();
//! assert_eq!(state.parent(), Some("slot-1"));
//! let restored = state.engine("baker", &game)?;
//! assert_eq!(restored.checkpoint(), engine.checkpoint());
//! # Ok::<(), gamectx::Error>(())
//! ```
use crate::character::cast::CastPack;
use crate::character::roleplay::engine::{EngineConfig, RoleplayEngine};
use crate::character::sheet::CharacterSheet;
use crate::knowledge::codex::{keys, Codex};
use crate::knowledge::props::{ident, line, Props};
use crate::{checked as c, fingerprint, obj, Result, Value};

/// Kinds a [`GameData`] section may hold.
pub const DATA_KINDS: [&str; 7] = ["codex", "props", "sheet", "engine-config", "lore", "cast-pack", "host"];
/// Kinds a [`SaveState`] section may hold.
pub const SAVE_KINDS: [&str; 8] = ["engine", "memory", "journal", "facts", "brain", "inventory", "timeline", "host"];
/// Most sections in one data file or one state.
pub const MAX_SECTIONS: usize = 4096;
/// Most states in one save file.
pub const MAX_STATES: usize = 256;
/// Largest data file or save file accepted by `parse`.
pub const MAX_FILE_BYTES: usize = 67_108_864;
const MAX_SECTION_BYTES: usize = 16_777_216;
const MAX_SAFE: u64 = 9_007_199_254_740_991;
const PER_CHARACTER: [&str; 3] = ["sheet", "engine-config", "engine"];

/// One validated section: a path, a kind, the data and its digest.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    path: String,
    kind: String,
    digest: String,
    data: Value,
}
impl Section {
    fn new(path: &str, kind: &str, data: &Value) -> Self {
        Self { path: path.into(), kind: kind.into(), digest: fingerprint(&data.to_json()), data: data.clone() }
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn kind(&self) -> &str {
        &self.kind
    }
    /// The section digest: FNV-1a of the data's compact JSON.
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn data(&self) -> &Value {
        &self.data
    }
    fn listing(&self) -> Value {
        obj! {"path"=>self.path.as_str(),"kind"=>self.kind.as_str(),"digest"=>self.digest.as_str()}
    }
    fn to_value(&self) -> Value {
        obj! {"path"=>self.path.as_str(),"kind"=>self.kind.as_str(),"digest"=>self.digest.as_str(),"data"=>self.data.clone()}
    }
}

fn path_parts<'a>(path: &'a str, kind: &str) -> Result<Vec<&'a str>> {
    let parts = path.split('/').collect::<Vec<_>>();
    let ok = path.len() <= 256
        && (2..=4).contains(&parts.len())
        && parts.iter().all(|p| ident(&Value::str(*p)).is_ok())
        && ["world", "characters", "host"].contains(&parts[0])
        && (parts[0] != "characters" || parts.len() >= 3)
        && (!PER_CHARACTER.contains(&kind) || parts[0] == "characters" && parts.len() == 3);
    if ok {
        Ok(parts)
    } else {
        Err(c::bad())
    }
}

fn listing(sections: &[Section]) -> Value {
    Value::Array(sections.iter().map(Section::listing).collect())
}

/// Insert or replace a section, keeping the list sorted by path.
fn place(sections: &mut Vec<Section>, s: Section) -> Result<()> {
    match sections.binary_search_by(|x| x.path.as_str().cmp(&s.path)) {
        Ok(i) => sections[i] = s,
        Err(i) => {
            if sections.len() >= MAX_SECTIONS {
                return Err(c::bad());
            }
            sections.insert(i, s)
        }
    }
    Ok(())
}

/// Parse a `sections` list: sorted unique paths, matching digests. Kinds are checked by the caller.
fn read_sections(v: &Value) -> Result<Vec<Section>> {
    let rows = c::list(v)?;
    if rows.len() > MAX_SECTIONS {
        return Err(c::bad());
    }
    let mut out: Vec<Section> = Vec::new();
    for r in rows {
        keys(r, &["path", "kind", "digest", "data"], &[])?;
        let s = Section::new(c::text(c::get(r, "path")?)?, c::text(c::get(r, "kind")?)?, c::get(r, "data")?);
        if c::text(c::get(r, "digest")?)? != s.digest || out.last().is_some_and(|last| last.path >= s.path) {
            return Err(c::bad());
        }
        out.push(s);
    }
    Ok(out)
}

fn title(v: &Value) -> Result<String> {
    Ok(line(v, 120, usize::MAX)?.to_owned())
}

/// Authored game definitions packed into one organized file.
#[derive(Clone, Debug, PartialEq)]
pub struct GameData {
    title: String,
    sections: Vec<Section>,
}
impl GameData {
    /// An empty data file; `title` is 1–120 characters.
    pub fn new(title: &str) -> Result<Self> {
        Ok(Self { title: self::title(&Value::str(title))?, sections: Vec::new() })
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }
    pub fn get(&self, path: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.path == path)
    }
    /// The codex at `world/codex`, or an empty one.
    pub fn codex(&self) -> Result<Codex> {
        match self.sections.iter().find(|s| s.path == "world/codex" && s.kind == "codex") {
            Some(s) => Codex::from_value(&s.data),
            None => Ok(Codex::default()),
        }
    }
    /// The engine config at `characters/<character>/engine`.
    pub fn engine_config(&self, character: &str) -> Result<EngineConfig> {
        self.config_at(&format!("characters/{character}/engine"))
    }
    fn config_at(&self, path: &str) -> Result<EngineConfig> {
        let s = self.get(path).filter(|s| s.kind == "engine-config").ok_or_else(c::bad)?;
        EngineConfig::from_value(&s.data)
    }
    /// The character sheet at `characters/<character>/sheet`, validated against [`GameData::codex`].
    pub fn sheet(&self, character: &str) -> Result<CharacterSheet> {
        let s = self.get(&format!("characters/{character}/sheet")).filter(|s| s.kind == "sheet").ok_or_else(c::bad)?;
        CharacterSheet::from_value(&s.data, &self.codex()?)
    }
    fn check(&self, s: &Section) -> Result<()> {
        crate::packet::request::bounded(&s.data, MAX_SECTION_BYTES)?;
        let parts = path_parts(&s.path, &s.kind)?;
        match s.kind.as_str() {
            "codex" => drop(Codex::from_value(&s.data)?),
            "props" => drop(Props::from_value(&s.data)?),
            "sheet" => drop(CharacterSheet::from_value(&s.data, &self.codex()?)?),
            "engine-config" => {
                if EngineConfig::from_value(&s.data)?.character_id() != parts[1] {
                    return Err(c::bad());
                }
            }
            "lore" => drop(crate::knowledge::lore::Lore::from_value(&s.data)?),
            "cast-pack" => drop(CastPack::from_value(&s.data)?),
            "host" => {}
            _ => return Err(c::bad()),
        }
        Ok(())
    }
    /// Check every section; sheets last so they see this file's codex.
    fn check_all(&self) -> Result<()> {
        for s in
            self.sections.iter().filter(|s| s.kind != "sheet").chain(self.sections.iter().filter(|s| s.kind == "sheet"))
        {
            self.check(s)?;
        }
        Ok(())
    }
    /// Add or replace the section at `path` after validating it as `kind`. Atomic: replacing
    /// the codex re-checks every sheet, and nothing changes on error.
    pub fn insert(&mut self, path: &str, kind: &str, data: &Value) -> Result<()> {
        let mut next = self.clone();
        let s = Section::new(path, kind, data);
        next.check(&s)?;
        place(&mut next.sections, s)?;
        next.check_sheets()?;
        *self = next;
        Ok(())
    }
    /// Remove a section. Fails (and changes nothing) if it is missing or a sheet still needs it.
    pub fn remove(&mut self, path: &str) -> Result<()> {
        let mut next = self.clone();
        next.sections.retain(|s| s.path != path);
        if next.sections.len() == self.sections.len() {
            return Err(c::bad());
        }
        next.check_sheets()?;
        *self = next;
        Ok(())
    }
    fn check_sheets(&self) -> Result<()> {
        self.sections.iter().filter(|s| s.kind == "sheet").try_for_each(|s| self.check(s))
    }
    /// The file digest, over the title and every section's path, kind and digest.
    pub fn digest(&self) -> String {
        fingerprint(
            &obj! {"format"=>"gamectx-data","version"=>1,"title"=>self.title.as_str(),"sections"=>listing(&self.sections)}
                .to_json(),
        )
    }
    pub fn to_value(&self) -> Value {
        obj! {
            "format"=>"gamectx-data","version"=>1,"title"=>self.title.as_str(),
            "sections"=>Value::Array(self.sections.iter().map(Section::to_value).collect()),"digest"=>self.digest()
        }
    }
    /// Compact JSON for writing to disk.
    pub fn to_json(&self) -> String {
        self.to_value().to_json()
    }
    /// Load a data file written by [`GameData::to_json`], revalidating every section and digest.
    pub fn parse(text: &str) -> Result<Self> {
        Self::from_value(&crate::data::json::parse(text, MAX_FILE_BYTES)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        keys(v, &["format", "version", "title", "sections", "digest"], &[])?;
        if c::text(c::get(v, "format")?)? != "gamectx-data" {
            return Err(c::bad());
        }
        c::uint(c::get(v, "version")?, 1, 1)?;
        let out = Self { title: title(c::get(v, "title")?)?, sections: read_sections(c::get(v, "sections")?)? };
        out.check_all()?;
        if c::text(c::get(v, "digest")?)? != out.digest() {
            return Err(c::bad());
        }
        Ok(out)
    }
}

/// One named snapshot of runtime state.
#[derive(Clone, Debug, PartialEq)]
pub struct SaveState {
    name: String,
    label: Option<String>,
    parent: Option<String>,
    saved_at: u64,
    sections: Vec<Section>,
}
impl SaveState {
    /// An empty state. `name` is an identifier; `saved_at` is any host clock value (a game
    /// turn or a timestamp) up to 2^53 − 1.
    pub fn new(name: &str, saved_at: u64) -> Result<Self> {
        let out = Self { name: name.into(), label: None, parent: None, saved_at, sections: Vec::new() };
        out.check_meta()?;
        Ok(out)
    }
    /// A display label of 1–120 characters, such as "Before the cellar".
    pub fn with_label(mut self, label: &str) -> Result<Self> {
        self.label = Some(label.into());
        self.check_meta()?;
        Ok(self)
    }
    /// Record the state this one continues from (any other state name).
    pub fn with_parent(mut self, parent: &str) -> Result<Self> {
        self.parent = Some(parent.into());
        self.check_meta()?;
        Ok(self)
    }
    fn check_meta(&self) -> Result<()> {
        ident(&Value::str(&self.name))?;
        if let Some(l) = &self.label {
            line(&Value::str(l), 120, usize::MAX)?;
        }
        if let Some(p) = &self.parent {
            if ident(&Value::str(p))? == self.name {
                return Err(c::bad());
            }
        }
        if self.saved_at > MAX_SAFE {
            return Err(c::bad());
        }
        Ok(())
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }
    pub fn saved_at(&self) -> u64 {
        self.saved_at
    }
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }
    pub fn get(&self, path: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.path == path)
    }
    fn check(s: &Section) -> Result<()> {
        crate::packet::request::bounded(&s.data, MAX_SECTION_BYTES)?;
        path_parts(&s.path, &s.kind)?;
        let d = &s.data;
        match s.kind.as_str() {
            // Deep validation needs the engine config: see `SaveFile::check` and `SaveState::engine`.
            "engine" => {
                keys(d, &["version", "greetings", "listeners", "pending", "asked"], &[])?;
                c::uint(c::get(d, "version")?, 1, 1)?;
            }
            "memory" => {
                let open =
                    crate::knowledge::memory::Limits { records: usize::MAX, text_chars: usize::MAX, tags: usize::MAX };
                drop(crate::knowledge::memory::Memory::from_value(d, open)?)
            }
            "journal" => drop(crate::knowledge::memory::Journal::restore(d)?),
            "facts" => drop(crate::knowledge::facts::FactRegistry::restore(d)?),
            "brain" => drop(crate::runtime::brain::Brain::restore(d)?),
            "inventory" => drop(crate::knowledge::inventory::Inventory::from_value(d)?),
            "timeline" => drop(crate::knowledge::timeline::Timeline::from_value(d)?),
            "host" => {}
            _ => return Err(c::bad()),
        }
        Ok(())
    }
    /// Add or replace the section at `path` after validating it as `kind`.
    pub fn insert(&mut self, path: &str, kind: &str, data: &Value) -> Result<()> {
        let s = Section::new(path, kind, data);
        Self::check(&s)?;
        place(&mut self.sections, s)
    }
    /// Remove a section; false if there was none.
    pub fn remove(&mut self, path: &str) -> bool {
        let n = self.sections.len();
        self.sections.retain(|s| s.path != path);
        self.sections.len() < n
    }
    /// Store a roleplay engine's checkpoint at `characters/<id>/engine`.
    pub fn put_engine(&mut self, engine: &RoleplayEngine) -> Result<()> {
        self.insert(&format!("characters/{}/engine", engine.config().character_id()), "engine", &engine.checkpoint())
    }
    /// Restore the engine at `characters/<character>/engine` with its config from `game`.
    pub fn engine(&self, character: &str, game: &GameData) -> Result<RoleplayEngine> {
        let path = format!("characters/{character}/engine");
        let s = self.get(&path).filter(|s| s.kind == "engine").ok_or_else(c::bad)?;
        RoleplayEngine::restore(game.config_at(&path)?, &s.data)
    }
    /// Re-validate every engine section against the matching engine config in `game`.
    fn check_engines(&self, game: &GameData) -> Result<()> {
        for s in self.sections.iter().filter(|s| s.kind == "engine") {
            RoleplayEngine::restore(game.config_at(&s.path)?, &s.data)?;
        }
        Ok(())
    }
    /// The state digest, over name, label, parent, saved_at and every section listing.
    pub fn digest(&self) -> String {
        fingerprint(&self.head().to_json())
    }
    fn head(&self) -> Value {
        obj! {"name"=>self.name.as_str(),"label"=>self.label.as_deref(),"parent"=>self.parent.as_deref(),"saved_at"=>self.saved_at,"sections"=>listing(&self.sections)}
    }
    fn manifest(&self) -> Value {
        obj! {"name"=>self.name.as_str(),"parent"=>self.parent.as_deref(),"saved_at"=>self.saved_at,"digest"=>self.digest(),"sections"=>listing(&self.sections)}
    }
    fn to_value(&self) -> Value {
        obj! {
            "name"=>self.name.as_str(),"label"=>self.label.as_deref(),"parent"=>self.parent.as_deref(),"saved_at"=>self.saved_at,
            "sections"=>Value::Array(self.sections.iter().map(Section::to_value).collect()),"digest"=>self.digest()
        }
    }
    fn from_value(v: &Value) -> Result<Self> {
        keys(v, &["name", "label", "parent", "saved_at", "sections", "digest"], &[])?;
        let text = |k: &str| -> Result<Option<String>> {
            Ok(match c::get(v, k)? {
                Value::Null => None,
                x => Some(c::text(x)?.to_owned()),
            })
        };
        let out = Self {
            name: c::text(c::get(v, "name")?)?.to_owned(),
            label: text("label")?,
            parent: text("parent")?,
            saved_at: c::uint(c::get(v, "saved_at")?, 0, MAX_SAFE)?,
            sections: read_sections(c::get(v, "sections")?)?,
        };
        out.check_meta()?;
        out.sections.iter().try_for_each(Self::check)?;
        if c::text(c::get(v, "digest")?)? != out.digest() {
            return Err(c::bad());
        }
        Ok(out)
    }
}

/// Any number of named save states, linked to the game data they were made against.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SaveFile {
    game: Option<(String, String)>,
    states: Vec<SaveState>,
}
impl SaveFile {
    /// An empty save file, linked to `game` when given.
    pub fn new(game: Option<&GameData>) -> Self {
        Self { game: game.map(|g| (g.title.clone(), g.digest())), states: Vec::new() }
    }
    /// Link (or re-link) this save to `game`.
    pub fn link(&mut self, game: &GameData) {
        self.game = Some((game.title.clone(), game.digest()));
    }
    /// The linked game data's title and digest.
    pub fn game(&self) -> Option<(&str, &str)> {
        self.game.as_ref().map(|(t, d)| (t.as_str(), d.as_str()))
    }
    pub fn states(&self) -> &[SaveState] {
        &self.states
    }
    pub fn state(&self, name: &str) -> Option<&SaveState> {
        self.states.iter().find(|s| s.name == name)
    }
    /// Add or replace a state by name. When `game` is given, engine sections must fit its
    /// engine configs. Atomic.
    pub fn put(&mut self, state: SaveState, game: Option<&GameData>) -> Result<()> {
        if let Some(g) = game {
            state.check_engines(g)?;
        }
        match self.states.binary_search_by(|s| s.name.as_str().cmp(&state.name)) {
            Ok(i) => self.states[i] = state,
            Err(i) => {
                if self.states.len() >= MAX_STATES {
                    return Err(c::bad());
                }
                self.states.insert(i, state)
            }
        }
        Ok(())
    }
    /// Copy state `from` to a new state `name` whose parent is `from`; sections are copied
    /// byte for byte. Fails if `from` is missing or `name` is taken.
    pub fn fork(&mut self, from: &str, name: &str, saved_at: u64) -> Result<&SaveState> {
        self.fork_labeled(from, name, saved_at, None)
    }
    fn fork_labeled(&mut self, from: &str, name: &str, saved_at: u64, label: Option<&str>) -> Result<&SaveState> {
        let src = self.state(from).ok_or_else(c::bad)?;
        let st = SaveState {
            name: name.into(),
            label: label.map(str::to_owned),
            parent: Some(from.into()),
            saved_at,
            sections: src.sections.clone(),
        };
        st.check_meta()?;
        if self.state(name).is_some() || self.states.len() >= MAX_STATES {
            return Err(c::bad());
        }
        self.put(st, None)?;
        self.state(name).ok_or_else(c::bad)
    }
    /// Remove a state; false if there was none. Children keep their recorded parent name.
    pub fn remove(&mut self, name: &str) -> bool {
        let n = self.states.len();
        self.states.retain(|s| s.name != name);
        self.states.len() < n
    }
    /// Re-validate every engine section against `game`; `Ok(true)` when this save was made
    /// against exactly this game data, `Ok(false)` when it was made against other (or no
    /// recorded) game data but still fits.
    pub fn check(&self, game: &GameData) -> Result<bool> {
        self.states.iter().try_for_each(|s| s.check_engines(game))?;
        Ok(self.game.as_ref().is_some_and(|(_, d)| *d == game.digest()))
    }
    fn game_value(&self) -> Value {
        match &self.game {
            Some((t, d)) => obj! {"title"=>t.as_str(),"digest"=>d.as_str()},
            None => Value::Null,
        }
    }
    /// The file digest, over the game link and every state's name and digest.
    pub fn digest(&self) -> String {
        let states = self.states.iter().map(|s| obj! {"name"=>s.name.as_str(),"digest"=>s.digest()}).collect();
        fingerprint(
            &obj! {"format"=>"gamectx-save","version"=>1,"game"=>self.game_value(),"states"=>Value::Array(states)}
                .to_json(),
        )
    }
    pub fn to_value(&self) -> Value {
        obj! {
            "format"=>"gamectx-save","version"=>1,"game"=>self.game_value(),
            "states"=>Value::Array(self.states.iter().map(SaveState::to_value).collect()),"digest"=>self.digest()
        }
    }
    /// Compact JSON for writing to disk.
    pub fn to_json(&self) -> String {
        self.to_value().to_json()
    }
    /// Load a save written by [`SaveFile::to_json`], revalidating every state, section and
    /// digest. Call [`SaveFile::check`] with the game data to validate engine memory too.
    pub fn parse(text: &str) -> Result<Self> {
        Self::from_value(&crate::data::json::parse(text, MAX_FILE_BYTES)?)
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        keys(v, &["format", "version", "game", "states", "digest"], &[])?;
        if c::text(c::get(v, "format")?)? != "gamectx-save" {
            return Err(c::bad());
        }
        c::uint(c::get(v, "version")?, 1, 1)?;
        let game = match c::get(v, "game")? {
            Value::Null => None,
            g => {
                keys(g, &["title", "digest"], &[])?;
                let d = c::text(c::get(g, "digest")?)?;
                if d.len() != 16 || !d.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                    return Err(c::bad());
                }
                Some((title(c::get(g, "title")?)?, d.to_owned()))
            }
        };
        let rows = c::list(c::get(v, "states")?)?;
        if rows.len() > MAX_STATES {
            return Err(c::bad());
        }
        let mut states: Vec<SaveState> = Vec::new();
        for r in rows {
            let s = SaveState::from_value(r)?;
            if states.last().is_some_and(|last| last.name >= s.name) {
                return Err(c::bad());
            }
            states.push(s);
        }
        let out = Self { game, states };
        if c::text(c::get(v, "digest")?)? != out.digest() {
            return Err(c::bad());
        }
        Ok(out)
    }
}

/// Conformance workflow: `{ops:[{op, args}]}` over one working data file and save file.
/// Each op yields `{"ok":...}` or `{"error":op}`; a failed op changes nothing.
pub(crate) fn workflow(a: &Value) -> Result<Value> {
    keys(a, &["ops"], &[])?;
    let ops = c::list(c::get(a, "ops")?)?;
    if ops.is_empty() || ops.len() > 32 {
        return Err(c::bad());
    }
    let mut w = (None::<GameData>, SaveFile::default());
    let mut out = Vec::new();
    for o in ops {
        keys(o, &["op", "args"], &[])?;
        let name = c::text(c::get(o, "op")?)?;
        let mut next = w.clone();
        match step(&mut next, name, c::get(o, "args")?) {
            Ok(v) => {
                w = next;
                out.push(obj! {"ok"=>v});
            }
            Err(_) => out.push(obj! {"error"=>name}),
        }
    }
    Ok(obj! {"results"=>Value::Array(out)})
}

fn sections_arg(v: &Value) -> Result<Vec<(&str, &str, &Value)>> {
    let rows = c::list(v)?;
    if rows.len() > MAX_SECTIONS {
        return Err(c::bad());
    }
    let mut out: Vec<(&str, &str, &Value)> = Vec::new();
    for r in rows {
        keys(r, &["path", "kind", "data"], &[])?;
        let path = c::text(c::get(r, "path")?)?;
        if out.iter().any(|(p, _, _)| *p == path) {
            return Err(c::bad());
        }
        out.push((path, c::text(c::get(r, "kind")?)?, c::get(r, "data")?));
    }
    Ok(out)
}

fn opt_text<'a>(a: &'a Value, k: &str) -> Result<Option<&'a str>> {
    match a.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => Ok(Some(c::text(v)?)),
    }
}

fn step(w: &mut (Option<GameData>, SaveFile), op: &str, a: &Value) -> Result<Value> {
    let (data, save) = w;
    match op {
        "pack_data" => {
            keys(a, &["title", "sections"], &[])?;
            let mut g = GameData::new(c::text(c::get(a, "title")?)?)?;
            for (path, kind, d) in sections_arg(c::get(a, "sections")?)? {
                place(&mut g.sections, Section::new(path, kind, d))?;
            }
            g.check_all()?;
            let v = g.to_value();
            *data = Some(g);
            Ok(v)
        }
        "put_data" => {
            keys(a, &["path", "kind", "data"], &[])?;
            let g = data.as_mut().ok_or_else(c::bad)?;
            g.insert(c::text(c::get(a, "path")?)?, c::text(c::get(a, "kind")?)?, c::get(a, "data")?)?;
            Ok(obj! {"digest"=>g.digest()})
        }
        "remove_data" => {
            keys(a, &["path"], &[])?;
            let g = data.as_mut().ok_or_else(c::bad)?;
            g.remove(c::text(c::get(a, "path")?)?)?;
            Ok(obj! {"digest"=>g.digest()})
        }
        "load_data" => {
            let g = GameData::from_value(a)?;
            let v = obj! {"title"=>g.title.as_str(),"digest"=>g.digest(),"sections"=>listing(&g.sections)};
            *data = Some(g);
            Ok(v)
        }
        "link" => {
            keys(a, &[], &[])?;
            save.link(data.as_ref().ok_or_else(c::bad)?);
            Ok(obj! {"game"=>save.game_value()})
        }
        "save" => {
            keys(a, &["name", "saved_at", "sections"], &["label", "parent"])?;
            let mut st = SaveState {
                name: c::text(c::get(a, "name")?)?.to_owned(),
                label: opt_text(a, "label")?.map(str::to_owned),
                parent: opt_text(a, "parent")?.map(str::to_owned),
                saved_at: c::uint(c::get(a, "saved_at")?, 0, MAX_SAFE)?,
                sections: Vec::new(),
            };
            st.check_meta()?;
            for (path, kind, d) in sections_arg(c::get(a, "sections")?)? {
                st.insert(path, kind, d)?;
            }
            let v = st.manifest();
            save.put(st, data.as_ref())?;
            Ok(v)
        }
        "fork" => {
            keys(a, &["from", "name", "saved_at"], &["label"])?;
            let saved_at = c::uint(c::get(a, "saved_at")?, 0, MAX_SAFE)?;
            let st = save.fork_labeled(
                c::text(c::get(a, "from")?)?,
                c::text(c::get(a, "name")?)?,
                saved_at,
                opt_text(a, "label")?,
            )?;
            Ok(st.manifest())
        }
        "remove_state" => {
            keys(a, &["name"], &[])?;
            if !save.remove(c::text(c::get(a, "name")?)?) {
                return Err(c::bad());
            }
            Ok(obj! {"states"=>Value::Array(save.states.iter().map(|s| Value::str(&s.name)).collect())})
        }
        "export_save" => {
            keys(a, &[], &[])?;
            Ok(save.to_value())
        }
        "load_save" => {
            let loaded = SaveFile::from_value(a)?;
            let matches = match data {
                Some(g) => {
                    let same = loaded.check(g)?;
                    if loaded.game.is_some() {
                        Value::Bool(same)
                    } else {
                        Value::Null
                    }
                }
                None => Value::Null,
            };
            let v = obj! {"game_matches"=>matches,"states"=>Value::Array(loaded.states.iter().map(SaveState::manifest).collect())};
            *save = loaded;
            Ok(v)
        }
        _ => Err(c::bad()),
    }
}
