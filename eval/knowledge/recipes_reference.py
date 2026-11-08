"""Independent original recipes reference: crafting, cooking, building, repairing, and dismantling.
Python standard library only; nothing here calls a model.

A recipe has inputs (kinds or tags with quantities), required tool functions with minimum power,
optional station (furniture function nearby in place), optional skill with minimum level
(read from character bio skills), outputs, and time.
Building changes the world: places a structure, adds spots or links, or patches place tags/kinds.
Checks feasibility first (with missing reasons), then executes atomically.
"""
import copy, json, re

MAX_RECIPES = 256
MAX_OPS = 64
MAX_SAFE = 9007199254740991

CATEGORIES = ['crafting', 'cooking', 'building', 'repairing', 'dismantling']


def enc(v): return json.dumps(v, ensure_ascii=False, separators=(',', ':'))
def control(s): return any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in s)
def text(s, chars):
    assert isinstance(s, str) and 1 <= len(s) <= chars and not control(s); return s
def ident(s): assert isinstance(s, str) and re.fullmatch('[A-Za-z0-9_:-]{1,64}', s); return s
def uint(n, lo=0, hi=MAX_SAFE): assert type(n) is int and lo <= n <= hi; return n
def boolean(b): assert type(b) is bool; return b
def keys(v, required, optional=()): assert isinstance(v, dict) and set(required) <= set(v) <= set(required) | set(optional)


def validate_input(inp):
    keys(inp, ['quantity'], ['kind', 'tag'])
    assert ('kind' in inp) != ('tag' in inp), "must specify either kind or tag"
    if 'kind' in inp: ident(inp['kind'])
    if 'tag' in inp: ident(inp['tag'])
    uint(inp['quantity'], 1, 1000000000)


def validate_tool(tl):
    keys(tl, ['function', 'power'])
    ident(tl['function']); uint(tl['power'], 1, 1000000000)


def validate_skill(sk):
    keys(sk, ['id', 'level'])
    ident(sk['id']); uint(sk['level'], 0, 100)


def validate_output(out):
    keys(out, ['kind', 'quantity'], ['condition'])
    ident(out['kind']); uint(out['quantity'], 1, 1000000000)
    if 'condition' in out: uint(out['condition'], 0, 100)


def validate_building(bld):
    keys(bld, [], ['structure_kind', 'add_spots', 'open_entrance', 'add_link', 'patch_place'])
    if 'structure_kind' in bld: ident(bld['structure_kind'])
    if 'add_spots' in bld:
        assert isinstance(bld['add_spots'], list) and len(bld['add_spots']) <= 8
        for s in bld['add_spots']:
            keys(s, ['id', 'label', 'description'])
            ident(s['id']); text(s['label'], 48); text(s['description'], 240)
    if 'open_entrance' in bld: ident(bld['open_entrance'])
    if 'add_link' in bld:
        assert isinstance(bld['add_link'], list) and len(bld['add_link']) <= 4
        for l in bld['add_link']:
            keys(l, ['place', 'distance'], ['direction'])
            ident(l['place']); uint(l['distance'], 0, 1000000000)
            if 'direction' in l:
                assert l['direction'] in ['north', 'northeast', 'east', 'southeast', 'south', 'southwest', 'west', 'northwest', 'up', 'down']
    if 'patch_place' in bld:
        pp = bld['patch_place']
        keys(pp, [], ['kind', 'add_tags'])
        if 'kind' in pp: ident(pp['kind'])
        if 'add_tags' in pp:
            assert isinstance(pp['add_tags'], list) and len(pp['add_tags']) <= 8
            for t in pp['add_tags']: ident(t)


