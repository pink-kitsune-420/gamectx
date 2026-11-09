//! Space inside a place: cells, blocking, queues and line of sight (pass 2, C3).
//!
//! A room is a grid (up to 256 × 256) with wall cells. Beings stand on cells, one being per cell.
//! `move` walks to a target cell by the shortest 4-neighbour path; walls and other beings block
//! the way, and the reply gives the number of steps (a target with no path is refused). Spots (a
//! well, a counter) have a cell, a capacity and a first-come queue: `use` takes a free place
//! when the being is on or next to the spot (8-neighbourhood) and nobody is waiting, otherwise
//! the being waits in line; `release` frees the place and serves the next in line. `see` checks
//! line of sight along an integer Bresenham line whose cells between the two beings must not be
//! walls, within an optional range (Chebyshev distance). Capacities are
//! [host-set limits](crate::character::limits). Pure host computation ($0 model spend).
//!
//! ```
//! use gamectx::data::json::parse;
//! use gamectx::character::space::Space;
//!
//! let mut yard = Space::new();
//! let req = parse(r#"{"ops": [
//!   {"op": "room", "args": {"id": "yard", "width": 8, "height": 5, "walls": [[3, 1], [3, 2], [3, 3]]}},
//!   {"op": "spot", "args": {"room": "yard", "id": "well", "x": 6, "y": 2}},
//!   {"op": "enter", "args": {"being": "mira", "room": "yard", "x": 0, "y": 2}},
//!   {"op": "enter", "args": {"being": "tomas", "room": "yard", "x": 6, "y": 1}},
//!   {"op": "use", "args": {"being": "tomas", "spot": "well"}},
//!   {"op": "see", "args": {"being": "mira", "other": "tomas"}},
//!   {"op": "move", "args": {"being": "mira", "x": 5, "y": 2}},
//!   {"op": "use", "args": {"being": "mira", "spot": "well"}}
//! ]}"#, 4096)?;
//! let out = yard.workflow(&req)?.to_json();
//! assert!(out.contains(r#""blocked":[3,1]"#)); // the wall hides tomas
//! assert!(out.contains(r#""steps":9"#)); // the way around the wall
//! assert!(out.contains(r#""queue":["mira"]"#) && out.contains(r#""position":1,"status":"waiting""#));
//! # Ok::<(), gamectx::Error>(())
//! ```

use crate::character::beliefs::{opt, opt_uint, uint_bound};
use crate::character::limits::{self, Limit, Limits};
use crate::checked as c;
use crate::knowledge::codex::keys;
use crate::knowledge::props::ident;
use crate::{obj, Result, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const LIMITS: &[Limit] = &[
    Limit { name: "rooms", default: 64, ceiling: 4096 },
    Limit { name: "beings", default: 1024, ceiling: 1_000_000 },
    Limit { name: "spots", default: 16, ceiling: 1024 },
    Limit { name: "queue", default: 32, ceiling: 4096 },
];
/// Largest room side.
pub const MAX_SIDE: u64 = 256;
/// Most wall cells in one `room` call.
pub const MAX_WALLS: usize = 4096;

type Cell = (i64, i64);

fn need(ok: bool) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(c::bad())
    }
}

fn id_of(args: &Value, k: &str) -> Result<String> {
    Ok(ident(c::get(args, k)?)?.to_string())
}

/// Cells of the integer Bresenham line from `a` to `b`, endpoints included.
pub fn line(a: Cell, b: Cell) -> Vec<Cell> {
    let (mut x0, mut y0) = a;
    let (x1, y1) = b;
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut out = Vec::new();
    loop {
        out.push((x0, y0));
        if (x0, y0) == (x1, y1) {
            return out;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// A spot in a room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spot {
    pub at: Cell,
    pub capacity: u64,
    pub users: Vec<String>,
    pub queue: VecDeque<String>,
}

/// A room: size, walls, spots and who stands where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
    pub width: i64,
    pub height: i64,
    pub walls: BTreeSet<Cell>,
    pub spots: BTreeMap<String, Spot>,
    pub cells: BTreeMap<Cell, String>,
}

/// Where a being stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub room: String,
    pub at: Cell,
}

/// Rooms and the beings in them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Space {
    pub rooms: BTreeMap<String, Room>,
    pub beings: BTreeMap<String, Place>,
    pub limits: Limits,
}

