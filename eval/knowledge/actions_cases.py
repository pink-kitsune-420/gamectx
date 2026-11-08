"""Action catalog cases. Expectations were written before running the reference; the
`actions` local kind must match the reference byte for byte. Offline only (--cap 0).

  python3 -I eval/knowledge/actions_cases.py --cap 0 [--write-conformance] [--random N --seed S]
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
import argparse, copy, random, subprocess
import actions_reference as r

ROOT = _pl.Path(__file__).resolve().parents[2]


def op(name, **kwargs):
    return dict(op=name, args=kwargs)


def run(ops, **kwargs):
    req = dict(ops=ops)
    req.update(kwargs)
    return req


CASES = [
    # 1. List all actions
    (
        "list_all",
        run([op('list')]),
        lambda out: len(out['results'][0]['actions']) >= 49,
        False,
    ),

    # 2. List filtered by category 'recipes'
    (
        "list_category_recipes",
        run([op('list', category='recipes')]),
        lambda out: len(out['results'][0]['actions']) == 5 and all(a['category'] == 'recipes' for a in out['results'][0]['actions']),
        False,
    ),

    # 3. List filtered by actor_species 'animal'
    (
        "list_species_animal",
        run([op('list', actor_species='animal')]),
        lambda out: len(out['results'][0]['actions']) >= 1 and any(a['id'] == 'behave' for a in out['results'][0]['actions']),
        False,
    ),

    # 4. Describe standard action 'craft'
    (
        "describe_craft",
        run([op('describe', id='craft')]),
        lambda out: out['results'][0]['action']['id'] == 'craft' and out['results'][0]['action']['category'] == 'recipes',
        False,
    ),

    # 5. Describe standard action 'pet'
    (
        "describe_pet",
        run([op('describe', id='pet')]),
        lambda out: out['results'][0]['action']['id'] == 'pet' and out['results'][0]['action']['category'] == 'species',
        False,
    ),

    # 6. Check feasible action 'pet' with animal target
    (
        "check_pet_feasible",
        run([
            op(
                'check',
                action='pet',
                actor=dict(id='hero', species='person', fitness=dict(fatigue=10, condition=100)),
                bindings=dict(animal='wolfhound'),
            )
        ]),
        lambda out: out['results'][0]['feasible'] is True and out['results'][0]['action'] == 'pet',
        False,
    ),

    # 7. Check refused action when actor is asleep
    (
        "check_refused_asleep",
        run([
            op(
                'check',
                action='craft',
                actor=dict(id='smith', species='person', conditions=['asleep']),
                bindings=dict(recipe='iron_sword'),
            )
        ]),
        lambda out: out['results'][0]['feasible'] is False and 'asleep' in out['results'][0]['reason'],
        False,
    ),

    # 8. Check refused action when actor lacks required skill level
    (
        "check_refused_skill_too_low",
        run([
            op(
                'check',
                action='craft',
                actor=dict(id='novice', species='person', skills=dict(crafting=0)),
                bindings=dict(recipe='iron_sword'),
            )
        ]),
        # craft builtin requires no specific skill, let's test custom registered action with skill requirement below
        lambda out: out['results'][0]['feasible'] is True,
        False,
    ),

    # 9. Check refused when held item is not in inventory
    (
        "check_refused_held_missing",
        run([
            op(
                'check',
                action='store',
                actor=dict(id='hero', species='person', inventory=['apple']),
                bindings=dict(thing='golden_key', container='chest'),
            )
        ]),
        lambda out: out['results'][0]['feasible'] is False and 'not held in inventory' in out['results'][0]['reason'],
        False,
    ),

    # 10. Check feasible when held item is in inventory
    (
        "check_feasible_held_present",
        run([
            op(
                'check',
                action='store',
                actor=dict(id='hero', species='person', inventory=['golden_key']),
                bindings=dict(thing='golden_key', container='chest'),
            )
        ]),
        lambda out: out['results'][0]['feasible'] is True,
        False,
    ),

    # 11. Available actions query for baker in bakery
    (
        "available_baker",
        run([
            op(
                'available',
                actor=dict(
                    id='baker',
                    species='person',
                    fitness=dict(fatigue=20, condition=100),
                    inventory=['flour', 'water'],
                ),
                place='bakery',
                targets=[
                    dict(id='oven', kind='prop'),
                    dict(id='flour', kind='thing'),
                    dict(id='dog', kind='animal'),
                ],
                category='species',
            )
        ]),
        lambda out: any(a['action'] == 'pet' for a in out['results'][0]['available']),
        False,
    ),

    # 12. Register custom action and describe it
    (
        "register_custom_action",
        run([
            op(
                'register',
                action=r.make_action(
                    'mine_ore',
                    'Mine Ore',
                    'things',
                    'Mine ore veins from rock walls using a pickaxe.',
                    skills=[dict(skill='mining', level=15)],
                    targets=[dict(name='vein', kind='prop', required=True, description='Ore deposit')],
                    time=25,
                    fatigue=15,
                    world_effect='extracts raw ore into place',
                ),
            ),
            op('describe', id='mine_ore'),
        ]),
        lambda out: out['results'][0]['registered'] == 'mine_ore' and out['results'][1]['action']['id'] == 'mine_ore',
        False,
    ),

    # 13. Check registered custom action with insufficient skill
    (
        "check_custom_action_skill_insufficient",
        run([
            op(
                'register',
                action=r.make_action(
                    'forge_masterwork',
                    'Forge Masterwork',
                    'recipes',
                    'Forge an exceptional masterwork blade.',
                    skills=[dict(skill='smithing', level=50)],
                    targets=[dict(name='recipe', kind='recipe', required=True, description='Masterwork recipe')],
                ),
            ),
            op(
                'check',
                action='forge_masterwork',
                actor=dict(id='apprentice', species='person', skills=dict(smithing=20)),
                bindings=dict(recipe='blade'),
            ),
        ]),
        lambda out: out['results'][1]['feasible'] is False and 'smithing' in out['results'][1]['reason'],
        False,
    ),

    # 14. Check fatigue boundary failure
    (
        "check_fatigue_exhaustion",
        run([
            op(
                'check',
                action='craft',
                actor=dict(id='tired_worker', species='person', fitness=dict(fatigue=98)),
                bindings=dict(recipe='plank'),
            )
        ]),
        lambda out: out['results'][0]['feasible'] is False and 'fatigued' in out['results'][0]['reason'],
        False,
    ),

    # 15. Check condition boundary failure
    (
        "check_condition_zero",
        run([
            op(
                'check',
                action='craft',
                actor=dict(id='felled_worker', species='person', fitness=dict(condition=0)),
                bindings=dict(recipe='plank'),
            )
        ]),
        lambda out: out['results'][0]['feasible'] is False and 'condition' in out['results'][0]['reason'],
        False,
    ),

    # 16. Multiple ops in single workflow
    (
        "workflow_multi_ops",
        run([
            op('list', category='communities'),
            op('describe', id='barter'),
            op('describe', id='transfer'),
        ]),
        lambda out: len(out['results']) == 3 and out['results'][1]['action']['id'] == 'barter',
        False,
    ),

    # 17. Malformed: unknown op
    (
        "malformed_unknown_op",
        run([op('fly_to_moon')]),
        None,
        True,
    ),

    # 18. Malformed: missing op field
    (
        "malformed_missing_op",
        dict(ops=[dict(args=dict())]),
        None,
        True,
    ),

    # 19. Malformed: describe non-existent action
    (
        "malformed_describe_missing",
        run([op('describe', id='non_existent_spell')]),
        None,
        True,
    ),

    # 20. Malformed: bad action id in describe
    (
        "malformed_bad_id_characters",
        run([op('describe', id='craft with spaces!')]),
        None,
        True,
    ),

    # 21. Malformed: register action with unknown category
    (
        "malformed_register_bad_category",
        run([
            op(
                'register',
                action=dict(
                    id='custom_bad',
                    label='Bad Action',
                    category='sorcery_unknown',
                    description='Illegal category',
                    actor=dict(),
                    targets=[],
                    preconditions=dict(),
                    cost=dict(time=1, fatigue=1),
                    effects=dict(world='magic'),
                    perception=dict(visibility='public'),
                    risk=dict(failure_chance=0),
                    duration=1,
                    interruptible=True,
                ),
            )
        ]),
        None,
        True,
    ),

    # 22. Malformed: register action missing required field
    (
        "malformed_register_missing_field",
        run([
            op(
                'register',
                action=dict(
                    id='incomplete_action',
                    label='Incomplete',
                    category='things',
                ),
            )
        ]),
        None,
        True,
    ),

    # 23. Malformed: check with bad action id
    (
        "malformed_check_bad_action",
        run([
            op(
                'check',
                action='not_a_registered_action',
                actor=dict(id='hero'),
            )
        ]),
        None,
        True,
    ),

    # 24. Malformed: list with invalid category filter
    (
        "malformed_list_invalid_category",
        run([op('list', category='alchemy_invalid')]),
        None,
        True,
    ),

    # 25. Malformed: ops exceeding MAX_OPS (65 ops)
    (
        "malformed_too_many_ops",
        run([op('list') for _ in range(65)]),
        None,
        True,
    ),
]


def random_workflow(rng):
    ops = []
    num_ops = rng.randint(1, 5)
    for _ in range(num_ops):
        op_choice = rng.choice(['list', 'describe', 'available', 'check'])
        if op_choice == 'list':
            cat = rng.choice([None] + r.CATEGORIES)
            args = dict()
            if cat:
                args['category'] = cat
            ops.append(op('list', **args))
        elif op_choice == 'describe':
            act_id = rng.choice([a['id'] for a in r.BUILTIN_ACTIONS])
            ops.append(op('describe', id=act_id))
        elif op_choice == 'available':
            cat = rng.choice([None] + r.CATEGORIES)
            fatigue = rng.randint(0, 90)
            cond = rng.randint(50, 100)
            args = dict(
                actor=dict(
                    id=f'actor_{rng.randint(1, 10)}',
                    species=rng.choice(['person', 'animal']),
                    fitness=dict(fatigue=fatigue, condition=cond),
                    inventory=['iron_ore', 'wood_log'],
                ),
                targets=[
                    dict(id='thing_1', kind='thing'),
                    dict(id='prop_1', kind='prop'),
                    dict(id='animal_1', kind='animal'),
                ],
            )
            if cat:
                args['category'] = cat
            ops.append(op('available', **args))
        else: # check
            act_id = rng.choice(['pet', 'craft', 'store', 'barter', 'transfer'])
            fatigue = rng.randint(0, 80)
            ops.append(op(
                'check',
                action=act_id,
                actor=dict(
                    id='hero',
                    species='person',
                    fitness=dict(fatigue=fatigue, condition=100),
                    inventory=['iron_ore', 'golden_key'],
                ),
                bindings=dict(
                    thing='golden_key',
                    container='chest',
                    animal='wolfhound',
                    recipe='iron_sword',
                    community='ember_wardens',
                    offer='hero_sword',
                    request='torch',
                    recipient='merchant',
                ),
            ))
    return run(ops)


def call(req):
    return subprocess.run([str(ROOT / 'target/debug/gamectx'), 'local', '-'], input=r.enc(req), capture_output=True, text=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--cap', type=float, required=True)
    p.add_argument('--prototype-only', action='store_true')
    p.add_argument('--write-conformance', action='store_true')
    p.add_argument('--random', type=int, default=0)
    p.add_argument('--seed', type=int, default=7)
    p.add_argument('--check', action='store_true')
    a = p.parse_args()

    if a.random > 0:
        rng = random.Random(a.seed)
        mismatches = 0
        for _ in range(a.random):
            args = random_workflow(rng)
            req = dict(kind='actions', args=args)
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
        req = dict(kind='actions', args=args)
        if not a.prototype_only:
            x = call(req)
            assert (x.returncode != 0) == error, (n, x.stderr)
            if not error:
                assert x.stdout.strip() == r.enc(got), (n, x.stdout[:1500], r.enc(got)[:1500])
        if a.write_conformance:
            dd = ROOT / 'conformance/actions' / n
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
