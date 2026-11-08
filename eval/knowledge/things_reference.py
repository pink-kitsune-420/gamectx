"""Independent original world things reference: materials, thing kinds, and thing instances.
Python standard library only; nothing here calls a model.

Materials define base matter types. Kinds define categories, salvage yields (materials),
functions with power levels, base value, weight, durability, and stackability.
Containers have capacity and accepted tags; carriers have load capacity and seats.
Instances have condition, owner, quantity and sit in a place, container, holder or carrier.
"""
import copy, json, re

MAX_MATERIALS, MAX_KINDS, MAX_THINGS, MAX_OPS = 256, 512, 4096, 64
MAX_SAFE = 9007199254740991

CATEGORIES = [
    'tool', 'weapon', 'furniture', 'container', 'food',
    'material', 'clothing', 'vehicle', 'structure', 'plant', 'other'
]

LOC_KEYS = ['place', 'container', 'holder', 'carrier']


def enc(v): return json.dumps(v, ensure_ascii=False, separators=(',', ':'))
def control(s): return any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in s)
def text(s, chars):
    assert isinstance(s, str) and 1 <= len(s) <= chars and not control(s); return s
def ident(s): assert isinstance(s, str) and re.fullmatch('[A-Za-z0-9_:-]{1,64}', s); return s
def uint(n, lo=0, hi=MAX_SAFE): assert type(n) is int and lo <= n <= hi; return n
def boolean(b): assert type(b) is bool; return b
def keys(v, required, optional=()): assert isinstance(v, dict) and set(required) <= set(v) <= set(required) | set(optional)


def validate_material(m):
    keys(m, ['id', 'label'], ['unit_weight', 'unit_value'])
    ident(m['id']); text(m['label'], 48)
    if 'unit_weight' in m: uint(m['unit_weight'], 0, 1000000000)
    if 'unit_value' in m: uint(m['unit_value'], 0, 1000000000)


def validate_kind(k, material_ids):
    keys(k, ['id', 'label', 'category', 'materials', 'functions', 'weight', 'base_value', 'durability', 'stackable'],
         ['capacity', 'accepted_tags', 'load_capacity', 'seats', 'tags'])
    ident(k['id']); text(k['label'], 48)
    assert k['category'] in CATEGORIES
    assert isinstance(k['materials'], list) and len(k['materials']) <= 8
    m_seen = set()
    for ym in k['materials']:
        keys(ym, ['material', 'amount'])
        ident(ym['material']); assert ym['material'] in material_ids and ym['material'] not in m_seen
        m_seen.add(ym['material'])
        uint(ym['amount'], 1, 1000000000)
    assert isinstance(k['functions'], list) and len(k['functions']) <= 16
    f_seen = set()
    for fn in k['functions']:
        keys(fn, ['function', 'power'])
        ident(fn['function']); assert fn['function'] not in f_seen
        f_seen.add(fn['function'])
        uint(fn['power'], 1, 1000000000)
    uint(k['weight'], 0, 1000000000)
    uint(k['base_value'], 0, 1000000000)
    uint(k['durability'], 0, 1000000000)
    boolean(k['stackable'])
    if 'capacity' in k: uint(k['capacity'], 0, 1000000000)
    if 'accepted_tags' in k:
        assert isinstance(k['accepted_tags'], list) and len(k['accepted_tags']) <= 8
        for t in k['accepted_tags']: ident(t)
        assert len(set(k['accepted_tags'])) == len(k['accepted_tags'])
    if 'load_capacity' in k: uint(k['load_capacity'], 0, 1000000000)
    if 'seats' in k: uint(k['seats'], 0, 64)
    if 'tags' in k:
        assert isinstance(k['tags'], list) and len(k['tags']) <= 8
        for t in k['tags']: ident(t)
        assert len(set(k['tags'])) == len(k['tags'])


def location_kind(loc):
    present = [k for k in LOC_KEYS if loc.get(k) is not None]
    assert len(present) == 1
    k = present[0]
    ident(loc[k])
    return k, loc[k]


