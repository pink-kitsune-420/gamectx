"""Independent original fitness reference: strength, endurance, agility, condition,
carry capacity, travel speed (places distances), fatigue, and health/needs interaction.
Python standard library only; nothing here calls a model.
"""
import copy, json, re

MAX_OPS = 64
MAX_SAFE = 9007199254740991

ATTRIBUTES = ['strength', 'endurance', 'agility', 'condition']
TRAIN_ATTRIBUTES = ['strength', 'endurance', 'agility']


def enc(v): return json.dumps(v, ensure_ascii=False, separators=(',', ':'))
def control(s): return any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in s)
def text(s, chars):
    assert isinstance(s, str) and 1 <= len(s) <= chars and not control(s); return s
def ident(s): assert isinstance(s, str) and re.fullmatch('[A-Za-z0-9_:-]{1,64}', s); return s
def uint(n, lo=0, hi=MAX_SAFE): assert type(n) is int and lo <= n <= hi; return n
def boolean(b): assert type(b) is bool; return b
def keys(v, required, optional=()): assert isinstance(v, dict) and set(required) <= set(v) <= set(required) | set(optional)


def validate_fitness(f):
    keys(f, [], ['strength', 'endurance', 'agility', 'condition'])
    for k in ATTRIBUTES:
        if k in f and f[k] is not None:
            uint(f[k], 0, 100)


def normalize_fitness(f):
    if f is None:
        return dict(strength=50, endurance=50, agility=50, condition=100)
    validate_fitness(f)
    return dict(
        strength=f.get('strength', 50),
        endurance=f.get('endurance', 50),
        agility=f.get('agility', 50),
        condition=f.get('condition', 100),
    )


def compute_effective(fit, health_status=None, needs=None):
    s = fit['strength']
    e = fit['endurance']
    a = fit['agility']
    c = fit['condition']

    if health_status == 'hurt':
        s -= 15
        a -= 15
        c -= 20
    elif health_status == 'sick':
        s -= 25
        e -= 30
        a -= 20
        c -= 40
    elif health_status == 'exhausted':
        e -= 40
        a -= 30
        c -= 50

    if needs is not None:
        assert isinstance(needs, dict)
        energy = needs.get('energy', 100)
        nourishment = needs.get('nourishment', 100)
        if energy < 30:
            e -= (30 - energy)
        if nourishment < 30:
            s -= (30 - nourishment)

    return dict(
        strength=max(0, min(100, s)),
        endurance=max(0, min(100, e)),
        agility=max(0, min(100, a)),
        condition=max(0, min(100, c)),
    )