impl Default for Space {
    fn default() -> Self {
        Space { rooms: BTreeMap::new(), beings: BTreeMap::new(), limits: Limits::defaults(LIMITS) }
    }
}

fn names(v: impl IntoIterator<Item = impl AsRef<str>>) -> Value {
    Value::Array(v.into_iter().map(|s| Value::from(s.as_ref())).collect())
}

fn served_view(served: &[(String, String)]) -> Value {
    Value::Array(served.iter().map(|(b, s)| obj! { "being" => b.as_str(), "spot" => s.as_str() }).collect())
}

impl Room {
    fn cell(&self, args: &Value) -> Result<Cell> {
        let x = uint_bound(c::get(args, "x")?, 0, (self.width - 1) as u64)? as i64;
        let y = uint_bound(c::get(args, "y")?, 0, (self.height - 1) as u64)? as i64;
        Ok((x, y))
    }

    /// Moves queued beings into free places; returns `(being, spot)` in order.
    fn serve(&mut self, s: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(sp) = self.spots.get_mut(s) {
            while (sp.users.len() as u64) < sp.capacity {
                let Some(next) = sp.queue.pop_front() else { break };
                sp.users.push(next.clone());
                out.push((next, s.to_string()));
            }
        }
        out
    }

    /// Removes `b` from every spot; returns beings served from queues, in spot order.
    fn drop_being(&mut self, b: &str) -> Vec<(String, String)> {
        let names: Vec<String> = self.spots.keys().cloned().collect();
        let mut served = Vec::new();
        for s in names {
            let sp = self.spots.get_mut(&s).expect("listed spot");
            sp.queue.retain(|x| x != b);
            if let Some(i) = sp.users.iter().position(|x| x == b) {
                sp.users.remove(i);
                served.extend(self.serve(&s));
            }
        }
        served
    }

    fn spot_view(&self, room: &str, s: &str) -> Vec<(String, Value)> {
        let sp = &self.spots[s];
        let mut users = sp.users.clone();
        users.sort();
        vec![
            ("capacity".into(), Value::from(sp.capacity)),
            ("queue".into(), names(&sp.queue)),
            ("room".into(), Value::from(room)),
            ("spot".into(), Value::from(s)),
            ("users".into(), names(&users)),
            ("x".into(), Value::from(sp.at.0)),
            ("y".into(), Value::from(sp.at.1)),
        ]
    }
}

impl Space {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with host limits applied (see [`LIMITS`]).
    pub fn with_limits(v: &Value) -> Result<Self> {
        let mut m = Self::new();
        m.limits.apply(v)?;
        Ok(m)
    }

    pub fn workflow(&mut self, request: &Value) -> Result<Value> {
        let mut lim = self.limits.clone();
        let out = limits::run(request, &mut lim, |op, args, l| match op {
            "room" => self.op_room(args, l),
            "spot" => self.op_spot(args, l),
            "enter" => self.op_enter(args, l),
            "exit" => self.op_exit(args),
            "move" => self.op_move(args),
            "use" => self.op_use(args, l),
            "release" => self.op_release(args),
            "see" => self.op_see(args),
            "crowd" => self.op_crowd(args),
            _ => Err(c::bad()),
        });
        self.limits = lim;
        out
    }

    fn view(&self, b: &str) -> Vec<(String, Value)> {
        let p = &self.beings[b];
        vec![
            ("being".into(), Value::from(b)),
            ("room".into(), Value::from(p.room.as_str())),
            ("x".into(), Value::from(p.at.0)),
            ("y".into(), Value::from(p.at.1)),
        ]
    }