class Things:
    def __init__(self, v=None):
        if v is None:
            v = dict(version=1, materials=[], kinds=[], things=[])
        assert isinstance(v, dict)
        keys(v, ['version'], ['materials', 'kinds', 'things'])
        uint(v['version'], 1, 1)
        self.materials = {}
        for m in v.get('materials', []):
            validate_material(m)
            assert m['id'] not in self.materials
            self.materials[m['id']] = copy.deepcopy(m)
        assert len(self.materials) <= MAX_MATERIALS
        self.kinds = {}
        for k in v.get('kinds', []):
            validate_kind(k, self.materials)
            assert k['id'] not in self.kinds
            self.kinds[k['id']] = copy.deepcopy(k)
        assert len(self.kinds) <= MAX_KINDS
        self.things = {}
        for t in v.get('things', []):
            self._validate_and_add_thing(t)
        assert len(self.things) <= MAX_THINGS
        self._check_all_containers_and_carriers()

    def to_value(self):
        return dict(
            version=1,
            materials=list(self.materials.values()),
            kinds=list(self.kinds.values()),
            things=list(self.things.values())
        )

    def _validate_and_add_thing(self, t):
        keys(t, ['id', 'kind', 'location', 'condition', 'quantity'], ['owner', 'passengers'])
        ident(t['id']); assert t['id'] not in self.things
        assert t['kind'] in self.kinds
        kind_def = self.kinds[t['kind']]
        lk, lv = location_kind(t['location'])
        uint(t['condition'], 0, 100)
        uint(t['quantity'], 1, 1000000000)
        if not kind_def['stackable']:
            assert t['quantity'] == 1
        owner = t.get('owner')
        if owner is not None:
            ident(owner)
        passengers = t.get('passengers', [])
        assert isinstance(passengers, list)
        for p in passengers:
            ident(p)
        assert len(set(passengers)) == len(passengers)
        max_seats = kind_def.get('seats', 0)
        assert len(passengers) <= max_seats
        self.things[t['id']] = dict(
            id=t['id'],
            kind=t['kind'],
            location=copy.deepcopy(t['location']),
            condition=t['condition'],
            owner=owner,
            quantity=t['quantity'],
            passengers=list(passengers)
        )

    def _effective_place(self, thing_id):
        seen = set()
        curr = thing_id
        while True:
            if curr in seen: return None
            seen.add(curr)
            if curr not in self.things: return None
            t = self.things[curr]
            lk, lv = location_kind(t['location'])
            if lk == 'place': return lv
            elif lk == 'container' or lk == 'carrier':
                curr = lv
            elif lk == 'holder':
                return None

    def _total_weight(self, thing_id):
        t = self.things[thing_id]
        kd = self.kinds[t['kind']]
        return kd['weight'] * t['quantity']

    def _check_container_fit(self, container_id, thing_id):
        assert container_id in self.things
        ct = self.things[container_id]
        ck = self.kinds[ct['kind']]
        assert ck['category'] == 'container' or 'capacity' in ck
        # Check cycle
        curr = container_id
        while True:
            if curr == thing_id: assert False, "container cycle"
            if curr not in self.things: break
            lk, lv = location_kind(self.things[curr]['location'])
            if lk == 'container' or lk == 'carrier':
                curr = lv
            else:
                break
        # Check tags
        if 'accepted_tags' in ck and ck['accepted_tags']:
            th = self.things[thing_id]
            tk = self.kinds[th['kind']]
            item_tags = set(tk.get('tags', []))
            assert any(at in item_tags for at in ck['accepted_tags']), "tag not accepted"
        # Check capacity
        if 'capacity' in ck:
            cap = ck['capacity']
            items_inside = [i for i, x in self.things.items() if location_kind(x['location']) == ('container', container_id) and i != thing_id]
            cur_weight = sum(self._total_weight(i) for i in items_inside)
            add_weight = self._total_weight(thing_id)
            assert cur_weight + add_weight <= cap, "exceeds container capacity"

    def _check_carrier_fit(self, carrier_id, thing_id):
        assert carrier_id in self.things
        ct = self.things[carrier_id]
        ck = self.kinds[ct['kind']]
        assert ck['category'] in ['vehicle', 'other'] or 'load_capacity' in ck
        # Check cycle
        curr = carrier_id
        while True:
            if curr == thing_id: assert False, "carrier cycle"
            if curr not in self.things: break
            lk, lv = location_kind(self.things[curr]['location'])
            if lk == 'container' or lk == 'carrier':
                curr = lv
            else:
                break
        if 'load_capacity' in ck:
            cap = ck['load_capacity']
            items_inside = [i for i, x in self.things.items() if location_kind(x['location']) == ('carrier', carrier_id) and i != thing_id]
            cur_weight = sum(self._total_weight(i) for i in items_inside)
            add_weight = self._total_weight(thing_id)
            assert cur_weight + add_weight <= cap, "exceeds carrier capacity"

    def _check_all_containers_and_carriers(self):
        for tid, t in self.things.items():
            lk, lv = location_kind(t['location'])
            if lk == 'container':
                self._check_container_fit(lv, tid)
            elif lk == 'carrier':
                self._check_carrier_fit(lv, tid)

    def op_create(self, at, args):
        keys(args, ['id', 'kind', 'location'], ['condition', 'owner', 'quantity'])
        assert len(self.things) < MAX_THINGS
        tid = ident(args['id'])
        assert tid not in self.things
        assert args['kind'] in self.kinds
        kd = self.kinds[args['kind']]
        lk, lv = location_kind(args['location'])
        cond = uint(args.get('condition', 100), 0, 100)
        qty = uint(args.get('quantity', 1), 1, 1000000000)
        if not kd['stackable']: assert qty == 1
        owner = args.get('owner')
        if owner is not None: ident(owner)
        
        t = dict(id=tid, kind=args['kind'], location=copy.deepcopy(args['location']),
                 condition=cond, owner=owner, quantity=qty, passengers=[])
        self.things[tid] = t
        if lk == 'container': self._check_container_fit(lv, tid)
        elif lk == 'carrier': self._check_carrier_fit(lv, tid)

        place = self._effective_place(tid)
        actors = [owner] if owner else []
        summary = f"Created {kd['label']} ({tid})."
        entry = dict(
            at=at, kind="create_thing", summary=summary,
            actors=actors, place=place, things=[tid],
            tags=["things", "create"], visibility="public",
            witnesses=[], phrases=[kd['label'], tid],
            facts=dict(kind=args['kind'], quantity=qty, condition=cond)
        )
        return dict(id=tid, entry=entry)

    def op_destroy(self, at, args):
        keys(args, ['thing'], ['actor'])
        tid = ident(args['thing'])
        assert tid in self.things
        t = self.things[tid]
        kd = self.kinds[t['kind']]
        actor = args.get('actor')
        if actor is not None: ident(actor)
        place = self._effective_place(tid)
        cond = t['condition']
        qty = t['quantity']
        # Calculate salvage yields: scaled by condition (floor(amount * qty * condition / 100))
        yields = []
        for ym in kd['materials']:
            tot = ym['amount'] * qty
            salvaged = (tot * cond) // 100
            yields.append(dict(material=ym['material'], amount=salvaged))
        
        # Remove item and clear any child references
        del self.things[tid]
        # Any items inside this destroyed container/carrier spill out to place
        for other_id, other_t in list(self.things.items()):
            olk, olv = location_kind(other_t['location'])
            if olk in ['container', 'carrier'] and olv == tid:
                other_t['location'] = dict(place=place if place else "void")

        actors = [actor] if actor else ([t['owner']] if t['owner'] else [])
        summary = f"Destroyed {kd['label']} ({tid})."
        entry = dict(
            at=at, kind="destroy_thing", summary=summary,
            actors=actors, place=place, things=[tid],
            tags=["things", "destroy"], visibility="public",
            witnesses=[], phrases=[kd['label'], tid],
            facts=dict(kind=t['kind'], condition=cond)
        )
        return dict(yields=yields, entry=entry)

    def op_remove(self, at, args):
        keys(args, ['thing'], ['actor'])
        tid = ident(args['thing'])
        assert tid in self.things
        t = self.things[tid]
        kd = self.kinds[t['kind']]
        actor = args.get('actor')
        if actor is not None: ident(actor)
        place = self._effective_place(tid)
        del self.things[tid]
        for other_id, other_t in list(self.things.items()):
            olk, olv = location_kind(other_t['location'])
            if olk in ['container', 'carrier'] and olv == tid:
                other_t['location'] = dict(place=place if place else "void")

        actors = [actor] if actor else ([t['owner']] if t['owner'] else [])
        summary = f"Removed {kd['label']} ({tid})."
        entry = dict(
            at=at, kind="remove_thing", summary=summary,
            actors=actors, place=place, things=[tid],
            tags=["things", "remove"], visibility="public",
            witnesses=[], phrases=[kd['label'], tid],
            facts=dict(kind=t['kind'])
        )
        return dict(entry=entry)

    def op_move(self, at, args):
        keys(args, ['thing', 'to_place'], ['actor'])
        tid = ident(args['thing'])
        assert tid in self.things
        to_place = ident(args['to_place'])
        actor = args.get('actor')
        if actor is not None: ident(actor)
        t = self.things[tid]
        kd = self.kinds[t['kind']]
        t['location'] = dict(place=to_place)
        actors = [actor] if actor else ([t['owner']] if t['owner'] else [])
        summary = f"Moved {kd['label']} ({tid}) to {to_place}."
        entry = dict(
            at=at, kind="move_thing", summary=summary,
            actors=actors, place=to_place, things=[tid],
            tags=["things", "move"], visibility="public",
            witnesses=[], phrases=[kd['label'], tid],
            facts=dict(to_place=to_place)
        )
        return dict(entry=entry)

    def op_store(self, at, args):
        keys(args, ['thing', 'container'], ['actor'])
        tid = ident(args['thing'])
        cid = ident(args['container'])
        assert tid in self.things and cid in self.things
        actor = args.get('actor')
        if actor is not None: ident(actor)
        self._check_container_fit(cid, tid)
        t = self.things[tid]
        kd = self.kinds[t['kind']]
        t['location'] = dict(container=cid)
        place = self._effective_place(cid)
        actors = [actor] if actor else ([t['owner']] if t['owner'] else [])
        summary = f"Stored {kd['label']} ({tid}) in {cid}."
        entry = dict(
            at=at, kind="store_thing", summary=summary,
            actors=actors, place=place, things=[tid, cid],
            tags=["things", "store"], visibility="public",
            witnesses=[], phrases=[kd['label'], tid],
            facts=dict(container=cid)
        )
        return dict(entry=entry)

    def op_take(self, at, args):
        keys(args, ['thing', 'actor'])
        tid = ident(args['thing'])
        actor = ident(args['actor'])
        assert tid in self.things
        t = self.things[tid]
        kd = self.kinds[t['kind']]
        place = self._effective_place(tid)
        t['location'] = dict(holder=actor)
        summary = f"{actor} took {kd['label']} ({tid})."
        entry = dict(
            at=at, kind="take_thing", summary=summary,
            actors=[actor], place=place, things=[tid],
            tags=["things", "take"], visibility="public",
            witnesses=[], phrases=[kd['label'], tid],
            facts=dict(holder=actor)
        )
        return dict(entry=entry)

    def op_load(self, at, args):
        keys(args, ['carrier'], ['thing', 'passenger', 'actor'])
        cid = ident(args['carrier'])
        assert cid in self.things
        car = self.things[cid]
        ck = self.kinds[car['kind']]
        actor = args.get('actor')
        if actor is not None: ident(actor)
        tid = args.get('thing')
        pid = args.get('passenger')
        assert (tid is not None) or (pid is not None)
        things_involved = [cid]
        actors = [actor] if actor else ([car['owner']] if car['owner'] else [])

        if tid is not None:
            tid = ident(tid)
            assert tid in self.things
            self._check_carrier_fit(cid, tid)
            self.things[tid]['location'] = dict(carrier=cid)
            things_involved.append(tid)
        if pid is not None:
            pid = ident(pid)
            max_seats = ck.get('seats', 0)
            assert len(car['passengers']) < max_seats
            assert pid not in car['passengers']
            car['passengers'].append(pid)
            if pid not in actors: actors.append(pid)

        place = self._effective_place(cid)
        summary = f"Loaded into carrier {cid}."
        entry = dict(
            at=at, kind="load_carrier", summary=summary,
            actors=actors, place=place, things=things_involved,
            tags=["things", "load"], visibility="public",
            witnesses=[], phrases=[cid],
            facts=dict(carrier=cid)
        )
        return dict(entry=entry)

    def op_unload(self, at, args):
        keys(args, ['carrier'], ['thing', 'passenger', 'to_place', 'actor'])
        cid = ident(args['carrier'])
        assert cid in self.things
        car = self.things[cid]
        actor = args.get('actor')
        if actor is not None: ident(actor)
        place = args.get('to_place')
        if place is not None: ident(place)
        else: place = self._effective_place(cid)
        assert place is not None

        tid = args.get('thing')
        pid = args.get('passenger')
        assert (tid is not None) or (pid is not None)
        things_involved = [cid]
        actors = [actor] if actor else ([car['owner']] if car['owner'] else [])

        if tid is not None:
            tid = ident(tid)
            assert tid in self.things
            assert location_kind(self.things[tid]['location']) == ('carrier', cid)
            self.things[tid]['location'] = dict(place=place)
            things_involved.append(tid)
        if pid is not None:
            pid = ident(pid)
            assert pid in car['passengers']
            car['passengers'].remove(pid)
            if pid not in actors: actors.append(pid)

        summary = f"Unloaded from carrier {cid}."
        entry = dict(
            at=at, kind="unload_carrier", summary=summary,
            actors=actors, place=place, things=things_involved,
            tags=["things", "unload"], visibility="public",
            witnesses=[], phrases=[cid],
            facts=dict(carrier=cid)
        )
        return dict(entry=entry)

    def op_travel(self, at, args):
        keys(args, ['carrier', 'to'], ['actor'])
        cid = ident(args['carrier'])
        to_place = ident(args['to'])
        actor = args.get('actor')
        if actor is not None: ident(actor)
        assert cid in self.things
        car = self.things[cid]
        ck = self.kinds[car['kind']]
        from_place = self._effective_place(cid)
        car['location'] = dict(place=to_place)
        actors = [actor] if actor else ([car['owner']] if car['owner'] else [])
        for p in car['passengers']:
            if p not in actors: actors.append(p)
        summary = f"Carrier {ck['label']} ({cid}) traveled to {to_place}."
        entry = dict(
            at=at, kind="travel_carrier", summary=summary,
            actors=actors, place=to_place, things=[cid],
            tags=["things", "travel"], visibility="public",
            witnesses=[], phrases=[ck['label'], cid],
            facts=dict(from_place=from_place, to_place=to_place)
        )
        return dict(from_place=from_place, to_place=to_place, entry=entry)

    def op_best_tool(self, args):
        keys(args, ['holder', 'function'])
        holder = ident(args['holder'])
        fn = ident(args['function'])
        best_id, best_kind, best_power, best_cond = None, None, 0, 0
        for tid, t in self.things.items():
            lk, lv = location_kind(t['location'])
            if lk == 'holder' and lv == holder:
                kd = self.kinds[t['kind']]
                for f in kd['functions']:
                    if f['function'] == fn:
                        pwr = f['power']
                        cnd = t['condition']
                        # Compare power desc, condition desc, tid asc
                        if (best_id is None or
                            pwr > best_power or
                            (pwr == best_power and cnd > best_cond) or
                            (pwr == best_power and cnd == best_cond and tid < best_id)):
                            best_id = tid
                            best_kind = t['kind']
                            best_power = pwr
                            best_cond = cnd
        return dict(thing=best_id, kind=best_kind, power=best_power)

    def op_whats_here(self, args):
        keys(args, ['place'])
        place = ident(args['place'])
        here = []
        for tid in sorted(self.things.keys()):
            eff = self._effective_place(tid)
            if eff == place:
                here.append(copy.deepcopy(self.things[tid]))
        return dict(place=place, total=len(here), things=here)

    def workflow(self, req):
        assert isinstance(req, dict)
        keys(req, ['ops'], ['things'])
        ops = req['ops']
        assert isinstance(ops, list) and len(ops) <= MAX_OPS
        results = []
        for o in ops:
            keys(o, ['op'], ['args', 'at'])
            op = o['op']
            args = o.get('args', {})
            at = uint(o.get('at', 0))
            # Atomic: backup state before mutation
            backup = copy.deepcopy(self.things)
            try:
                if op == 'create': res = self.op_create(at, args)
                elif op == 'destroy': res = self.op_destroy(at, args)
                elif op == 'remove': res = self.op_remove(at, args)
                elif op == 'move': res = self.op_move(at, args)
                elif op == 'store': res = self.op_store(at, args)
                elif op == 'take': res = self.op_take(at, args)
                elif op == 'load': res = self.op_load(at, args)
                elif op == 'unload': res = self.op_unload(at, args)
                elif op == 'travel': res = self.op_travel(at, args)
                elif op == 'best_tool': res = self.op_best_tool(args)
                elif op == 'whats_here': res = self.op_whats_here(args)
                else: assert False, f"unknown op {op}"
                results.append(dict(ok=True, result=res))
            except Exception as ex:
                self.things = backup
                results.append(dict(ok=False))
        return dict(results=results, things=self.to_value())


def workflow(a):
    assert isinstance(a, dict)
    keys(a, ['ops'], ['things'])
    th_val = a.get('things')
    inst = Things(copy.deepcopy(th_val)) if th_val is not None else Things()
    return inst.workflow(a)