def calc_carry_capacity(eff):
    max_cap = 20 + eff['strength'] + (eff['endurance'] // 2)
    eff_cap = (max_cap * (50 + eff['condition'] // 2)) // 100
    return max(10, eff_cap)


def calc_travel_speed(eff, current_load=0):
    base_speed = 500 + eff['agility'] * 6 + eff['endurance'] * 4
    cap = calc_carry_capacity(eff)
    if current_load > cap:
        overload = current_load - cap
        penalty = (overload * 1000) // cap
        base_speed = max(100, base_speed - penalty)
    return base_speed


def calc_travel_cost(eff, distance, current_load=0):
    speed = calc_travel_speed(eff, current_load)
    if distance > 0:
        time = (distance * 1000) // speed
        if time == 0: time = 1
    else:
        time = 0

    dist_fatigue = (distance * 100) // (eff['endurance'] + eff['condition'] // 2 + 10)
    load_fatigue = (current_load * 50) // (eff['strength'] + 10)
    total_fatigue = min(100, dist_fatigue + load_fatigue)

    return dict(travel_speed=speed, travel_time=time, fatigue_cost=total_fatigue)


def calc_can_carry(eff, current_load, add_weight=0):
    cap = calc_carry_capacity(eff)
    total = current_load + add_weight
    return dict(can_carry=(total <= cap), carry_capacity=cap, current_load=total, remaining=cap - total)


def train_attribute(fit, attribute, effort):
    assert attribute in TRAIN_ATTRIBUTES
    uint(effort, 1, 100)
    cur = fit[attribute]
    if cur >= 100:
        gain = 0
    else:
        gain = (effort * (100 - cur)) // 1000
        if effort >= 10 and cur < 100 and gain == 0:
            gain = 1
    new_level = min(100, cur + gain)
    fatigue = min(100, (effort * 100) // (fit['endurance'] + 10))
    return gain, new_level, fatigue


class FitnessWorld:
    def __init__(self, fitness=None):
        self.fitness = normalize_fitness(fitness)

    def to_value(self):
        return dict(
            strength=self.fitness['strength'],
            endurance=self.fitness['endurance'],
            agility=self.fitness['agility'],
            condition=self.fitness['condition'],
        )

    def op_assess(self, fitness=None, health=None, needs=None, load=0):
        uint(load, 0, 10000)
        fit = normalize_fitness(fitness) if fitness is not None else self.fitness
        h_status = health.get('status') if isinstance(health, dict) else health
        eff = compute_effective(fit, h_status, needs)
        cap = calc_carry_capacity(eff)
        speed = calc_travel_speed(eff, load)
        fatigue_rate = (100 * 100) // (eff['endurance'] + eff['condition'] // 2 + 10)

        return dict(
            ok=True,
            strength=fit['strength'],
            endurance=fit['endurance'],
            agility=fit['agility'],
            condition=fit['condition'],
            effective=dict(
                strength=eff['strength'],
                endurance=eff['endurance'],
                agility=eff['agility'],
                condition=eff['condition'],
            ),
            carry_capacity=cap,
            travel_speed=speed,
            fatigue_rate=fatigue_rate
        )

    def op_travel_cost(self, distance, fitness=None, load=0, health=None, needs=None):
        uint(distance, 0, 1000000)
        uint(load, 0, 10000)
        fit = normalize_fitness(fitness) if fitness is not None else self.fitness
        h_status = health.get('status') if isinstance(health, dict) else health
        eff = compute_effective(fit, h_status, needs)
        res = calc_travel_cost(eff, distance, load)
        return dict(
            ok=True,
            distance=distance,
            current_load=load,
            travel_speed=res['travel_speed'],
            travel_time=res['travel_time'],
            fatigue_cost=res['fatigue_cost'],
        )

    def op_can_carry(self, current_load, add_weight=0, fitness=None, health=None, needs=None):
        uint(current_load, 0, 10000)
        uint(add_weight, 0, 10000)
        fit = normalize_fitness(fitness) if fitness is not None else self.fitness
        h_status = health.get('status') if isinstance(health, dict) else health
        eff = compute_effective(fit, h_status, needs)
        res = calc_can_carry(eff, current_load, add_weight)
        return dict(
            ok=True,
            can_carry=res['can_carry'],
            carry_capacity=res['carry_capacity'],
            current_load=res['current_load'],
            remaining=res['remaining'],
        )

    def op_train(self, at, actor, attribute, effort, fitness=None, place=None):
        ident(actor)
        if place is not None: ident(place)
        fit = normalize_fitness(fitness) if fitness is not None else self.fitness
        prev = fit[attribute]
        gain, new_level, fatigue = train_attribute(fit, attribute, effort)

        # Update fitness
        new_fit = copy.deepcopy(fit)
        new_fit[attribute] = new_level
        if fitness is None:
            self.fitness = new_fit

        entry = dict(
            at=at,
            kind="fitness_training",
            summary=f"{actor} trained {attribute} (+{gain} to {new_level}).",
            actors=[actor],
            place=place or "",
            things=[],
            tags=["fitness", "training", attribute],
            visibility="public",
            witnesses=[],
            phrases=[actor, attribute, "training"],
            facts=dict(
                actor=actor,
                attribute=attribute,
                previous=prev,
                gain=gain,
                new_level=new_level,
                effort=effort,
                fatigue=fatigue
            )
        )

        return dict(
            ok=True,
            actor=actor,
            attribute=attribute,
            previous=prev,
            gain=gain,
            new_level=new_level,
            fatigue=fatigue,
            fitness=dict(
                strength=new_fit['strength'],
                endurance=new_fit['endurance'],
                agility=new_fit['agility'],
                condition=new_fit['condition'],
            ),
            entry=entry
        )

    def workflow(self, req):
        keys(req, ['ops'], ['fitness'])
        ops = req['ops']
        assert isinstance(ops, list) and len(ops) <= MAX_OPS

        if 'fitness' in req:
            self.fitness = normalize_fitness(req['fitness'])

        results = []
        for op_call in ops:
            keys(op_call, ['op'], ['at', 'args'])
            op = op_call['op']
            at = uint(op_call.get('at', 0), 0, MAX_SAFE)
            args = op_call.get('args') or {}
            assert isinstance(args, dict)

            if op == 'assess':
                keys(args, [], ['fitness', 'health', 'needs', 'load'])
                res = self.op_assess(
                    args.get('fitness'),
                    args.get('health'),
                    args.get('needs'),
                    args.get('load', 0)
                )
                results.append(res)
            elif op == 'travel_cost':
                keys(args, ['distance'], ['fitness', 'load', 'health', 'needs'])
                res = self.op_travel_cost(
                    args['distance'],
                    args.get('fitness'),
                    args.get('load', 0),
                    args.get('health'),
                    args.get('needs')
                )
                results.append(res)
            elif op == 'can_carry':
                keys(args, ['current_load'], ['add_weight', 'fitness', 'health', 'needs'])
                res = self.op_can_carry(
                    args['current_load'],
                    args.get('add_weight', 0),
                    args.get('fitness'),
                    args.get('health'),
                    args.get('needs')
                )
                results.append(res)
            elif op == 'train':
                keys(args, ['actor', 'attribute', 'effort'], ['fitness', 'place'])
                res = self.op_train(
                    at,
                    args['actor'],
                    args['attribute'],
                    args['effort'],
                    args.get('fitness'),
                    args.get('place')
                )
                results.append(res)
            else:
                assert False, f"Unknown op: {op}"

        return dict(
            results=results,
            fitness=self.to_value()
        )


def workflow(a):
    keys(a, ['ops'], ['fitness'])
    fw = FitnessWorld(a.get('fitness'))
    return fw.workflow(a)
