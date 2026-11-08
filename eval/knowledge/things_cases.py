"""World things cases. Expectations were written before running the reference; the
`things` local kind must match the reference byte for byte. Offline only (--cap 0).

  python3 -I eval/knowledge/things_cases.py --cap 0 [--write-conformance] [--random N --seed S]
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
import argparse, copy, random, subprocess
import things_reference as t

ROOT = _pl.Path(__file__).resolve().parents[2]


SAMPLE_MATERIALS = [
    dict(id='wood', label='Timber', unit_weight=2, unit_value=1),
    dict(id='iron', label='Iron Ingot', unit_weight=5, unit_value=10),
    dict(id='cloth', label='Woven Cloth', unit_weight=1, unit_value=3),
    dict(id='leather', label='Tanned Leather', unit_weight=1, unit_value=4),
]

SAMPLE_KINDS = [
    dict(id='wood_axe', label='Woodsman Axe', category='tool',
         materials=[dict(material='wood', amount=2), dict(material='iron', amount=1)],
         functions=[dict(function='chop', power=5), dict(function='cut', power=2)],
         weight=7, base_value=15, durability=100, stackable=False),
    dict(id='iron_sword', label='Iron Shortsword', category='weapon',
         materials=[dict(material='iron', amount=2), dict(material='leather', amount=1)],
         functions=[dict(function='cut', power=8), dict(function='defend', power=4)],
         weight=11, base_value=25, durability=150, stackable=False),
    dict(id='chest', label='Wooden Chest', category='container',
         materials=[dict(material='wood', amount=4), dict(material='iron', amount=1)],
         functions=[dict(function='store', power=10)],
         weight=15, base_value=20, durability=200, stackable=False,
         capacity=50, accepted_tags=['gear', 'valuable']),
    dict(id='cart', label='Handcart', category='vehicle',
         materials=[dict(material='wood', amount=8), dict(material='iron', amount=2)],
         functions=[dict(function='carry', power=20), dict(function='ride', power=2)],
         weight=30, base_value=45, durability=300, stackable=False,
         load_capacity=100, seats=2),
    dict(id='bread', label='Rye Bread', category='food',
         materials=[],
         functions=[dict(function='cook', power=1)],
         weight=1, base_value=2, durability=10, stackable=True, tags=['ration']),
    dict(id='coin', label='Silver Coin', category='material',
         materials=[],
         functions=[],
         weight=0, base_value=1, durability=0, stackable=True, tags=['valuable']),
    dict(id='pouch', label='Coin Pouch', category='container',
         materials=[dict(material='leather', amount=1)],
         functions=[dict(function='store', power=2)],
         weight=1, base_value=5, durability=50, stackable=False,
         capacity=10, accepted_tags=['valuable']),
]

BASE_THINGS = dict(
    version=1,
    materials=SAMPLE_MATERIALS,
    kinds=SAMPLE_KINDS,
    things=[
        dict(id='starter_axe', kind='wood_axe', location=dict(place='forge'), condition=100, owner='smith', quantity=1),
        dict(id='store_chest', kind='chest', location=dict(place='forge'), condition=80, owner='smith', quantity=1),
        dict(id='village_cart', kind='cart', location=dict(place='square'), condition=100, owner=None, quantity=1),
    ]
)


def op(name, **args): return dict(op=name, args=args)
def run(ops, things=None): return dict(things=things if things is not None else copy.deepcopy(BASE_THINGS), ops=ops)
def ok(i, **want):
    def check(out):
        r = out['results'][i]
        return r['ok'] and all(r['result'].get(k) == v for k, v in want.items())
    return check
def failed(i): return lambda out: not out['results'][i]['ok']


CASES = [
    ('create_basic', run([op('create', id='axe2', kind='wood_axe', location=dict(place='woods'))]), ok(0, id='axe2'), False),
    ('create_stackable', run([op('create', id='breads', kind='bread', location=dict(place='bakery'), quantity=5)]), ok(0, id='breads'), False),
    ('create_nonstackable_quantity_fails', run([op('create', id='axe_bad', kind='wood_axe', location=dict(place='forge'), quantity=2)]), failed(0), False),
    ('create_dup_id_fails', run([op('create', id='starter_axe', kind='wood_axe', location=dict(place='forge'))]), failed(0), False),
    ('create_unknown_kind_fails', run([op('create', id='ghost', kind='unknown_kind', location=dict(place='forge'))]), failed(0), False),
    ('destroy_salvage_100_percent', run([
        op('create', id='new_axe', kind='wood_axe', location=dict(place='forge'), condition=100),
        op('destroy', thing='new_axe', actor='smith')
    ]), lambda o: o['results'][1]['ok'] and o['results'][1]['result']['yields'] == [dict(material='wood', amount=2), dict(material='iron', amount=1)], False),
    ('destroy_salvage_scaled', run([
        op('create', id='worn_axe', kind='wood_axe', location=dict(place='forge'), condition=50),
        op('destroy', thing='worn_axe')
    ]), lambda o: o['results'][1]['ok'] and o['results'][1]['result']['yields'] == [dict(material='wood', amount=1), dict(material='iron', amount=0)], False),
    ('remove_no_yield', run([op('remove', thing='starter_axe')]), lambda o: o['results'][0]['ok'] and 'yields' not in o['results'][0]['result'], False),
    ('move_thing', run([op('move', thing='starter_axe', to_place='woods')]), lambda o: o['things']['things'][0]['location'] == dict(place='woods'), False),
    ('take_thing', run([op('take', thing='starter_axe', actor='player_1')]), lambda o: o['things']['things'][0]['location'] == dict(holder='player_1'), False),
    ('store_in_container_ok', run([
        op('create', id='gem1', kind='coin', location=dict(place='forge'), quantity=5),
        op('store', thing='gem1', container='store_chest')
    ]), lambda o: o['results'][1]['ok'] and any(t['id'] == 'gem1' and t['location'] == dict(container='store_chest') for t in o['things']['things']), False),
    ('store_rejected_tag_fails', run([
        op('create', id='snack', kind='bread', location=dict(place='forge'), quantity=1),
        op('store', thing='snack', container='store_chest')
    ]), failed(1), False),
    ('store_cycle_fails', run([
        op('create', id='pouch1', kind='pouch', location=dict(place='forge')),
        op('create', id='pouch2', kind='pouch', location=dict(place='forge')),
        op('store', thing='pouch1', container='pouch2'),
        op('store', thing='pouch2', container='pouch1'),
    ]), failed(3), False),
    ('load_cargo_carrier', run([
        op('load', carrier='village_cart', thing='starter_axe')
    ]), lambda o: o['results'][0]['ok'] and any(t['id'] == 'starter_axe' and t['location'] == dict(carrier='village_cart') for t in o['things']['things']), False),
    ('load_passenger_carrier', run([
        op('load', carrier='village_cart', passenger='traveller')
    ]), lambda o: o['results'][0]['ok'] and any(t['id'] == 'village_cart' and 'traveller' in t['passengers'] for t in o['things']['things']), False),
    ('load_passenger_full_fails', run([
        op('load', carrier='village_cart', passenger='p1'),
        op('load', carrier='village_cart', passenger='p2'),
        op('load', carrier='village_cart', passenger='p3'),
    ]), failed(2), False),
    ('travel_carrier', run([
        op('load', carrier='village_cart', thing='starter_axe', passenger='rider1'),
        op('travel', carrier='village_cart', to='castle')
    ]), lambda o: o['results'][1]['ok'] and any(t['id'] == 'village_cart' and t['location'] == dict(place='castle') for t in o['things']['things']), False),
    ('unload_carrier_cargo', run([
        op('load', carrier='village_cart', thing='starter_axe'),
        op('unload', carrier='village_cart', thing='starter_axe', to_place='square')
    ]), lambda o: o['results'][1]['ok'] and any(t['id'] == 'starter_axe' and t['location'] == dict(place='square') for t in o['things']['things']), False),
    ('best_tool_found', run([
        op('take', thing='starter_axe', actor='player_1'),
        op('best_tool', holder='player_1', function='chop')
    ]), ok(1, thing='starter_axe', power=5), False),
    ('best_tool_none', run([
        op('best_tool', holder='player_1', function='dig')
    ]), ok(0, thing=None, power=0), False),
    ('whats_here_lists_places', run([
        op('whats_here', place='forge')
    ]), lambda o: o['results'][0]['ok'] and o['results'][0]['result']['total'] == 2, False),
    ('atomic_failure_reverts_state', run([
        op('create', id='ghost_good', kind='bread', location=dict(place='forge'), quantity=1),
        op('create', id='ghost_bad', kind='unknown_kind', location=dict(place='forge')),
    ]), lambda o: o['results'][0]['ok'] and not o['results'][1]['ok'] and any(t['id'] == 'ghost_good' for t in o['things']['things']), False),
    ('destroy_spills_contents', run([
        op('create', id='box', kind='chest', location=dict(place='forge')),
        op('create', id='coin1', kind='coin', location=dict(place='forge')),
        op('store', thing='coin1', container='box'),
        op('destroy', thing='box'),
    ]), lambda o: o['results'][3]['ok'] and any(t['id'] == 'coin1' and t['location'] == dict(place='forge') for t in o['things']['things']), False),
    ('destroy_missing_fails', run([op('destroy', thing='nonexistent')]), failed(0), False),
    ('remove_missing_fails', run([op('remove', thing='nonexistent')]), failed(0), False),
    ('move_missing_fails', run([op('move', thing='nonexistent', to_place='woods')]), failed(0), False),
    ('store_missing_container_fails', run([op('store', thing='starter_axe', container='nonexistent')]), failed(0), False),
    ('store_exceed_capacity_fails', run([
        op('create', id='big_axe1', kind='wood_axe', location=dict(place='forge')),
        op('create', id='tiny_pouch', kind='pouch', location=dict(place='forge')),
        op('store', thing='big_axe1', container='tiny_pouch'),
    ]), failed(2), False),
    ('load_missing_carrier_fails', run([op('load', carrier='nonexistent', thing='starter_axe')]), failed(0), False),
    ('load_missing_thing_fails', run([op('load', carrier='village_cart', thing='nonexistent')]), failed(0), False),
    ('unload_missing_passenger_fails', run([op('unload', carrier='village_cart', passenger='ghost_rider')]), failed(0), False),
    ('unload_missing_thing_fails', run([op('unload', carrier='village_cart', thing='starter_axe')]), failed(0), False),
    ('travel_missing_carrier_fails', run([op('travel', carrier='nonexistent', to='woods')]), failed(0), False),
    ('bad_op_unknown', run([dict(op='nonexistent_op')]), failed(0), False),
    ('bad_empty_id_fails', run([op('create', id='', kind='bread', location=dict(place='forge'))]), failed(0), False),
    ('bad_negative_quantity_fails', run([op('create', id='b_neg', kind='bread', location=dict(place='forge'), quantity=-1)]), failed(0), False),
    ('bad_condition_high_fails', run([op('create', id='b_cond', kind='bread', location=dict(place='forge'), condition=150)]), failed(0), False),
    ('bad_location_empty_fails', run([op('create', id='b_loc', kind='bread', location=dict())]), failed(0), False),
    ('bad_location_multi_fails', run([op('create', id='b_loc', kind='bread', location=dict(place='forge', holder='smith'))]), failed(0), False),
    # Top-level error cases
    ('error_unknown_key', dict(things=None, ops=[], extra_unknown=1), None, True),
    ('error_too_many_ops', dict(things=None, ops=[op('whats_here', place='forge') for _ in range(65)]), None, True),
    ('error_bad_version', dict(things=dict(version=2, materials=[], kinds=[], things=[]), ops=[]), None, True),
]


def random_workflow(r):
    n_mat = r.randint(1, 4)
    materials = [
        dict(id=f'm_{i}', label=f'Mat {i}', unit_weight=r.randint(1, 10), unit_value=r.randint(1, 20))
        for i in range(n_mat)
    ]
    mat_ids = [m['id'] for m in materials]
    
    n_kinds = r.randint(2, 6)
    kinds = []
    for i in range(n_kinds):
        cat = r.choice(t.CATEGORIES)
        funcs = [dict(function=f'fn_{j}', power=r.randint(1, 10)) for j in range(r.randint(0, 3))]
        y_mats = [dict(material=r.choice(mat_ids), amount=r.randint(1, 4)) for _ in range(r.randint(0, min(2, len(mat_ids))))]
        seen_ym = set()
        clean_ym = []
        for ym in y_mats:
            if ym['material'] not in seen_ym:
                seen_ym.add(ym['material'])
                clean_ym.append(ym)
        
        stackable = (cat in ['food', 'material']) and r.choice([True, False])
        kd = dict(
            id=f'k_{i}', label=f'Kind {i}', category=cat,
            materials=clean_ym, functions=funcs,
            weight=r.randint(1, 20), base_value=r.randint(1, 50),
            durability=r.randint(10, 200), stackable=stackable
        )
        if cat == 'container':
            kd['capacity'] = r.randint(20, 100)
            kd['accepted_tags'] = ['tag_a', 'tag_b'] if r.choice([True, False]) else []
        elif cat == 'vehicle':
            kd['load_capacity'] = r.randint(50, 200)
            kd['seats'] = r.randint(1, 4)
        if r.choice([True, False]):
            kd['tags'] = ['tag_a']
        kinds.append(kd)

    init_things = dict(version=1, materials=materials, kinds=kinds, things=[])

    ops = []
    places = ['town', 'forest', 'cave', 'castle']
    holders = ['hero', 'companion', 'villager']
    thing_ids = []

    for step in range(r.randint(5, 15)):
        choice = r.choice(['create', 'move', 'take', 'destroy', 'best_tool', 'whats_here'])
        if choice == 'create' and len(thing_ids) < 10:
            tid = f't_{step}'
            kd = r.choice(kinds)
            loc = dict(place=r.choice(places))
            qty = r.randint(1, 5) if kd['stackable'] else 1
            op_dict = op('create', id=tid, kind=kd['id'], location=loc, condition=r.randint(50, 100), quantity=qty)
            ops.append(op_dict)
            thing_ids.append(tid)
        elif choice == 'move' and thing_ids:
            tid = r.choice(thing_ids)
            ops.append(op('move', thing=tid, to_place=r.choice(places)))
        elif choice == 'take' and thing_ids:
            tid = r.choice(thing_ids)
            ops.append(op('take', thing=tid, actor=r.choice(holders)))
        elif choice == 'destroy' and thing_ids:
            tid = r.choice(thing_ids)
            ops.append(op('destroy', thing=tid))
        elif choice == 'best_tool':
            ops.append(op('best_tool', holder=r.choice(holders), function='fn_0'))
        else:
            ops.append(op('whats_here', place=r.choice(places)))

    return dict(things=init_things, ops=ops)


def call(req):
    return subprocess.run([str(ROOT / 'target/debug/gamectx'), 'local', '-'], input=t.enc(req), capture_output=True, text=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--cap', type=float, required=True)
    p.add_argument('--prototype-only', action='store_true')
    p.add_argument('--write-conformance', action='store_true')
    p.add_argument('--random', type=int, default=0)
    p.add_argument('--seed', type=int, default=7)
    a = p.parse_args()

    assert a.cap == 0, 'W3 things eval is $0 offline only'

    if a.random > 0:
        rng = random.Random(a.seed)
        mismatches = 0
        for _ in range(a.random):
            args = random_workflow(rng)
            req = dict(kind='things', args=args)
            got = t.workflow(copy.deepcopy(args))
            if not a.prototype_only:
                x = call(req)
                if x.returncode != 0 or x.stdout.strip() != t.enc(got):
                    mismatches += 1
                    print('MISMATCH', t.enc(args)[:600])
                    print('Ref :', t.enc(got)[:600])
                    print('Rust:', x.stdout.strip()[:600])
                    print('Err :', x.stderr[:600])
                    assert False, "differential fuzzing mismatch"
        print(t.enc(dict(workflows=a.random, mismatches=mismatches, actual_model_calls=0, api_spend_usd=0)))
        return

    for n, args, check, error in CASES:
        try:
            got = t.workflow(copy.deepcopy(args))
            assert not error, n
        except (AssertionError, KeyError, TypeError, ValueError, StopIteration, IndexError):
            assert error, n
            got = None
        if check is not None:
            assert check(got), (n, t.enc(got)[:1500])
        req = dict(kind='things', args=args)
        if not a.prototype_only:
            x = call(req)
            assert (x.returncode != 0) == error, (n, x.stderr)
            if not error:
                assert x.stdout.strip() == t.enc(got), (n, x.stdout[:1500], t.enc(got)[:1500])
        if a.write_conformance:
            dd = ROOT / 'conformance/things' / n
            new = not dd.exists()
            dd.mkdir(parents=True, exist_ok=True)
            for k, v in [('case', dict(op='local', **(dict(error=True) if error else {}))), ('input', req)] + ([] if error else [('expected', got)]):
                f = dd / (k + '.json')
                if new:
                    f.write_text(t.enc(v) + '\n')
                else:
                    assert f.read_text() == t.enc(v) + '\n', f
    print(t.enc(dict(workflows=len(CASES), malformed=sum(e for *_, e in CASES), actual_model_calls=0, api_spend_usd=0)))


if __name__ == '__main__':
    main()