def validate_recipe(r):
    keys(r, ['id', 'label', 'category', 'inputs', 'tools', 'outputs', 'time'],
         ['station', 'skill', 'building'])
    ident(r['id']); text(r['label'], 48)
    assert r['category'] in CATEGORIES
    assert isinstance(r['inputs'], list) and len(r['inputs']) <= 16
    for inp in r['inputs']: validate_input(inp)
    assert isinstance(r['tools'], list) and len(r['tools']) <= 8
    t_funcs = set()
    for tl in r['tools']:
        validate_tool(tl)
        assert tl['function'] not in t_funcs
        t_funcs.add(tl['function'])
    if 'station' in r: ident(r['station'])
    if 'skill' in r: validate_skill(r['skill'])
    assert isinstance(r['outputs'], list) and len(r['outputs']) <= 16
    for out in r['outputs']: validate_output(out)
    uint(r['time'], 1, 1000000000)
    if 'building' in r:
        assert r['category'] == 'building', "building specs only on building recipes"
        validate_building(r['building'])


class Recipes:
    def __init__(self, recipes=None):
        self.recipes = {}
        if recipes is not None:
            assert isinstance(recipes, list) and len(recipes) <= MAX_RECIPES
            for r in recipes:
                validate_recipe(r)
                assert r['id'] not in self.recipes
                self.recipes[r['id']] = copy.deepcopy(r)

    def to_value(self):
        return list(self.recipes.values())

    def check(self, recipe_id, actor, place, things_state, bio=None, target_thing=None):
        assert recipe_id in self.recipes
        r = self.recipes[recipe_id]
        missing = []

        # 1. Check skill if required
        if r.get('skill'):
            req_sk = r['skill']
            actor_skills = {}
            if bio and isinstance(bio.get('skills'), list):
                for sk in bio['skills']:
                    actor_skills[sk['id']] = sk.get('level', 0)
            actual_lvl = actor_skills.get(req_sk['id'], 0)
            if actual_lvl < req_sk['level']:
                missing.append(dict(reason="missing_skill", details=dict(skill=req_sk['id'], required=req_sk['level'], actual=actual_lvl)))

        # 2. Check tools: actor must hold tool with function of power >= req
        things_list = things_state.get('things', []) if things_state else []
        kinds_map = {k['id']: k for k in things_state.get('kinds', [])} if things_state else {}

        for tl in r['tools']:
            req_fn = tl['function']
            req_pwr = tl['power']
            has_tool = False
            for t in things_list:
                loc = t.get('location', {})
                if loc.get('holder') == actor:
                    kd = kinds_map.get(t['kind'], {})
                    for f in kd.get('functions', []):
                        if f['function'] == req_fn and f['power'] >= req_pwr:
                            has_tool = True
                            break
                if has_tool: break
            if not has_tool:
                missing.append(dict(reason="missing_tool", details=dict(function=req_fn, required_power=req_pwr)))

        # 3. Check station in place if required
        if r.get('station'):
            req_st = r['station']
            has_station = False
            for t in things_list:
                loc = t.get('location', {})
                if loc.get('place') == place:
                    kd = kinds_map.get(t['kind'], {})
                    for f in kd.get('functions', []):
                        if f['function'] == req_st:
                            has_station = True
                            break
                if has_station: break
            if not has_station:
                missing.append(dict(reason="missing_station", details=dict(station=req_st)))

        # 4. Check inputs available (held by actor or in place)
        available_things = []
        for t in things_list:
            loc = t.get('location', {})
            if loc.get('holder') == actor or loc.get('place') == place:
                available_things.append(t)

        for inp in r['inputs']:
            req_qty = inp['quantity']
            matched_qty = 0
            if 'kind' in inp:
                k_target = inp['kind']
                for t in available_things:
                    if t['kind'] == k_target and t['id'] != target_thing:
                        matched_qty += t.get('quantity', 1)
                if matched_qty < req_qty:
                    missing.append(dict(reason="missing_input", details=dict(kind=k_target, required=req_qty, available=matched_qty)))
            elif 'tag' in inp:
                t_target = inp['tag']
                for t in available_things:
                    if t['id'] != target_thing:
                        kd = kinds_map.get(t['kind'], {})
                        if t_target in kd.get('tags', []):
                            matched_qty += t.get('quantity', 1)
                if matched_qty < req_qty:
                    missing.append(dict(reason="missing_input", details=dict(tag=t_target, required=req_qty, available=matched_qty)))

        # 5. Target item check for repair or dismantle
        if r['category'] in ['repairing', 'dismantling']:
            if not target_thing:
                missing.append(dict(reason="missing_target", details=dict(category=r['category'])))
            else:
                found_target = None
                for t in things_list:
                    if t['id'] == target_thing:
                        found_target = t
                        break
                if not found_target:
                    missing.append(dict(reason="target_not_found", details=dict(thing=target_thing)))
                else:
                    loc = found_target.get('location', {})
                    if loc.get('holder') != actor and loc.get('place') != place:
                        missing.append(dict(reason="target_not_accessible", details=dict(thing=target_thing)))
                    elif r['category'] == 'repairing' and found_target.get('condition', 100) >= 100:
                        missing.append(dict(reason="target_already_repaired", details=dict(condition=found_target.get('condition', 100))))

        return dict(feasible=len(missing) == 0, missing=missing, time=r['time'])

    def execute(self, at, recipe_id, actor, place, things_state, bio=None, codex=None, target_thing=None, new_ids=None):
        chk = self.check(recipe_id, actor, place, things_state, bio=bio, target_thing=target_thing)
        if not chk['feasible']:
            return dict(ok=False, missing=chk['missing'])

        r = self.recipes[recipe_id]
        things_list = things_state.setdefault('things', [])
        kinds_map = {k['id']: k for k in things_state.get('kinds', [])}

        # 1. Consume inputs
        for inp in r['inputs']:
            req_qty = inp['quantity']
            rem_needed = req_qty
            for t in list(things_list):
                if rem_needed == 0: break
                loc = t.get('location', {})
                if loc.get('holder') == actor or loc.get('place') == place:
                    if t['id'] == target_thing: continue
                    is_match = False
                    if 'kind' in inp and t['kind'] == inp['kind']: is_match = True
                    elif 'tag' in inp and inp['tag'] in kinds_map.get(t['kind'], {}).get('tags', []): is_match = True
                    if is_match:
                        t_qty = t.get('quantity', 1)
                        if t_qty <= rem_needed:
                            rem_needed -= t_qty
                            things_list.remove(t)
                        else:
                            t['quantity'] = t_qty - rem_needed
                            rem_needed = 0

        created_things = []
        # 2. Category specific execution
        if r['category'] == 'repairing':
            for t in things_list:
                if t['id'] == target_thing:
                    t['condition'] = 100
                    break
        elif r['category'] == 'dismantling':
            for t in list(things_list):
                if t['id'] == target_thing:
                    things_list.remove(t)
                    break
            # Outputs spawn in place or holder
            id_idx = 0
            for out in r['outputs']:
                out_id = new_ids[id_idx] if new_ids and id_idx < len(new_ids) else f"{out['kind']}_{at}_{id_idx}"
                id_idx += 1
                cond = out.get('condition', 100)
                new_t = dict(id=out_id, kind=out['kind'], location=dict(place=place), condition=cond, owner=actor, quantity=out['quantity'], passengers=[])
                things_list.append(new_t)
                created_things.append(new_t)
        elif r['category'] == 'building':
            bld = r.get('building', {})
            if 'structure_kind' in bld:
                out_id = new_ids[0] if new_ids and len(new_ids) > 0 else f"{bld['structure_kind']}_{place}"
                new_t = dict(id=out_id, kind=bld['structure_kind'], location=dict(place=place), condition=100, owner=actor, quantity=1, passengers=[])
                things_list.append(new_t)
                created_things.append(new_t)
            # World patches to codex if present
            if codex and isinstance(codex.get('places'), list):
                for p in codex['places']:
                    if p['id'] == place:
                        if 'add_spots' in bld:
                            p.setdefault('spots', []).extend(copy.deepcopy(bld['add_spots']))
                        if 'add_link' in bld:
                            p.setdefault('links', []).extend(copy.deepcopy(bld['add_link']))
                        if 'patch_place' in bld:
                            pp = bld['patch_place']
                            if 'kind' in pp: p['kind'] = pp['kind']
                            if 'add_tags' in pp:
                                cur_tags = p.setdefault('tags', [])
                                for tg in pp['add_tags']:
                                    if tg not in cur_tags: cur_tags.append(tg)
                        break
            if codex and 'open_entrance' in bld and isinstance(codex.get('entrances'), list):
                for e in codex['entrances']:
                    if e['id'] == bld['open_entrance']:
                        e['status'] = 'open'
                        break
        else: # crafting or cooking
            id_idx = 0
            for out in r['outputs']:
                out_id = new_ids[id_idx] if new_ids and id_idx < len(new_ids) else f"{out['kind']}_{at}_{id_idx}"
                id_idx += 1
                cond = out.get('condition', 100)
                new_t = dict(id=out_id, kind=out['kind'], location=dict(holder=actor), condition=cond, owner=actor, quantity=out['quantity'], passengers=[])
                things_list.append(new_t)
                created_things.append(new_t)

        things_involved = [t['id'] for t in created_things]
        if target_thing: things_involved.append(target_thing)
        summary = f"{actor} completed {r['label']} ({r['category']})."
        entry = dict(
            at=at, kind=f"{r['category']}_recipe", summary=summary,
            actors=[actor], place=place, things=things_involved,
            tags=["recipes", r['category']], visibility="public",
            witnesses=[], phrases=[r['label'], recipe_id],
            facts=dict(recipe=recipe_id, time=r['time'])
        )
        return dict(ok=True, recipe=recipe_id, created=created_things, entry=entry)

    def workflow(self, req):
        assert isinstance(req, dict)
        keys(req, ['ops'], ['recipes', 'things', 'codex', 'bio'])
        ops = req['ops']
        assert isinstance(ops, list) and len(ops) <= MAX_OPS
        recipes_val = req.get('recipes')
        if recipes_val is not None:
            self.__init__(recipes_val)
        things_state = req.get('things', dict(version=1, materials=[], kinds=[], things=[]))
        codex_state = req.get('codex')
        bio_state = req.get('bio')

        results = []
        for o in ops:
            keys(o, ['op'], ['args', 'at'])
            op = o['op']
            args = o.get('args', {})
            at = uint(o.get('at', 0))

            # Atomic backup of state
            backup_things = copy.deepcopy(things_state)
            backup_codex = copy.deepcopy(codex_state)
            try:
                if op == 'check':
                    keys(args, ['recipe', 'actor', 'place'], ['target_thing'])
                    res = self.check(args['recipe'], args['actor'], args['place'], things_state, bio=bio_state, target_thing=args.get('target_thing'))
                    results.append(dict(ok=True, result=res))
                elif op == 'execute':
                    keys(args, ['recipe', 'actor', 'place'], ['target_thing', 'new_ids'])
                    res = self.execute(at, args['recipe'], args['actor'], args['place'], things_state, bio=bio_state, codex=codex_state, target_thing=args.get('target_thing'), new_ids=args.get('new_ids'))
                    if res['ok']:
                        results.append(dict(ok=True, result=res))
                    else:
                        things_state = backup_things
                        codex_state = backup_codex
                        results.append(dict(ok=False, missing=res['missing']))
                else:
                    assert False, f"unknown op {op}"
            except Exception:
                things_state = backup_things
                codex_state = backup_codex
                results.append(dict(ok=False))

        return dict(results=results, recipes=self.to_value(), things=things_state, codex=codex_state)


def workflow(a):
    assert isinstance(a, dict)
    keys(a, ['ops'], ['recipes', 'things', 'codex', 'bio'])
    rec = Recipes(a.get('recipes'))
    return rec.workflow(a)
