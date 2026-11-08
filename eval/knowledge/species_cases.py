"""World species cases. Expectations written before running the reference; the
`species` local kind must match the reference byte for byte. Offline only (--cap 0).

  python3 -I eval/knowledge/species_cases.py --cap 0 [--write-conformance] [--random N --seed S]
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
import argparse, copy, random, subprocess
import species_reference as r

ROOT = _pl.Path(__file__).resolve().parents[2]

SAMPLE_SPECIES = [
    dict(
        id='horse',
        label='Domestic Horse',
        kind='animal',
        diet='herbivore',
        habitats=['plains', 'clearing'],
        lifespan=30,
        temperament=dict(boldness=60, sociability=80, aggression=20, trainability=70),
        tameable=True,
        rideable=True,
        carrier_capacity=150,
        produce=[],
        sounds=['neigh', 'whinny', 'snort'],
        gestures=['swishes tail', 'pricks ears', 'nudges hand']
    ),
    dict(
        id='donkey',
        label='Pack Donkey',
        kind='animal',
        diet='herbivore',
        habitats=['mountain', 'clearing'],
        lifespan=35,
        temperament=dict(boldness=70, sociability=50, aggression=30, trainability=60),
        tameable=True,
        rideable=True,
        carrier_capacity=100,
        produce=[],
        sounds=['bray', 'snort'],
        gestures=['lowers head', 'braces feet']
    ),
    dict(
        id='mule',
        label='Surefooted Mule',
        kind='hybrid',
        parents=['horse', 'donkey'],
        carrier_capacity=140, # explicit override
        sounds=['whinny-bray'],
        gestures=['shakes mane', 'steps firmly']
    ),
    dict(
        id='sheep',
        label='Woolly Sheep',
        kind='animal',
        diet='herbivore',
        habitats=['plains', 'hills'],
        lifespan=12,
        temperament=dict(boldness=20, sociability=90, aggression=10, trainability=40),
        tameable=True,
        rideable=False,
        carrier_capacity=0,
        produce=[dict(kind='wool', amount=2, interval=48)],
        sounds=['baa', 'bleat'],
        gestures=['chews cud', 'huddles']
    ),
    dict(
        id='wolf',
        label='Timber Wolf',
        kind='animal',
        diet='carnivore',
        habitats=['forest', 'mountain'],
        lifespan=14,
        temperament=dict(boldness=80, sociability=70, aggression=85, trainability=30),
        tameable=True,
        rideable=False,
        carrier_capacity=0,
        produce=[],
        sounds=['howl', 'growl', 'snarl'],
        gestures=['bares teeth', 'stalks low', 'circles']
    ),
    dict(
        id='apple_tree',
        label='Orchard Apple Tree',
        kind='plant',
        diet='photosynthetic',
        habitats=['orchard', 'clearing'],
        lifespan=100,
        temperament=dict(boldness=0, sociability=0, aggression=0, trainability=0),
        tameable=False,
        rideable=False,
        carrier_capacity=0,
        produce=[dict(kind='apple', amount=5, interval=72)],
        sounds=[],
        gestures=[]
    ),
]

SAMPLE_ANIMALS = [
    dict(
        id='bramble',
        species='mule',
        tameness=80,
        trust=dict(farmer=50, stranger=0),
        lead_by=None,
        ridden_by=None
    ),
    dict(
        id='shadow',
        species='wolf',
        tameness=10,
        trust=dict(hunter=20, stranger=-20),
        lead_by=None,
        ridden_by=None
    ),
    dict(
        id='daisy',
        species='sheep',
        tameness=90,
        trust=dict(farmer=60),
        last_harvest=dict(wool=0)
    ),
]

SAMPLE_THINGS = dict(
    version=1,
    materials=[dict(id='wood', label='Timber'), dict(id='fiber', label='Plant Fiber')],
    kinds=[
        dict(id='apple', label='Crisp Apple', category='food', materials=[], functions=[], weight=1, base_value=1, durability=5, stackable=True, tags=['plant', 'ration']),
        dict(id='hay', label='Dry Hay', category='material', materials=[], functions=[], weight=5, base_value=1, durability=50, stackable=True, tags=['plant', 'ration']),
        dict(id='raw_meat', label='Raw Venison', category='food', materials=[], functions=[], weight=2, base_value=3, durability=10, stackable=True, tags=['meat', 'ration']),
        dict(id='wool', label='Soft Wool', category='material', materials=[], functions=[], weight=2, base_value=4, durability=100, stackable=True, tags=['fiber']),
    ],
    things=[
        dict(id='farmer_hay', kind='hay', location=dict(holder='farmer'), condition=100, owner='farmer', quantity=5, passengers=[]),
        dict(id='farmer_apple', kind='apple', location=dict(holder='farmer'), condition=100, owner='farmer', quantity=3, passengers=[]),
        dict(id='hunter_meat', kind='raw_meat', location=dict(holder='hunter'), condition=100, owner='hunter', quantity=2, passengers=[]),
    ]
)


def op(name, at=0, **args): return dict(op=name, at=at, args=args)
def run(ops, species=None, animals=None, things=None):
    return dict(
        species=species if species is not None else copy.deepcopy(SAMPLE_SPECIES),
        animals=animals if animals is not None else copy.deepcopy(SAMPLE_ANIMALS),
        things=things if things is not None else copy.deepcopy(SAMPLE_THINGS),
        ops=ops
    )


CASES = [
    ('pet_calm_animal_increases_trust', run([op('pet', actor='farmer', animal='bramble', place='clearing')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['success'] and o['results'][0]['result']['trust'] > 50, False),
    ('pet_aggressive_wild_animal_fails', run([op('pet', actor='stranger', animal='shadow', place='forest')]),
     lambda o: o['results'][0]['ok'] and not o['results'][0]['result']['success'] and o['results'][0]['result']['trust'] < -20, False),
    ('feed_herbivore_food_increases_trust_and_tameness', run([op('feed', actor='farmer', animal='bramble', place='clearing', food_thing='farmer_apple')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['trust'] > 50 and o['results'][0]['result']['tameness'] > 80, False),
    ('feed_carnivore_meat_ok', run([op('feed', actor='hunter', animal='shadow', place='forest', food_thing='hunter_meat')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['trust'] > 20, False),
    ('feed_herbivore_meat_fails_diet', run([op('feed', actor='hunter', animal='bramble', place='clearing', food_thing='hunter_meat')]),
     lambda o: not o['results'][0]['ok'] and o['results'][0]['result']['reason'] == 'incompatible_diet', False),
    ('calm_animal_increases_trust', run([op('calm', actor='stranger', animal='bramble', place='clearing')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['trust'] == 5, False),
    ('lead_animal_ok_with_trust', run([op('lead', actor='farmer', animal='bramble', place='clearing')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['leading'] is True, False),
    ('lead_animal_fails_without_trust', run([op('lead', actor='stranger', animal='shadow', place='forest')]),
     lambda o: not o['results'][0]['ok'] and o['results'][0]['result']['reason'] == 'insufficient_trust_or_tameness', False),
    ('lead_stop_ok', run([op('lead', actor='farmer', animal='bramble', place='clearing', stop=True)]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['leading'] is False, False),
    ('ride_rideable_tamed_animal_ok', run([op('ride', actor='farmer', animal='bramble', place='clearing')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['riding'] is True, False),
    ('ride_not_rideable_species_fails', run([op('ride', actor='farmer', animal='daisy', place='plains')]),
     lambda o: not o['results'][0]['ok'] and o['results'][0]['result']['reason'] == 'not_rideable', False),
    ('ride_wild_untamed_fails', run([op('ride', actor='hunter', animal='shadow', place='forest')]),
     lambda o: not o['results'][0]['ok'], False),
    ('ride_dismount_ok', run([op('ride', actor='farmer', animal='bramble', place='clearing', dismount=True)]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['riding'] is False, False),
    ('train_animal_increases_tameness', run([op('train', actor='farmer', animal='bramble', place='clearing')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['tameness'] > 80, False),
    ('train_untameable_fails', run([op('train', actor='farmer', animal='ancient_tree', place='orchard')],
                                   animals=[dict(id='ancient_tree', species='apple_tree', tameness=0)]),
     lambda o: not o['results'][0]['ok'] and o['results'][0]['result']['reason'] == 'not_tameable', False),
    ('harvest_produce_ok', run([op('harvest', at=100, actor='farmer', animal='daisy', place='clearing', new_id='fresh_wool')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['product'] == 'wool' and any(t['id'] == 'fresh_wool' for t in o['things']['things']), False),
    ('harvest_before_interval_fails', run([op('harvest', at=20, actor='farmer', animal='daisy', place='clearing')]),
     lambda o: not o['results'][0]['ok'] and o['results'][0]['result']['reason'] == 'not_ready', False),
    ('behave_affectionate', run([op('behave', actor='farmer', animal='bramble', place='clearing')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['attitude'] == 'affectionate', False),
    ('behave_hostile', run([op('behave', actor='stranger', animal='shadow', place='forest')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['attitude'] == 'hostile', False),
    ('behave_calm', run([op('behave', actor='stranger', animal='bramble', place='clearing')]),
     lambda o: o['results'][0]['ok'] and o['results'][0]['result']['attitude'] == 'calm', False),
    # Top-level error cases
    ('error_unknown_key', dict(ops=[], extra_bad=1), None, True),
    ('error_too_many_ops', dict(ops=[op('calm', actor='farmer', animal='bramble', place='clearing') for _ in range(65)]), None, True),
    ('error_bad_species_kind', run([], species=[dict(SAMPLE_SPECIES[0], kind='alien')]), None, True),
    ('error_hybrid_without_parents', run([], species=[dict(SAMPLE_SPECIES[2], parents=[])]), None, True),
    ('error_hybrid_inheritance_cycle', run([], species=[
        dict(id='sp_a', label='Species A', kind='hybrid', parents=['sp_b']),
        dict(id='sp_b', label='Species B', kind='hybrid', parents=['sp_a'])
    ]), None, True),
    ('error_bad_temperament_range', run([], species=[dict(SAMPLE_SPECIES[0], temperament=dict(boldness=150, sociability=50, aggression=50, trainability=50))]), None, True),
    ('error_bad_op_name', run([dict(op='unknown_op', args={})]), None, True),
]


def random_workflow(r):
    ops = []
    species = copy.deepcopy(SAMPLE_SPECIES)
    animals = copy.deepcopy(SAMPLE_ANIMALS)
    things = copy.deepcopy(SAMPLE_THINGS)

    actors = ['farmer', 'hunter', 'stranger']
    an_ids = ['bramble', 'shadow', 'daisy']
    places = ['clearing', 'forest', 'plains']
    foods = ['farmer_hay', 'farmer_apple', 'hunter_meat']

    for i in range(r.randint(3, 8)):
        op_type = r.choice(['pet', 'feed', 'calm', 'lead', 'ride', 'train', 'harvest', 'behave'])
        act = r.choice(actors)
        an = r.choice(an_ids)
        plc = r.choice(places)
        at = r.randint(0, 200)

        if op_type == 'pet':
            ops.append(op('pet', actor=act, animal=an, place=plc, at=at))
        elif op_type == 'feed':
            ops.append(op('feed', actor=act, animal=an, place=plc, food_thing=r.choice(foods), at=at))
        elif op_type == 'calm':
            ops.append(op('calm', actor=act, animal=an, place=plc, at=at))
        elif op_type == 'lead':
            ops.append(op('lead', actor=act, animal=an, place=plc, stop=r.choice([True, False]), at=at))
        elif op_type == 'ride':
            ops.append(op('ride', actor=act, animal=an, place=plc, dismount=r.choice([True, False]), at=at))
        elif op_type == 'train':
            ops.append(op('train', actor=act, animal=an, place=plc, at=at))
        elif op_type == 'harvest':
            ops.append(op('harvest', actor=act, animal=an, place=plc, new_id=f"harv_{i}", at=at))
        else: # behave
            ops.append(op('behave', actor=act, animal=an, place=plc))

    return run(ops, species=species, animals=animals, things=things)


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

    assert a.cap == 0, 'W5 species eval is $0 offline only'

    if a.random > 0:
        rng = random.Random(a.seed)
        mismatches = 0
        for _ in range(a.random):
            args = random_workflow(rng)
            req = dict(kind='species', args=args)
            got = r.workflow(copy.deepcopy(args))
            if not a.prototype_only:
                x = call(req)
                if x.returncode != 0 or x.stdout.strip() != r.enc(got):
                    mismatches += 1
                    import difflib
                    print('MISMATCH len Ref:', len(r.enc(got)), 'Rust:', len(x.stdout.strip()))
                    for diff_line in difflib.unified_diff([r.enc(got)], [x.stdout.strip()]):
                        print('DIFF:', diff_line)
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
        req = dict(kind='species', args=args)
        if not a.prototype_only:
            x = call(req)
            assert (x.returncode != 0) == error, (n, x.stderr)
            if not error:
                assert x.stdout.strip() == r.enc(got), (n, x.stdout[:1500], r.enc(got)[:1500])
        if a.write_conformance:
            dd = ROOT / 'conformance/species' / n
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
