"""World recipes cases. Expectations were written before running the reference; the
`recipes` local kind must match the reference byte for byte. Offline only (--cap 0).

  python3 -I eval/knowledge/recipes_cases.py --cap 0 [--write-conformance] [--random N --seed S]
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
import argparse, copy, random, subprocess
import recipes_reference as r

ROOT = _pl.Path(__file__).resolve().parents[2]


SAMPLE_MATERIALS = [
    dict(id='wood', label='Timber', unit_weight=2, unit_value=1),
    dict(id='iron', label='Iron Ingot', unit_weight=5, unit_value=10),
    dict(id='stone', label='Cut Stone', unit_weight=10, unit_value=2),
    dict(id='flour', label='Flour', unit_weight=1, unit_value=2),
]

SAMPLE_KINDS = [
    dict(id='wood_log', label='Wood Log', category='material', materials=[], functions=[], weight=4, base_value=2, durability=100, stackable=True, tags=['timber']),
    dict(id='iron_ore', label='Iron Ore', category='material', materials=[], functions=[], weight=6, base_value=5, durability=100, stackable=True, tags=['ore']),
    dict(id='bread_dough', label='Bread Dough', category='food', materials=[], functions=[], weight=1, base_value=1, durability=5, stackable=True, tags=['dough']),
    dict(id='bread', label='Baked Bread', category='food', materials=[], functions=[dict(function='cook', power=1)], weight=1, base_value=3, durability=10, stackable=True, tags=['ration']),
    dict(id='hammer', label='Smith Hammer', category='tool', materials=[], functions=[dict(function='hammer', power=5)], weight=5, base_value=10, durability=100, stackable=False),
    dict(id='chisel', label='Stone Chisel', category='tool', materials=[], functions=[dict(function='cut', power=3)], weight=2, base_value=8, durability=80, stackable=False),
    dict(id='forge_furnace', label='Forge Furnace', category='structure', materials=[], functions=[dict(function='forge', power=10)], weight=100, base_value=50, durability=500, stackable=False),
    dict(id='oven', label='Stone Oven', category='furniture', materials=[], functions=[dict(function='cook_station', power=5)], weight=50, base_value=30, durability=300, stackable=False),
    dict(id='iron_sword', label='Iron Shortsword', category='weapon', materials=[dict(material='iron', amount=2)], functions=[dict(function='cut', power=8)], weight=10, base_value=30, durability=150, stackable=False),
    dict(id='bridge', label='Wooden Footbridge', category='structure', materials=[dict(material='wood', amount=10)], functions=[], weight=200, base_value=100, durability=1000, stackable=False),
]

SAMPLE_RECIPES = [
    dict(
        id='craft_sword',
        label='Forge Iron Sword',
        category='crafting',
        inputs=[dict(kind='iron_ore', quantity=2), dict(tag='timber', quantity=1)],
        tools=[dict(function='hammer', power=3)],
        station='forge',
        skill=dict(id='smithing', level=10),
        outputs=[dict(kind='iron_sword', quantity=1, condition=100)],
        time=20
    ),
    dict(
        id='bake_bread',
        label='Bake Bread',
        category='cooking',
        inputs=[dict(kind='bread_dough', quantity=2)],
        tools=[],
        station='cook_station',
        outputs=[dict(kind='bread', quantity=2, condition=100)],
        time=10
    ),
    dict(
        id='repair_sword',
        label='Repair Iron Sword',
        category='repairing',
        inputs=[dict(kind='iron_ore', quantity=1)],
        tools=[dict(function='hammer', power=4)],
        station='forge',
        skill=dict(id='smithing', level=5),
        outputs=[],
        time=15
    ),
    dict(
        id='dismantle_sword',
        label='Dismantle Sword',
        category='dismantling',
        inputs=[],
        tools=[dict(function='hammer', power=2)],
        station='forge',
        outputs=[dict(kind='iron_ore', quantity=1, condition=80)],
        time=5
    ),
    dict(
        id='build_bridge',
        label='Construct Footbridge',
        category='building',
        inputs=[dict(tag='timber', quantity=4)],
        tools=[dict(function='hammer', power=2)],
        skill=dict(id='carpentry', level=5),
        outputs=[],
        time=50,
        building=dict(
            structure_kind='bridge',
            add_spots=[dict(id='bridge_edge', label='The Bridge Crossing', description='Planks stretching across the brook.')],
            add_link=[dict(place='forest', distance=2, direction='north')],
            patch_place=dict(kind='bridge', add_tags=['accessible'])
        )
    ),
    dict(
        id='clear_mine_entrance',
        label='Clear Mine Entrance',
        category='building',
        inputs=[dict(tag='timber', quantity=2)],
        tools=[dict(function='hammer', power=2)],
        outputs=[],
        time=30,
        building=dict(
            open_entrance='mine_gate'
        )
    ),
]

BASE_THINGS = dict(
    version=1,
    materials=SAMPLE_MATERIALS,
    kinds=SAMPLE_KINDS,
    things=[
        dict(id='smith_hammer', kind='hammer', location=dict(holder='smith'), condition=100, owner='smith', quantity=1, passengers=[]),
        dict(id='forge_station', kind='forge_furnace', location=dict(place='forge'), condition=100, owner='smith', quantity=1, passengers=[]),
        dict(id='bakery_oven', kind='oven', location=dict(place='bakery'), condition=100, owner='baker', quantity=1, passengers=[]),
        dict(id='ores_in_forge', kind='iron_ore', location=dict(place='forge'), condition=100, owner='smith', quantity=5, passengers=[]),
        dict(id='logs_in_forge', kind='wood_log', location=dict(holder='smith'), condition=100, owner='smith', quantity=5, passengers=[]),
        dict(id='dough_in_bakery', kind='bread_dough', location=dict(holder='baker'), condition=100, owner='baker', quantity=4, passengers=[]),
        dict(id='damaged_sword', kind='iron_sword', location=dict(holder='smith'), condition=40, owner='smith', quantity=1, passengers=[]),
        dict(id='perfect_sword', kind='iron_sword', location=dict(holder='smith'), condition=100, owner='smith', quantity=1, passengers=[]),
        dict(id='lost_sword', kind='iron_sword', location=dict(place='forest'), condition=50, owner='smith', quantity=1, passengers=[]),
    ]
)

SAMPLE_BIO = dict(
    skills=[
        dict(id='smithing', label='Blacksmithing', level=15),
        dict(id='carpentry', label='Carpentry', level=10),
    ]
)

SAMPLE_CODEX = dict(
    version=1,
    places=[
        dict(id='forge', label='The Forge', kind='workshop', within='village'),
        dict(id='bakery', label='The Bakery', kind='bakery', within='village'),
        dict(id='clearing', label='Forest Clearing', kind='clearing', within='vale', tags=['wild']),
        dict(id='forest', label='Deep Forest', kind='forest', within='vale'),
    ],
    entrances=[
        dict(id='mine_gate', label='Mine Gate', sides=[dict(place='clearing'), dict(place='forest')], status='closed')
    ]
)


def op(name, **args): return dict(op=name, args=args)
def run(ops, recipes=None, things=None, bio=None, codex=None):
    return dict(
        recipes=recipes if recipes is not None else copy.deepcopy(SAMPLE_RECIPES),
        things=things if things is not None else copy.deepcopy(BASE_THINGS),
        bio=bio if bio is not None else copy.deepcopy(SAMPLE_BIO),
        codex=codex if codex is not None else copy.deepcopy(SAMPLE_CODEX),
        ops=ops
    )


CASES = [
    ('check_feasible_crafting', run([op('check', recipe='craft_sword', actor='smith', place='forge')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['feasible'] is True, False),
    ('check_missing_tool_fails', run([op('check', recipe='craft_sword', actor='baker', place='forge')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['feasible'] is False and any(m['reason'] == 'missing_tool' for m in o['results'][0]['result']['missing']), False),
    ('check_missing_station_fails', run([op('check', recipe='craft_sword', actor='smith', place='clearing')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['feasible'] is False and any(m['reason'] == 'missing_station' for m in o['results'][0]['result']['missing']), False),
    ('check_missing_skill_fails', run([op('check', recipe='craft_sword', actor='smith', place='forge')], bio=dict(skills=[dict(id='smithing', label='Smithing', level=5)])),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['feasible'] is False and any(m['reason'] == 'missing_skill' for m in o['results'][0]['result']['missing']), False),
    ('check_missing_input_fails', run([op('check', recipe='craft_sword', actor='smith', place='forge')], things=dict(BASE_THINGS, things=[BASE_THINGS['things'][0], BASE_THINGS['things'][1]])),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['feasible'] is False and any(m['reason'] == 'missing_input' for m in o['results'][0]['result']['missing']), False),
    ('check_missing_target_fails', run([op('check', recipe='repair_sword', actor='smith', place='forge')]),
     lambda o: o['results'][0]['ok'] and any(m['reason'] == 'missing_target' for m in o['results'][0]['result']['missing']), False),
    ('check_target_not_found_fails', run([op('check', recipe='repair_sword', actor='smith', place='forge', target_thing='ghost_item')]),
     lambda o: o['results'][0]['ok'] and any(m['reason'] == 'target_not_found' for m in o['results'][0]['result']['missing']), False),
    ('check_target_not_accessible_fails', run([op('check', recipe='repair_sword', actor='smith', place='forge', target_thing='lost_sword')]),
     lambda o: o['results'][0]['ok'] and any(m['reason'] == 'target_not_accessible' for m in o['results'][0]['result']['missing']), False),
    ('execute_crafting_ok', run([op('execute', recipe='craft_sword', actor='smith', place='forge', new_ids=['forged_sword_1'])]),
     lambda o: o['results'][0]['ok'] and any(t['id'] == 'forged_sword_1' and t['location'] == dict(holder='smith') for t in o['things']['things']), False),
    ('execute_cooking_ok', run([op('execute', recipe='bake_bread', actor='baker', place='bakery', new_ids=['bread_fresh_1'])]),
     lambda o: o['results'][0]['ok'] and any(t['id'] == 'bread_fresh_1' for t in o['things']['things']), False),
    ('execute_repair_ok', run([op('execute', recipe='repair_sword', actor='smith', place='forge', target_thing='damaged_sword')]),
     lambda o: o['results'][0]['ok'] and any(t['id'] == 'damaged_sword' and t['condition'] == 100 for t in o['things']['things']), False),
    ('execute_repair_already_perfect_fails', run([
        op('execute', recipe='repair_sword', actor='smith', place='forge', target_thing='perfect_sword')
    ]), lambda o: not o['results'][0]['ok'], False),
    ('execute_dismantle_ok', run([op('execute', recipe='dismantle_sword', actor='smith', place='forge', target_thing='damaged_sword', new_ids=['scrap_iron_1'])]),
     lambda o: o['results'][0]['ok'] and not any(t['id'] == 'damaged_sword' for t in o['things']['things']) and any(t['id'] == 'scrap_iron_1' for t in o['things']['things']), False),
    ('execute_building_changes_world', run([
        op('execute', recipe='build_bridge', actor='smith', place='clearing', new_ids=['footbridge_1'])
    ]), lambda o: o['results'][0]['ok'] and any(p['id'] == 'clearing' and p.get('kind') == 'bridge' and 'accessible' in p.get('tags', []) for p in o['codex']['places']), False),
    ('execute_building_opens_entrance', run([
        op('execute', recipe='clear_mine_entrance', actor='smith', place='clearing')
    ]), lambda o: o['results'][0]['ok'] and any(e['id'] == 'mine_gate' and e.get('status') == 'open' for e in o['codex']['entrances']), False),
    ('atomic_execution_reverts_on_fail', run([
        op('execute', recipe='craft_sword', actor='smith', place='clearing')
    ]), lambda o: not o['results'][0]['ok'], False),
    # Top-level error cases
    ('error_unknown_key', dict(ops=[], extra_bad=1), None, True),
    ('error_too_many_ops', dict(ops=[op('check', recipe='craft_sword', actor='smith', place='forge') for _ in range(65)]), None, True),
    ('error_bad_category', run([], recipes=[dict(SAMPLE_RECIPES[0], category='invalid_cat')]), None, True),
    ('error_duplicate_recipe_id', run([], recipes=[SAMPLE_RECIPES[0], SAMPLE_RECIPES[0]]), None, True),
    ('error_duplicate_tool_function', run([], recipes=[dict(SAMPLE_RECIPES[0], tools=[dict(function='hammer', power=1), dict(function='hammer', power=2)])]), None, True),
    ('error_building_on_non_building_recipe', run([], recipes=[dict(SAMPLE_RECIPES[0], building=dict(structure_kind='bridge'))]), None, True),
    ('error_input_both_kind_and_tag', run([], recipes=[dict(SAMPLE_RECIPES[0], inputs=[dict(kind='iron_ore', tag='ore', quantity=1)])]), None, True),
    ('error_input_neither_kind_nor_tag', run([], recipes=[dict(SAMPLE_RECIPES[0], inputs=[dict(quantity=1)])]), None, True),
    ('error_bad_op_name', run([dict(op='unknown_op', args={})]), None, True),
]


def random_workflow(r):
    ops = []
    recipes = copy.deepcopy(SAMPLE_RECIPES)
    things = copy.deepcopy(BASE_THINGS)
    bio = copy.deepcopy(SAMPLE_BIO)
    codex = copy.deepcopy(SAMPLE_CODEX)

    rec_ids = [rec['id'] for rec in recipes]
    actors = ['smith', 'baker', 'traveller']
    places = ['forge', 'bakery', 'clearing', 'forest']

    for _ in range(r.randint(3, 8)):
        op_type = r.choice(['check', 'execute'])
        rc = r.choice(rec_ids)
        act = r.choice(actors)
        plc = r.choice(places)
        if rc in ['repair_sword', 'dismantle_sword']:
            tgt = 'damaged_sword'
        else:
            tgt = None
        if op_type == 'check':
            ops.append(op('check', recipe=rc, actor=act, place=plc, target_thing=tgt))
        else:
            ops.append(op('execute', recipe=rc, actor=act, place=plc, target_thing=tgt, new_ids=[f"rnd_{_}"]))

    return run(ops, recipes=recipes, things=things, bio=bio, codex=codex)


def call(req):
    return subprocess.run([str(ROOT / 'target/debug/gamectx'), 'local', '-'], input=r.enc(req), capture_output=True, text=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--cap', type=float, required=True)
    p.add_argument('--prototype-only', action='store_true')
    p.add_argument('--write-conformance', action='store_true')
    p.add_argument('--random', type=int, default=0)
    p.add_argument('--seed', type=int, default=7)
    a = p.parse_args()

    assert a.cap == 0, 'W4 recipes eval is $0 offline only'

    if a.random > 0:
        rng = random.Random(a.seed)
        mismatches = 0
        for _ in range(a.random):
            args = random_workflow(rng)
            req = dict(kind='recipes', args=args)
            got = r.workflow(copy.deepcopy(args))
            if not a.prototype_only:
                x = call(req)
                if x.returncode != 0 or x.stdout.strip() != r.enc(got):
                    mismatches += 1
                    print('MISMATCH', r.enc(args)[:600])
                    print('Ref :', r.enc(got)[:600])
                    print('Rust:', x.stdout.strip()[:600])
                    print('Err :', x.stderr[:600])
                    assert False, "differential fuzzing mismatch"
        print(r.enc(dict(workflows=a.random, mismatches=mismatches, actual_model_calls=0, api_spend_usd=0)))
        return

    for n, args, check, error in CASES:
        try:
            got = r.workflow(copy.deepcopy(args))
            assert not error, n
        except (AssertionError, KeyError, TypeError, ValueError, StopIteration, IndexError):
            assert error, n
            got = None
        if check is not None:
            assert check(got), (n, r.enc(got)[:1500])
        req = dict(kind='recipes', args=args)
        if not a.prototype_only:
            x = call(req)
            assert (x.returncode != 0) == error, (n, x.stderr)
            if not error:
                assert x.stdout.strip() == r.enc(got), (n, x.stdout[:1500], r.enc(got)[:1500])
        if a.write_conformance:
            dd = ROOT / 'conformance/recipes' / n
            new = not dd.exists()
            dd.mkdir(parents=True, exist_ok=True)
            for k, v in [('case', dict(op='local', **(dict(error=True) if error else {}))), ('input', req)] + ([] if error else [('expected', got)]):
                f = dd / (k + '.json')
                if new:
                    f.write_text(r.enc(v) + '\n')
                else:
                    assert f.read_text() == r.enc(v) + '\n', f
    print(r.enc(dict(workflows=len(CASES), malformed=sum(e for *_, e in CASES), actual_model_calls=0, api_spend_usd=0)))


if __name__ == '__main__':
    main()
