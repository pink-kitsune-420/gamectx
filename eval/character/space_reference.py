"""Independent original reference for space inside a place (pass 2, C3).
Python standard library only; nothing here calls a model.

A room is a grid (up to 256 x 256) with wall cells. Beings stand on cells, one being per cell.
`move` walks to a target cell by the shortest 4-neighbour path; walls and other beings block
the way, and the reply gives the number of steps (or refuses when there is no path). Spots (a
well, a counter) have a cell, a capacity and a FIFO queue: `use` takes a free place when the
being is on or next to the spot (8-neighbourhood), otherwise it waits in line; `release` frees
the place and serves the next in line. `see` checks line of sight with an integer Bresenham
line whose cells between the two beings must not be walls, within an optional range.
"""
import pathlib
import sys
from collections import deque
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_uint, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_SIDE = 256
MAX_WALLS = 4096
SPEC = [("rooms", 64, 4096), ("beings", 1024, 1_000_000), ("spots", 16, 1024), ("queue", 32, 4096)]


def line(a: Tuple[int, int], b: Tuple[int, int]) -> List[Tuple[int, int]]:
    """Cells of the integer Bresenham line from a to b, endpoints included."""
    (x0, y0), (x1, y1) = a, b
    dx, dy = abs(x1 - x0), -abs(y1 - y0)
    sx = 1 if x0 < x1 else -1
    sy = 1 if y0 < y1 else -1
    err = dx + dy
    out = []
    while True:
        out.append((x0, y0))
        if (x0, y0) == (x1, y1):
            return out
        e2 = 2 * err
        if e2 >= dy:
            err += dy
            x0 += sx
        if e2 <= dx:
            err += dx
            y0 += sy