    fn op_room(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["id", "width", "height"], &["walls"])?;
        let id = id_of(args, "id")?;
        let w = uint_bound(c::get(args, "width")?, 1, MAX_SIDE)? as i64;
        let h = uint_bound(c::get(args, "height")?, 1, MAX_SIDE)? as i64;
        let rows = match opt(args, "walls") {
            Some(v) => v.as_array().ok_or_else(c::bad)?,
            None => &[],
        };
        need(rows.len() <= MAX_WALLS)?;
        let mut walls = BTreeSet::new();
        for cell in rows {
            let xy = cell.as_array().filter(|a| a.len() == 2).ok_or_else(c::bad)?;
            walls
                .insert((uint_bound(&xy[0], 0, (w - 1) as u64)? as i64, uint_bound(&xy[1], 0, (h - 1) as u64)? as i64));
        }
        need(!self.rooms.contains_key(&id) && self.rooms.len() < l.get("rooms"))?;
        let count = walls.len() as u64;
        self.rooms
            .insert(id.clone(), Room { width: w, height: h, walls, spots: BTreeMap::new(), cells: BTreeMap::new() });
        Ok(obj! { "height" => h, "id" => id, "walls" => count, "width" => w })
    }

    fn op_spot(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["room", "id", "x", "y"], &["capacity"])?;
        let r = id_of(args, "room")?;
        let s = id_of(args, "id")?;
        let room = self.rooms.get_mut(&r).ok_or_else(c::bad)?;
        let at = room.cell(args)?;
        let capacity = opt_uint(args, "capacity", 1, 1024, 1)?;
        need(!room.spots.contains_key(&s) && room.spots.len() < l.get("spots") && !room.walls.contains(&at))?;
        room.spots.insert(s.clone(), Spot { at, capacity, users: Vec::new(), queue: VecDeque::new() });
        Ok(Value::Object(room.spot_view(&r, &s)))
    }

    fn op_enter(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "room", "x", "y"], &[])?;
        let b = id_of(args, "being")?;
        let r = id_of(args, "room")?;
        let room = self.rooms.get(&r).ok_or_else(c::bad)?;
        let at = room.cell(args)?;
        need(!room.walls.contains(&at) && !room.cells.contains_key(&at))?;
        need(self.beings.contains_key(&b) || self.beings.len() < l.get("beings"))?;
        let served = match self.beings.remove(&b) {
            Some(old) => {
                let from = self.rooms.get_mut(&old.room).ok_or_else(c::bad)?;
                from.cells.remove(&old.at);
                from.drop_being(&b)
            }
            None => Vec::new(),
        };
        self.rooms.get_mut(&r).ok_or_else(c::bad)?.cells.insert(at, b.clone());
        self.beings.insert(b.clone(), Place { room: r, at });
        let mut v = self.view(&b);
        v.push(("served".into(), served_view(&served)));
        Ok(Value::Object(v))
    }

    fn op_exit(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being"], &[])?;
        let b = id_of(args, "being")?;
        let old = self.beings.remove(&b).ok_or_else(c::bad)?;
        let room = self.rooms.get_mut(&old.room).ok_or_else(c::bad)?;
        room.cells.remove(&old.at);
        let served = room.drop_being(&b);
        Ok(obj! { "being" => b, "room" => old.room, "served" => served_view(&served) })
    }

    fn op_move(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "x", "y"], &[])?;
        let b = id_of(args, "being")?;
        let me = self.beings.get(&b).ok_or_else(c::bad)?.clone();
        let room = self.rooms.get_mut(&me.room).ok_or_else(c::bad)?;
        let goal = room.cell(args)?;
        need(!room.walls.contains(&goal) && (goal == me.at || !room.cells.contains_key(&goal)))?;
        let (w, h) = (room.width, room.height);
        let idx = |(x, y): Cell| (y * w + x) as usize;
        let mut dist = vec![u32::MAX; (w * h) as usize];
        dist[idx(me.at)] = 0;
        let mut todo = VecDeque::from([me.at]);
        while let Some((cx, cy)) = todo.pop_front() {
            if dist[idx(goal)] != u32::MAX {
                break;
            }
            for n in [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)] {
                if n.0 >= 0
                    && n.0 < w
                    && n.1 >= 0
                    && n.1 < h
                    && dist[idx(n)] == u32::MAX
                    && !room.walls.contains(&n)
                    && !room.cells.contains_key(&n)
                {
                    dist[idx(n)] = dist[idx((cx, cy))] + 1;
                    todo.push_back(n);
                }
            }
        }
        let steps = dist[idx(goal)];
        need(steps != u32::MAX)?;
        room.cells.remove(&me.at);
        room.cells.insert(goal, b.clone());
        self.beings.get_mut(&b).ok_or_else(c::bad)?.at = goal;
        let mut v = self.view(&b);
        v.push(("steps".into(), Value::from(steps)));
        Ok(Value::Object(v))
    }

    fn op_use(&mut self, args: &Value, l: &Limits) -> Result<Value> {
        keys(args, &["being", "spot"], &[])?;
        let b = id_of(args, "being")?;
        let s = id_of(args, "spot")?;
        let me = self.beings.get(&b).ok_or_else(c::bad)?;
        let r = me.room.clone();
        let at = me.at;
        let room = self.rooms.get_mut(&r).ok_or_else(c::bad)?;
        let sp = room.spots.get_mut(&s).ok_or_else(c::bad)?;
        need(!sp.users.contains(&b) && !sp.queue.contains(&b))?;
        let near = (at.0 - sp.at.0).abs().max((at.1 - sp.at.1).abs()) <= 1;
        let position = if near && (sp.users.len() as u64) < sp.capacity && sp.queue.is_empty() {
            sp.users.push(b.clone());
            Value::Null
        } else {
            need(sp.queue.len() < l.get("queue"))?;
            sp.queue.push_back(b.clone());
            Value::from(sp.queue.len() as u64)
        };
        let status = if position == Value::Null { "using" } else { "waiting" };
        let mut v = room.spot_view(&r, &s);
        v.extend([
            ("being".into(), Value::from(b)),
            ("position".into(), position),
            ("status".into(), Value::from(status)),
        ]);
        Ok(Value::Object(v))
    }

    fn op_release(&mut self, args: &Value) -> Result<Value> {
        keys(args, &["being", "spot"], &[])?;
        let b = id_of(args, "being")?;
        let s = id_of(args, "spot")?;
        let r = self.beings.get(&b).ok_or_else(c::bad)?.room.clone();
        let room = self.rooms.get_mut(&r).ok_or_else(c::bad)?;
        let sp = room.spots.get_mut(&s).ok_or_else(c::bad)?;
        let served = if let Some(i) = sp.queue.iter().position(|x| *x == b) {
            sp.queue.remove(i);
            Vec::new()
        } else {
            let i = sp.users.iter().position(|x| *x == b).ok_or_else(c::bad)?;
            sp.users.remove(i);
            room.serve(&s)
        };
        let mut v = room.spot_view(&r, &s);
        v.push(("served".into(), served_view(&served)));
        Ok(Value::Object(v))
    }

    fn op_see(&self, args: &Value) -> Result<Value> {
        keys(args, &["being", "other"], &["range"])?;
        let b = id_of(args, "being")?;
        let o = id_of(args, "other")?;
        let range = opt_uint(args, "range", 0, 512, 512)? as i64;
        need(b != o)?;
        let x = self.beings.get(&b).ok_or_else(c::bad)?;
        let y = self.beings.get(&o).ok_or_else(c::bad)?;
        let same = x.room == y.room;
        let (mut dist, mut blocked, mut visible) = (Value::Null, Value::Null, false);
        if same {
            let d = (x.at.0 - y.at.0).abs().max((x.at.1 - y.at.1).abs());
            dist = Value::from(d);
            let walls = &self.rooms[&x.room].walls;
            let cells = line(x.at, y.at);
            if let Some(cell) = cells[1..cells.len() - 1].iter().find(|cell| walls.contains(cell)) {
                blocked = Value::Array(vec![Value::from(cell.0), Value::from(cell.1)]);
            }
            visible = blocked == Value::Null && d <= range;
        }
        Ok(obj! {
            "being" => b, "blocked" => blocked, "distance" => dist, "other" => o, "same_room" => same,
            "visible" => visible,
        })
    }

    fn op_crowd(&self, args: &Value) -> Result<Value> {
        keys(args, &["room"], &[])?;
        let r = id_of(args, "room")?;
        let room = self.rooms.get(&r).ok_or_else(c::bad)?;
        let mut here: Vec<&String> = room.cells.values().collect();
        here.sort();
        let free = (room.width * room.height) as u64 - room.walls.len() as u64 - room.cells.len() as u64;
        let spots = room.spots.keys().map(|s| Value::Object(room.spot_view(&r, s))).collect();
        Ok(obj! { "beings" => names(here), "free" => free, "room" => r, "spots" => Value::Array(spots) })
    }
}

pub fn workflow(req: &Value) -> Result<Value> {
    Space::new().workflow(req)
}
