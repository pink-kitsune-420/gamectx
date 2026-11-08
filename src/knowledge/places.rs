//! Place kinds, nesting and nearby places over a [`Codex`].
//!
//! A codex place may name its `kind` (a built-in kind such as `workshop`, `city`,
//! `forest`, `house`, `castle` or `cave`, or one from the codex's `place_kinds`), the place
//! it is `within`, `tags`, and `links` to other places with a distance and an optional
//! compass `direction`. Entrances may carry a `distance` (default 1).
//!
//! [`nearby`] answers three questions for a character standing in one place:
//! - `here`: the chain of places they are inside (room → house → village → valley);
//! - `routes`: places they can reach, by shortest distance through open entrances and
//!   links, starting from their place and every place containing it (a link on the village
//!   applies inside the bakery too), with each step of the way;
//! - `around`: places inside the same containers that have no authored route.
//!
//! Ties are broken by codex order, so results are deterministic.
//!
//! ```
//! use gamectx::knowledge::codex::Codex;
//! use gamectx::knowledge::places::nearby;
//! let codex = Codex::parse(r#"{"version":1,"places":[
//!   {"id":"vale","label":"Ember Vale","kind":"valley"},
//!   {"id":"village","label":"Emberford","kind":"village","within":"vale",
//!    "links":[{"place":"woods","distance":5,"direction":"north"}]},
//!   {"id":"bakery","label":"the bakery","kind":"bakery","within":"village"},
//!   {"id":"woods","label":"Whisper Woods","kind":"forest","within":"vale"}]}"#)?;
//! let out = nearby(&codex, &gamectx::obj! {"from"=>"bakery"})?;
//! let woods = &out.get("routes").and_then(|r| r.as_array()).unwrap()[0];
//! assert_eq!(woods.get("place").and_then(|v| v.as_str()), Some("woods"));
//! assert_eq!(woods.get("distance").and_then(|v| v.as_usize()), Some(5));
//! # Ok::<(), gamectx::Error>(())
//! ```
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::sync::OnceLock;

use crate::knowledge::codex::{keys, list, unique_ids, Codex};
use crate::knowledge::props::{ident, line};
use crate::{checked as c, obj, Result, Value};

/// Compass directions a link may name (the other end gets the opposite).
pub const DIRECTIONS: [&str; 10] =
    ["north", "northeast", "east", "southeast", "south", "southwest", "west", "northwest", "up", "down"];
const MAX_DISTANCE: u64 = 1_000_000_000;
const MAX_DEPTH: usize = 8;

fn builtin() -> &'static Value {
    static KINDS: OnceLock<Value> = OnceLock::new();
    KINDS.get_or_init(|| {
        crate::data::json::parse(include_str!("place_kinds.json"), 65536).expect("built-in place kinds")
    })
}
/// The scales a place kind may have, largest first.
pub fn scales() -> &'static [Value] {
    builtin().get("scales").and_then(Value::as_array).unwrap_or(&[])
}
/// Built-in place kinds: `{id, label, scale, indoor}`.
pub fn builtin_kinds() -> &'static [Value] {
    builtin().get("kinds").and_then(Value::as_array).unwrap_or(&[])
}
fn opposite(d: &str) -> &'static str {
    let i = DIRECTIONS.iter().position(|x| *x == d).unwrap_or(0);
    DIRECTIONS[if i >= 8 { 17 - i } else { (i + 4) % 8 }]
}
fn text<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}
fn id_of(v: &Value) -> &str {
    v.get("id").and_then(Value::as_str).unwrap_or("")
}