class Space:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.rooms: Dict[str, dict] = {}
        self.beings: Dict[str, dict] = {}  # being -> {room, x, y}

    def cell(self, room: dict, a: dict, kx: str = "x", ky: str = "y") -> Tuple[int, int]:
        x, y = uint(a[kx], 0, room["w"] - 1), uint(a[ky], 0, room["h"] - 1)
        return x, y

    def occupied(self, r: str) -> Dict[Tuple[int, int], str]:
        return {(b["x"], b["y"]): k for k, b in self.beings.items() if b["room"] == r}

    def spot_view(self, r: str, s: str) -> dict:
        sp = self.rooms[r]["spots"][s]
        return {"capacity": sp["capacity"], "queue": list(sp["queue"]), "room": r, "spot": s, "users": sorted(sp["users"]),
                "x": sp["x"], "y": sp["y"]}

    def view(self, b: str) -> dict:
        x = self.beings[b]
        return {"being": b, "room": x["room"], "x": x["x"], "y": x["y"]}

    def drop(self, b: str) -> List[dict]:
        """Remove b from every spot of its room; returns beings served from queues, in spot order."""
        r = self.beings[b]["room"]
        served = []
        for s in sorted(self.rooms[r]["spots"]):
            sp = self.rooms[r]["spots"][s]
            if b in sp["queue"]:
                sp["queue"].remove(b)
            if b in sp["users"]:
                sp["users"].remove(b)
                served += self.serve(r, s)
        return served

    def serve(self, r: str, s: str) -> List[dict]:
        sp = self.rooms[r]["spots"][s]
        out = []
        while sp["queue"] and len(sp["users"]) < sp["capacity"]:
            nxt = sp["queue"].pop(0)
            sp["users"].append(nxt)
            out.append({"being": nxt, "spot": s})
        return out

    # ---------------------------------------------------------------- ops

    def op_room(self, a: dict) -> dict:
        keys(a, ["id", "width", "height"], ["walls"])
        r = ident(a["id"])
        w, h = uint(a["width"], 1, MAX_SIDE), uint(a["height"], 1, MAX_SIDE)
        rows = [] if a.get("walls") is None else a["walls"]
        need(isinstance(rows, list) and len(rows) <= MAX_WALLS)
        walls = set()
        for c in rows:
            need(isinstance(c, list) and len(c) == 2)
            walls.add((uint(c[0], 0, w - 1), uint(c[1], 0, h - 1)))
        need(r not in self.rooms and len(self.rooms) < self.lim["rooms"])
        self.rooms[r] = {"w": w, "h": h, "walls": walls, "spots": {}}
        return {"height": h, "id": r, "walls": len(walls), "width": w}

    def op_spot(self, a: dict) -> dict:
        keys(a, ["room", "id", "x", "y"], ["capacity"])
        r, s = ident(a["room"]), ident(a["id"])
        need(r in self.rooms)
        room = self.rooms[r]
        x, y = self.cell(room, a)
        cap = opt_uint(a, "capacity", 1, 1024, 1)
        need(s not in room["spots"] and len(room["spots"]) < self.lim["spots"] and (x, y) not in room["walls"])
        room["spots"][s] = {"x": x, "y": y, "capacity": cap, "users": [], "queue": []}
        return self.spot_view(r, s)

    def op_enter(self, a: dict) -> dict:
        keys(a, ["being", "room", "x", "y"], [])
        b, r = ident(a["being"]), ident(a["room"])
        need(r in self.rooms)
        room = self.rooms[r]
        x, y = self.cell(room, a)
        need((x, y) not in room["walls"] and (x, y) not in self.occupied(r))
        need(b in self.beings or len(self.beings) < self.lim["beings"])
        served = self.drop(b) if b in self.beings else []
        self.beings[b] = {"room": r, "x": x, "y": y}
        return {**self.view(b), "served": served}

    def op_exit(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        need(b in self.beings)
        served = self.drop(b)
        r = self.beings.pop(b)["room"]
        return {"being": b, "room": r, "served": served}

    def op_move(self, a: dict) -> dict:
        keys(a, ["being", "x", "y"], [])
        b = ident(a["being"])
        need(b in self.beings)
        me = self.beings[b]
        room = self.rooms[me["room"]]
        goal = self.cell(room, a)
        occ = self.occupied(me["room"])
        start = (me["x"], me["y"])
        need(goal not in room["walls"] and (goal == start or goal not in occ))
        dist = {start: 0}
        todo = deque([start])
        while todo and goal not in dist:
            cx, cy = todo.popleft()
            for nx, ny in ((cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)):
                if 0 <= nx < room["w"] and 0 <= ny < room["h"] and (nx, ny) not in dist \
                        and (nx, ny) not in room["walls"] and (nx, ny) not in occ:
                    dist[(nx, ny)] = dist[(cx, cy)] + 1
                    todo.append((nx, ny))
        need(goal in dist)
        me["x"], me["y"] = goal
        return {**self.view(b), "steps": dist[goal]}

    def op_use(self, a: dict) -> dict:
        keys(a, ["being", "spot"], [])
        b, s = ident(a["being"]), ident(a["spot"])
        need(b in self.beings)
        me = self.beings[b]
        r = me["room"]
        need(s in self.rooms[r]["spots"])
        sp = self.rooms[r]["spots"][s]
        need(b not in sp["users"] and b not in sp["queue"])
        near = max(abs(me["x"] - sp["x"]), abs(me["y"] - sp["y"])) <= 1
        if near and len(sp["users"]) < sp["capacity"] and not sp["queue"]:
            sp["users"].append(b)
            status = "using"
        else:
            need(len(sp["queue"]) < self.lim["queue"])
            sp["queue"].append(b)
            status = "waiting"
        return {**self.spot_view(r, s), "being": b, "position": None if status == "using" else len(sp["queue"]),
                "status": status}

    def op_release(self, a: dict) -> dict:
        keys(a, ["being", "spot"], [])
        b, s = ident(a["being"]), ident(a["spot"])
        need(b in self.beings)
        r = self.beings[b]["room"]
        need(s in self.rooms[r]["spots"])
        sp = self.rooms[r]["spots"][s]
        need(b in sp["users"] or b in sp["queue"])
        if b in sp["queue"]:
            sp["queue"].remove(b)
            served = []
        else:
            sp["users"].remove(b)
            served = self.serve(r, s)
        return {**self.spot_view(r, s), "served": served}

    def op_see(self, a: dict) -> dict:
        keys(a, ["being", "other"], ["range"])
        b, o = ident(a["being"]), ident(a["other"])
        rng = opt_uint(a, "range", 0, 512, 512)
        need(b != o and b in self.beings and o in self.beings)
        x, y = self.beings[b], self.beings[o]
        same = x["room"] == y["room"]
        dist = max(abs(x["x"] - y["x"]), abs(x["y"] - y["y"])) if same else None
        blocked = None
        if same:
            walls = self.rooms[x["room"]]["walls"]
            for c in line((x["x"], x["y"]), (y["x"], y["y"]))[1:-1]:
                if c in walls:
                    blocked = [c[0], c[1]]
                    break
        visible = same and blocked is None and dist <= rng
        return {"being": b, "blocked": blocked, "distance": dist, "other": o, "same_room": same, "visible": visible}

    def op_crowd(self, a: dict) -> dict:
        keys(a, ["room"], [])
        r = ident(a["room"])
        need(r in self.rooms)
        occ = self.occupied(r)
        room = self.rooms[r]
        free = room["w"] * room["h"] - len(room["walls"]) - len(occ)
        return {"beings": sorted(occ.values()), "free": free, "room": r,
                "spots": [self.spot_view(r, s) for s in sorted(room["spots"])]}

    def workflow(self, request: Any) -> dict:
        table = {"room": self.op_room, "spot": self.op_spot, "enter": self.op_enter, "exit": self.op_exit,
                 "move": self.op_move, "use": self.op_use, "release": self.op_release, "see": self.op_see,
                 "crowd": self.op_crowd}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Space().workflow(request)
