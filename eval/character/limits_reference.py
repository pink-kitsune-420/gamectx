"""Pure-Python reference for host-set capacity limits (src/character/limits.rs).

A request may carry `limits` next to `ops`; each named limit is 1..ceiling and only refuses
growth. With `limits` present the output gains `limits` (every name, declaration order).
"""
from typing import Any, Callable, Dict, List, Tuple



def _need(cond: bool) -> None:
    if not cond:
        raise ValueError("invalid limits")


def keys(v: Any, req: List[str], optional: List[str]) -> None:
    _need(isinstance(v, dict) and all(k in v for k in req) and all(k in req or k in optional for k in v))


def uint(v: Any, lo: int, hi: int) -> int:
    _need(isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi)
    return v


class Limits:
    def __init__(self, spec: List[Tuple[str, int, int]]) -> None:
        self.spec = spec
        self.values = {name: default for name, default, _ in spec}

    def __getitem__(self, name: str) -> int:
        return self.values[name]

    def apply(self, v: Any) -> None:
        keys(v, [], [n for n, _, _ in self.spec])
        nxt = dict(self.values)
        for name, _, ceiling in self.spec:
            if v.get(name) is not None:
                nxt[name] = uint(v[name], 1, ceiling)
        self.values = nxt

    def copy_shared(self, other: "Limits") -> None:
        for name in self.values:
            if name in other.values:
                self.values[name] = other.values[name]

    def view(self) -> Dict[str, int]:
        return {n: self.values[n] for n, _, _ in self.spec}


def limited(request: Any, limits: Limits, inner: Callable[[Any], Dict[str, Any]]) -> Dict[str, Any]:
    """Modules with their own op loop: strip and apply `limits`, run, echo."""
    if isinstance(request, dict) and "limits" in request:
        if request["limits"] is not None:
            limits.apply(request["limits"])
        out = inner({k: v for k, v in request.items() if k != "limits"})
        return {"limits": limits.view(), **out}
    return inner(request)