/// Validate place kinds, `within`, `tags` and `links` (called by [`Codex::from_value`]).
pub(crate) fn validate(v: &Value, places: &[Value]) -> Result<()> {
    let custom = list(v.get("place_kinds"), 0, 64)?;
    let builtin_ids = builtin_kinds().iter().map(id_of).collect::<Vec<_>>();
    for k in custom {
        keys(k, &["id", "label", "scale", "indoor"], &[])?;
        if builtin_ids.contains(&ident(c::get(k, "id")?)?) {
            return Err(c::bad());
        }
        line(c::get(k, "label")?, 48, usize::MAX)?;
        if !scales().contains(c::get(k, "scale")?) {
            return Err(c::bad());
        }
        c::boolean(c::get(k, "indoor")?)?;
    }
    unique_ids(custom)?;
    let ids = places.iter().map(id_of).collect::<BTreeSet<_>>();
    let mut pairs: BTreeSet<(&str, &str)> = BTreeSet::new();
    for p in places {
        let me = id_of(p);
        if let Some(k) = p.get("kind") {
            let k = c::text(k)?;
            if !builtin_ids.contains(&k) && !custom.iter().any(|x| id_of(x) == k) {
                return Err(c::bad());
            }
        }
        if let Some(w) = p.get("within") {
            let w = c::text(w)?;
            if !ids.contains(&w) || w == me {
                return Err(c::bad());
            }
        }
        let tags = list(p.get("tags"), 0, 8)?;
        for (i, t) in tags.iter().enumerate() {
            ident(t)?;
            if tags[..i].contains(t) {
                return Err(c::bad());
            }
        }
        for l in list(p.get("links"), 0, 16)? {
            keys(l, &["place", "distance"], &["direction"])?;
            let other = c::text(c::get(l, "place")?)?;
            if !ids.contains(&other) || other == me {
                return Err(c::bad());
            }
            c::uint(c::get(l, "distance")?, 0, MAX_DISTANCE)?;
            if let Some(d) = l.get("direction") {
                if !DIRECTIONS.contains(&c::text(d)?) {
                    return Err(c::bad());
                }
            }
            let pair = if me < other { (me, other) } else { (other, me) };
            if !pairs.insert(pair) {
                return Err(c::bad());
            }
        }
    }
    let within: BTreeMap<&str, &str> =
        places.iter().filter_map(|p| p.get("within").and_then(Value::as_str).map(|w| (id_of(p), w))).collect();
    let parent = |id: &str| within.get(id).copied();
    for p in places {
        let (mut x, mut depth) = (id_of(p), 0);
        while let Some(up) = parent(x) {
            x = up;
            depth += 1;
            if depth > MAX_DEPTH {
                return Err(c::bad());
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct Edge {
    to: usize,
    cost: u64,
    /// Index into the codex entrances (`None` for a link).
    entrance: Option<usize>,
    direction: Option<&'static str>,
}

/// Static place graph of a codex: entrance and link edges per place (entrances first, in
/// codex order, then links), each place's container and contained places. Closed entrances
/// are skipped per query.
#[derive(Clone, Debug)]
pub(crate) struct Graph {
    adj: Vec<Vec<Edge>>,
    parent: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
}

fn graph(codex: &Codex) -> Graph {
    let places = codex.places();
    let index = |id: &str| codex.position("places", id);
    let mut adj: Vec<Vec<Edge>> = places.iter().map(|_| Vec::new()).collect();
    for (k, e) in codex.entrances().iter().enumerate() {
        let sides = e.get("sides").and_then(Value::as_array).unwrap_or(&[]);
        if sides.len() < 2 {
            continue;
        }
        let side = |i: usize| text(&sides[i], "place").and_then(index);
        let (Some(a), Some(b)) = (side(0), side(1)) else { continue };
        let cost = e.get("distance").and_then(Value::as_usize).unwrap_or(1) as u64;
        adj[a].push(Edge { to: b, cost, entrance: Some(k), direction: None });
        if text(e, "passage") != Some("one_way") {
            adj[b].push(Edge { to: a, cost, entrance: Some(k), direction: None });
        }
    }
    for (a, p) in places.iter().enumerate() {
        for l in p.get("links").and_then(Value::as_array).unwrap_or(&[]) {
            let Some(b) = text(l, "place").and_then(index) else { continue };
            let cost = l.get("distance").and_then(Value::as_usize).unwrap_or(0) as u64;
            let way = text(l, "direction").and_then(|d| DIRECTIONS.iter().find(|x| **x == d).copied());
            adj[a].push(Edge { to: b, cost, entrance: None, direction: way });
            adj[b].push(Edge { to: a, cost, entrance: None, direction: way.map(opposite) });
        }
    }
    let parent: Vec<Option<usize>> = places.iter().map(|p| text(p, "within").and_then(index)).collect();
    let mut children = vec![Vec::new(); places.len()];
    for (i, p) in parent.iter().enumerate() {
        if let Some(p) = p {
            children[*p].push(i);
        }
    }
    Graph { adj, parent, children }
}

/// Where a character in `from` is, what they can reach and what else is around:
/// `{from, closed? ≤256 entrance ids, max_distance? 0–10⁹ = 10⁹, limit? 1–64 = 16}` →
/// `{here, routes, total_routes, around, total_around}`. Work grows with the places reached
/// (the codex's place graph is built once, on the first query).
pub fn nearby(codex: &Codex, q: &Value) -> Result<Value> {
    keys(q, &["from"], &["closed", "max_distance", "limit"])?;
    let places = codex.places();
    let entrances = codex.entrances();
    let start = codex.position("places", ident(c::get(q, "from")?)?).ok_or_else(c::bad)?;
    let closed = list(q.get("closed"), 0, 256)?;
    for (i, x) in closed.iter().enumerate() {
        ident(x)?;
        if closed[..i].contains(x) {
            return Err(c::bad());
        }
    }
    let closed_ids = closed.iter().filter_map(Value::as_str).collect::<BTreeSet<_>>();
    let max = q.get("max_distance").map(|v| c::uint(v, 0, MAX_DISTANCE)).transpose()?.unwrap_or(MAX_DISTANCE);
    let limit = q.get("limit").map(|v| c::uint(v, 1, 64)).transpose()?.unwrap_or(16) as usize;
    let g = codex.graph.get_or_init(|| graph(codex));
    let mut chain = vec![start];
    while let Some(up) = g.parent[chain[chain.len() - 1]] {
        chain.push(up);
    }
    let open = |e: &Edge| e.entrance.is_none_or(|k| !closed_ids.contains(id_of(&entrances[k])));
    // best[i] = (distance, source in chain, steps as (to, entrance, direction)).
    type Step = (usize, Option<usize>, Option<&'static str>);
    let mut best: BTreeMap<usize, (u64, usize, Vec<Step>)> = BTreeMap::new();
    for &x in &chain {
        best.insert(x, (0, x, Vec::new()));
    }
    // Settle places in (distance, index) order; stale heap entries are skipped.
    let mut done = BTreeSet::new();
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = chain.iter().map(|&x| Reverse((0, x))).collect();
    while let Some(Reverse((du, u))) = heap.pop() {
        if done.contains(&u) || best.get(&u).is_none_or(|b| b.0 != du) {
            continue;
        }
        done.insert(u);
        let (src, steps) = best.get(&u).map(|b| (b.1, b.2.clone())).unwrap_or_default();
        for e in g.adj[u].iter().filter(|e| open(e)) {
            let nd = du + e.cost;
            if nd <= max && best.get(&e.to).is_none_or(|b| nd < b.0) {
                let mut s = steps.clone();
                s.push((e.to, e.entrance, e.direction));
                best.insert(e.to, (nd, src, s));
                heap.push(Reverse((nd, e.to)));
            }
        }
    }
    let mut routes = best.keys().copied().filter(|i| !chain.contains(i)).collect::<Vec<_>>();
    routes.sort_by_key(|&i| (best.get(&i).map_or(0, |b| b.0), i));
    let around = chain
        .iter()
        .flat_map(|a| g.children[*a].iter().copied())
        .filter(|i| !chain.contains(i) && !best.contains_key(i))
        .collect::<Vec<_>>();
    let id = |i: usize| Value::str(id_of(&places[i]));
    let label = |i: usize| places[i].get("label").cloned().unwrap_or(Value::Null);
    let kind = |i: usize| places[i].get("kind").cloned().unwrap_or(Value::Null);
    let kinds = builtin_kinds().iter().chain(codex.place_kinds()).collect::<Vec<_>>();
    let scale = |i: usize| {
        let k = text(&places[i], "kind");
        kinds.iter().find(|x| k == Some(id_of(x))).and_then(|x| x.get("scale")).cloned().unwrap_or(Value::Null)
    };
    let here = chain
        .iter()
        .map(|&i| obj! {"place"=>id(i),"label"=>label(i),"kind"=>kind(i),"scale"=>scale(i)})
        .collect::<Vec<_>>();
    let route_rows = routes
        .iter()
        .take(limit)
        .map(|&i| {
            let (d, src, steps) = best.get(&i).cloned().unwrap_or_default();
            let steps = steps
                .into_iter()
                .map(|(to, ent, dir)| obj! {"to"=>id(to),"entrance"=>ent.map_or(Value::Null, |k| Value::str(id_of(&entrances[k]))),"direction"=>dir.map_or(Value::Null, Value::str)})
                .collect::<Vec<_>>();
            obj! {"place"=>id(i),"label"=>label(i),"kind"=>kind(i),"distance"=>d,"from"=>id(src),"steps"=>Value::Array(steps)}
        })
        .collect::<Vec<_>>();
    let around_rows = around
        .iter()
        .take(limit)
        .map(|&i| obj! {"place"=>id(i),"label"=>label(i),"kind"=>kind(i),"within"=>places[i].get("within").cloned().unwrap_or(Value::Null)})
        .collect::<Vec<_>>();
    Ok(obj! {
        "here"=>Value::Array(here),
        "routes"=>Value::Array(route_rows),
        "total_routes"=>routes.len(),
        "around"=>Value::Array(around_rows),
        "total_around"=>around.len(),
    })
}

/// The `places` local kind: `{codex, queries ≤64, limits?}` → `{limits?, results: [{ok, result}|{ok:false}]}`.
/// `limits` are the codex's [host-set limits](crate::knowledge::codex::LIMITS), echoed when supplied.
pub fn workflow(a: &Value) -> Result<Value> {
    let mut limits = crate::knowledge::codex::CodexLimits::default();
    let stripped = crate::character::limits::take(a, &mut limits)?;
    let a = stripped.as_ref().unwrap_or(a);
    keys(a, &["codex", "queries"], &[])?;
    let codex = Codex::from_value_within(c::get(a, "codex")?, &limits)?;
    let queries = list(Some(c::get(a, "queries")?), 0, 64)?;
    let results = queries
        .iter()
        .map(|q| match nearby(&codex, q) {
            Ok(v) => obj! {"ok"=>true,"result"=>v},
            Err(_) => obj! {"ok"=>false},
        })
        .collect::<Vec<_>>();
    Ok(crate::character::limits::echo(obj! {"results"=>Value::Array(results)}, &limits, stripped.is_some()))
}
