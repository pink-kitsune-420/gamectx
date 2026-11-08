"""Character fitness cases. Expectations written before running the reference; the
`fitness` local kind must match the reference byte for byte. Offline only (--cap 0).

  python3 -I eval/character/fitness_cases.py --cap 0 [--write-conformance] [--random N --seed S]
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
import argparse, copy, random, subprocess
import fitness_reference as r

ROOT = _pl.Path(__file__).resolve().parents[2]

WARRIOR_FITNESS = dict(strength=75, endurance=65, agility=60, condition=90)
SCOUT_FITNESS = dict(strength=40, endurance=70, agility=85, condition=95)
SCHOLAR_FITNESS = dict(strength=25, endurance=30, agility=35, condition=70)


def op(name, at=0, **args): return dict(op=name, at=at, args=args)
def run(ops, fitness=None):
    res = dict(ops=ops)
    if fitness is not None:
        res['fitness'] = fitness
    return res


CASES = [
    (
        'assess_standard_warrior',
        run([op('assess')], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'assess_with_hurt_health_penalty',
        run([op('assess', health=dict(status='hurt'))], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'assess_with_sick_health_penalty',
        run([op('assess', health=dict(status='sick'))], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'assess_with_exhausted_health_penalty',
        run([op('assess', health=dict(status='exhausted'))], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'assess_with_low_energy_needs',
        run([op('assess', needs=dict(energy=15, nourishment=20))], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'assess_with_overload_speed_penalty',
        run([op('assess', load=150)], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'travel_cost_short_walk',
        run([op('travel_cost', distance=50)], fitness=copy.deepcopy(SCOUT_FITNESS))
    ),
    (
        'travel_cost_long_march_fatigue',
        run([op('travel_cost', distance=500)], fitness=copy.deepcopy(SCOUT_FITNESS))
    ),
    (
        'travel_cost_with_heavy_load',
        run([op('travel_cost', distance=100, load=60)], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'can_carry_within_capacity',
        run([op('can_carry', current_load=40, add_weight=20)], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'can_carry_exceeding_capacity',
        run([op('can_carry', current_load=60, add_weight=80)], fitness=copy.deepcopy(SCHOLAR_FITNESS))
    ),
    (
        'can_carry_exact_capacity',
        run([op('can_carry', current_load=30, add_weight=5)], fitness=copy.deepcopy(SCHOLAR_FITNESS))
    ),
    (
        'train_strength_gain',
        run([op('train', at=10, actor='warrior', attribute='strength', effort=50)], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'train_endurance_gain',
        run([op('train', at=15, actor='scout', attribute='endurance', effort=60)], fitness=copy.deepcopy(SCOUT_FITNESS))
    ),
    (
        'train_agility_gain',
        run([op('train', at=20, actor='scholar', attribute='agility', effort=40)], fitness=copy.deepcopy(SCHOLAR_FITNESS))
    ),
    (
        'train_near_max_diminishing_returns',
        run([op('train', at=25, actor='master', attribute='strength', effort=50, fitness=dict(strength=95, endurance=80, agility=80, condition=100))])
    ),
    (
        'train_at_100_zero_gain',
        run([op('train', at=30, actor='titan', attribute='strength', effort=100, fitness=dict(strength=100, endurance=100, agility=100, condition=100))])
    ),
    (
        'train_emits_timeline_entry',
        run([op('train', at=35, actor='hero', attribute='agility', effort=50, place='training_yard')], fitness=copy.deepcopy(SCOUT_FITNESS))
    ),
    (
        'multi_op_fitness_training_regimen',
        run([
            op('assess'),
            op('train', at=40, actor='hero', attribute='strength', effort=40),
            op('train', at=45, actor='hero', attribute='endurance', effort=30),
            op('assess')
        ], fitness=copy.deepcopy(SCHOLAR_FITNESS))
    ),
    (
        'assess_default_fitness_values',
        run([op('assess')])
    ),
    (
        'travel_cost_zero_distance',
        run([op('travel_cost', distance=0)], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'can_carry_with_hurt_health_reduced_capacity',
        run([op('can_carry', current_load=40, add_weight=20, health=dict(status='hurt'))], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'train_with_place',
        run([op('train', at=50, actor='recruit', attribute='endurance', effort=50, place='barracks_gym')], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
    (
        'multi_op_assess_and_travel',
        run([
            op('assess', health=dict(status='healthy')),
            op('travel_cost', distance=200, load=25),
            op('can_carry', current_load=25, add_weight=30)
        ], fitness=copy.deepcopy(WARRIOR_FITNESS))
    ),
]

MALFORMED = [
    ('error_unknown_key', dict(unknown='bad', ops=[])),
    ('error_bad_op_name', dict(ops=[dict(op='nonexistent_op')])),
    ('error_too_many_ops', dict(ops=[dict(op='assess')] * 65)),
    ('error_bad_attribute_train', dict(ops=[dict(op='train', args=dict(actor='a', attribute='intellect', effort=50))])),
    ('error_attribute_out_of_range', dict(fitness=dict(strength=120), ops=[])),
    ('error_effort_out_of_range', dict(ops=[dict(op='train', args=dict(actor='a', attribute='strength', effort=0))])),
]


def random_workflow(seed):
    rng = random.Random(seed)
    num_ops = rng.randint(1, 6)
    ops = []
    actors = ['hero', 'warrior', 'scout', 'scholar', 'guard']
    places = ['gym', 'yard', 'road', 'forest', 'keep']
    healths = ['healthy', 'hurt', 'sick', 'exhausted', None]
    attrs = ['strength', 'endurance', 'agility']

    init_fit = dict(
        strength=rng.randint(20, 95),
        endurance=rng.randint(20, 95),
        agility=rng.randint(20, 95),
        condition=rng.randint(40, 100),
    ) if rng.random() < 0.8 else None

    for _ in range(num_ops):
        op_name = rng.choice(['assess', 'travel_cost', 'can_carry', 'train'])
        at = rng.randint(0, 1000)

        if op_name == 'assess':
            h = rng.choice(healths)
            h_arg = dict(status=h) if h else None
            ld = rng.randint(0, 150) if rng.random() < 0.5 else 0
            ops.append(op(op_name, at=at, health=h_arg, load=ld))
        elif op_name == 'travel_cost':
            dist = rng.randint(0, 1000)
            ld = rng.randint(0, 100) if rng.random() < 0.5 else 0
            h = rng.choice(healths)
            h_arg = dict(status=h) if h else None
            ops.append(op(op_name, at=at, distance=dist, load=ld, health=h_arg))
        elif op_name == 'can_carry':
            cur = rng.randint(0, 80)
            add = rng.randint(0, 50)
            ops.append(op(op_name, at=at, current_load=cur, add_weight=add))
        elif op_name == 'train':
            act = rng.choice(actors)
            attr = rng.choice(attrs)
            eff = rng.randint(1, 100)
            pl = rng.choice(places) if rng.random() < 0.6 else None
            ops.append(op(op_name, at=at, actor=act, attribute=attr, effort=eff, place=pl))

    return run(ops, fitness=init_fit)


def run_local(payload):
    proc = subprocess.run(
        [str(ROOT / 'target' / 'debug' / 'gamectx'), 'local', '-'],
        input=r.enc(dict(kind='fitness', args=payload)).encode('utf-8') + b'\n',
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True
    )
    line = proc.stdout.strip().split(b'\n')[-1]
    return r.json.loads(line.decode('utf-8'))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--cap', type=int, default=0)
    parser.add_argument('--write-conformance', action='store_true')
    parser.add_argument('--random', type=int, default=0)
    parser.add_argument('--seed', type=int, default=7)
    args = parser.parse_args()

    conf_dir = ROOT / 'conformance' / 'fitness'
    if args.write_conformance:
        conf_dir.mkdir(parents=True, exist_ok=True)
        for name, payload in CASES:
            case_d = conf_dir / name
            case_d.mkdir(exist_ok=True)
            res = r.workflow(payload)
            req = dict(kind='fitness', args=payload)
            (case_d / 'case.json').write_text(r.enc(dict(op='local')) + '\n')
            (case_d / 'input.json').write_text(r.enc(req) + '\n')
            (case_d / 'expected.json').write_text(r.enc(res) + '\n')
        for name, payload in MALFORMED:
            case_d = conf_dir / name
            case_d.mkdir(exist_ok=True)
            req = dict(kind='fitness', args=payload)
            (case_d / 'case.json').write_text(r.enc(dict(op='local', error=True)) + '\n')
            (case_d / 'input.json').write_text(r.enc(req) + '\n')
        print(r.enc(dict(workflows=len(CASES), malformed=len(MALFORMED), actual_model_calls=0, api_spend_usd=0)))
        return

    if args.random > 0:
        mismatches = 0
        for i in range(args.random):
            seed = args.seed + i
            wf = random_workflow(seed)
            expected = r.workflow(wf)
            actual = run_local(wf)
            if r.enc(expected) != r.enc(actual):
                mismatches += 1
                if mismatches <= 3:
                    print(f"Mismatch at seed {seed}:\nExpected: {r.enc(expected)}\nActual:   {r.enc(actual)}")
        print(r.enc(dict(workflows=args.random, mismatches=mismatches, actual_model_calls=0, api_spend_usd=0)))
        if mismatches > 0:
            _sys.exit(1)
        return

    # Check conformance cases
    for name, payload in CASES:
        expected = r.workflow(payload)
        actual = run_local(payload)
        assert r.enc(expected) == r.enc(actual), f"Mismatch in {name}:\nExp: {r.enc(expected)}\nAct: {r.enc(actual)}"

    for name, payload in MALFORMED:
        proc = subprocess.run(
            [str(ROOT / 'target' / 'debug' / 'gamectx'), 'local', '-'],
            input=r.enc(dict(kind='fitness', args=payload)).encode('utf-8') + b'\n',
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        assert proc.returncode != 0 or b'"error"' in proc.stdout, f"Malformed case {name} succeeded!"

    print(r.enc(dict(workflows=len(CASES), malformed=len(MALFORMED), actual_model_calls=0, api_spend_usd=0)))


if __name__ == '__main__':
    main()
