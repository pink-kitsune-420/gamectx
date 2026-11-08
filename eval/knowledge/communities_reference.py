"""Independent original communities reference: factions extension, home places,
members, customs (norms), relations with numeric trust, markets, and bartering.
Python standard library only; nothing here calls a model.
"""
import copy, json, re

MAX_COMMUNITIES = 64
MAX_MEMBERS = 64
MAX_CUSTOMS = 32
MAX_RELATIONS = 64
MAX_RULES = 32
MAX_OPS = 64
MAX_SAFE = 9007199254740991

STANCES = ['allied', 'friendly', 'neutral', 'wary', 'hostile']
STANCE_DEFAULTS = {
    'allied':   dict(trust=75,  buy_mult=1100, sell_mult=800),
    'friendly': dict(trust=50,  buy_mult=1050, sell_mult=900),
    'neutral':  dict(trust=0,   buy_mult=1000, sell_mult=1000),
    'wary':     dict(trust=-25, buy_mult=900,  sell_mult=1250),
    'hostile':  dict(trust=-75, buy_mult=500,  sell_mult=2000),
}


def enc(v): return json.dumps(v, ensure_ascii=False, separators=(',', ':'))
def control(s): return any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in s)
def text(s, chars):
    assert isinstance(s, str) and 1 <= len(s) <= chars and not control(s); return s
def ident(s): assert isinstance(s, str) and re.fullmatch('[A-Za-z0-9_:-]{1,64}', s); return s
def uint(n, lo=0, hi=MAX_SAFE): assert type(n) is int and lo <= n <= hi; return n
def int_bound(n, lo=-100, hi=100): assert type(n) is int and lo <= n <= hi; return n
def boolean(b): assert type(b) is bool; return b
def keys(v, required, optional=()): assert isinstance(v, dict) and set(required) <= set(v) <= set(required) | set(optional)


def stance_from_trust(trust):
    if trust >= 60: return 'allied'
    if trust >= 25: return 'friendly'
    if trust >= -25: return 'neutral'
    if trust >= -60: return 'wary'
    return 'hostile'


def multipliers_from_trust(trust):
    buy_mult = max(100, min(2000, 1000 + trust * 2))
    sell_mult = max(100, min(3000, 1000 - trust * 2))
    return buy_mult, sell_mult


def validate_rule(r):
    keys(r, [], ['kind', 'tag', 'price_mult'])
    assert ('kind' in r) ^ ('tag' in r)
    if 'kind' in r: ident(r['kind'])
    if 'tag' in r: ident(r['tag'])
    if 'price_mult' in r and r['price_mult'] is not None:
        uint(r['price_mult'], 1, 10000)


def validate_market(m):
    keys(m, [], ['buys', 'sells'])
    if 'buys' in m and m['buys'] is not None:
        assert isinstance(m['buys'], list) and len(m['buys']) <= MAX_RULES
        for r in m['buys']: validate_rule(r)
    if 'sells' in m and m['sells'] is not None:
        assert isinstance(m['sells'], list) and len(m['sells']) <= MAX_RULES
        for r in m['sells']: validate_rule(r)


def validate_relation(rel):
    keys(rel, ['target', 'stance'], ['trust'])
    ident(rel['target'])
    assert rel['stance'] in STANCES
    if 'trust' in rel and rel['trust'] is not None:
        int_bound(rel['trust'], -100, 100)


def validate_community(c):
    keys(c, ['id', 'label', 'description'],
         ['home_places', 'members', 'customs', 'relations', 'market'])
    ident(c['id'])
    text(c['label'], 48)
    text(c['description'], 240)
    if 'home_places' in c and c['home_places'] is not None:
        assert isinstance(c['home_places'], list) and len(c['home_places']) <= 16
        for p in c['home_places']: ident(p)
    if 'members' in c and c['members'] is not None:
        assert isinstance(c['members'], list) and len(c['members']) <= MAX_MEMBERS
        for m in c['members']: ident(m)
    if 'customs' in c and c['customs'] is not None:
        assert isinstance(c['customs'], list) and len(c['customs']) <= MAX_CUSTOMS
        for cust in c['customs']: text(cust, 120)
    if 'relations' in c and c['relations'] is not None:
        assert isinstance(c['relations'], list) and len(c['relations']) <= MAX_RELATIONS
        targets = set()
        for r in c['relations']:
            validate_relation(r)
            assert r['target'] not in targets and r['target'] != c['id']
            targets.add(r['target'])
    if 'market' in c and c['market'] is not None:
        validate_market(c['market'])


class CommunityWorld:
    def __init__(self, communities=None):
        self.communities = {}
        if communities is not None:
            assert isinstance(communities, list) and len(communities) <= MAX_COMMUNITIES
            for c in communities:
                validate_community(c)
                assert c['id'] not in self.communities
                self.communities[c['id']] = copy.deepcopy(c)

    def to_value(self):
        res = []
        for c in self.communities.values():
            item = dict(
                id=c['id'],
                label=c['label'],
                description=c['description']
            )
            if 'home_places' in c and c['home_places'] is not None:
                item['home_places'] = c['home_places']
            if 'members' in c and c['members'] is not None:
                item['members'] = c['members']
            if 'customs' in c and c['customs'] is not None:
                item['customs'] = c['customs']
            if 'relations' in c and c['relations'] is not None:
                rel_list = []
                for r in c['relations']:
                    ro = dict(target=r['target'], stance=r['stance'])
                    if 'trust' in r and r['trust'] is not None:
                        ro['trust'] = r['trust']
                    rel_list.append(ro)
                item['relations'] = rel_list
            if 'market' in c and c['market'] is not None:
                mo = {}
                if 'buys' in c['market'] and c['market']['buys'] is not None:
                    buys = []
                    for b in c['market']['buys']:
                        rule = {}
                        if 'kind' in b: rule['kind'] = b['kind']
                        if 'tag' in b: rule['tag'] = b['tag']
                        if 'price_mult' in b and b['price_mult'] is not None:
                            rule['price_mult'] = b['price_mult']
                        buys.append(rule)
                    mo['buys'] = buys
                if 'sells' in c['market'] and c['market']['sells'] is not None:
                    sells = []
                    for s in c['market']['sells']:
                        rule = {}
                        if 'kind' in s: rule['kind'] = s['kind']
                        if 'tag' in s: rule['tag'] = s['tag']
                        if 'price_mult' in s and s['price_mult'] is not None:
                            rule['price_mult'] = s['price_mult']
                        sells.append(rule)
                    mo['sells'] = sells
                item['market'] = mo
            res.append(item)
        return res

    def get_community(self, cid):
        return self.communities.get(cid)

    def get_relation_info(self, cid, target):
        comm = self.get_community(cid)
        if comm is None:
            return dict(stance='neutral', trust=0, buy_mult=1000, sell_mult=1000)
        relations = comm.get('relations') or []
        for r in relations:
            if r['target'] == target:
                stance = r['stance']
                if 'trust' in r and r['trust'] is not None:
                    trust = r['trust']
                    buy_mult, sell_mult = multipliers_from_trust(trust)
                else:
                    defs = STANCE_DEFAULTS.get(stance, STANCE_DEFAULTS['neutral'])
                    trust = defs['trust']
                    buy_mult = defs['buy_mult']
                    sell_mult = defs['sell_mult']
                return dict(stance=stance, trust=trust, buy_mult=buy_mult, sell_mult=sell_mult)
        # Default neutral
        return dict(stance='neutral', trust=0, buy_mult=1000, sell_mult=1000)

    def _match_rule(self, rules, thing, kind_def):
        if not rules:
            return None
        t_kind = thing.get('kind')
        k_tags = kind_def.get('tags') or []
        k_cat = kind_def.get('category')
        for r in rules:
            if 'kind' in r and r['kind'] == t_kind:
                return r
            if 'tag' in r and (r['tag'] in k_tags or r['tag'] == k_cat):
                return r
        return None

    def _calc_item_value(self, thing, kind_def, rule, rel_mult, is_buy):
        base_val = kind_def.get('base_value', 0)
        condition = thing.get('condition', 100)
        quantity = thing.get('quantity', 1)
        nominal = (base_val * condition // 100) * quantity
        if nominal == 0:
            return 0
        rule_mult = rule.get('price_mult', 1000) if rule else 1000
        # effective = nominal * rule_mult // 1000 * rel_mult // 1000
        val = (nominal * rule_mult * rel_mult) // 1000000
        if val == 0 and nominal > 0:
            val = 1
        return val

    def op_check_relation(self, community, target):
        ident(community); ident(target)
        comm = self.get_community(community)
        assert comm is not None
        info = self.get_relation_info(community, target)
        return dict(
            ok=True,
            community=community,
            target=target,
            stance=info['stance'],
            trust=info['trust'],
            buy_mult=info['buy_mult'],
            sell_mult=info['sell_mult'],
        )

    def op_set_relation(self, at, community, target, stance=None, trust=None, place=None):
        ident(community); ident(target)
        assert community != target
        if place is not None: ident(place)
        comm = self.get_community(community)
        assert comm is not None
        if stance is None and trust is None:
            stance = 'neutral'; trust = 0
        elif stance is None:
            int_bound(trust, -100, 100)
            stance = stance_from_trust(trust)
        elif trust is None:
            assert stance in STANCES
            trust = STANCE_DEFAULTS[stance]['trust']
        else:
            assert stance in STANCES
            int_bound(trust, -100, 100)

        if 'relations' not in comm or comm['relations'] is None:
            comm['relations'] = []
        found = False
        for r in comm['relations']:
            if r['target'] == target:
                r['stance'] = stance
                r['trust'] = trust
                found = True
                break
        if not found:
            assert len(comm['relations']) < MAX_RELATIONS
            comm['relations'].append(dict(target=target, stance=stance, trust=trust))

        entry = dict(
            at=at,
            kind="relation_change",
            summary=f"{community} relation to {target} is now {stance} (trust: {trust}).",
            actors=sorted([community, target]),
            place=place or "",
            things=[],
            tags=["community", "relation", stance],
            visibility="public",
            witnesses=[],
            phrases=[community, target, stance],
            facts=dict(community=community, target=target, stance=stance, trust=trust)
        )
        return dict(ok=True, community=community, target=target, stance=stance, trust=trust, entry=entry)

    def op_add_member(self, at, community, actor, place=None):
        ident(community); ident(actor)
        if place is not None: ident(place)
        comm = self.get_community(community)
        assert comm is not None
        if 'members' not in comm or comm['members'] is None:
            comm['members'] = []
        if actor not in comm['members']:
            assert len(comm['members']) < MAX_MEMBERS
            comm['members'].append(actor)
        entry = dict(
            at=at,
            kind="community_member",
            summary=f"{actor} joined {community}.",
            actors=sorted([actor, community]),
            place=place or "",
            things=[],
            tags=["community", "member", "joined"],
            visibility="public",
            witnesses=[],
            phrases=[community, actor, "joined"],
            facts=dict(community=community, actor=actor, action="join")
        )
        return dict(ok=True, community=community, actor=actor, entry=entry)

    def op_remove_member(self, at, community, actor, place=None):
        ident(community); ident(actor)
        if place is not None: ident(place)
        comm = self.get_community(community)
        assert comm is not None
        if 'members' in comm and comm['members'] is not None and actor in comm['members']:
            comm['members'].remove(actor)
        entry = dict(
            at=at,
            kind="community_member",
            summary=f"{actor} left {community}.",
            actors=sorted([actor, community]),
            place=place or "",
            things=[],
            tags=["community", "member", "left"],
            visibility="public",
            witnesses=[],
            phrases=[community, actor, "left"],
            facts=dict(community=community, actor=actor, action="leave")
        )
        return dict(ok=True, community=community, actor=actor, entry=entry)

    def op_set_customs(self, at, community, customs, place=None):
        ident(community)
        if place is not None: ident(place)
        assert isinstance(customs, list) and len(customs) <= MAX_CUSTOMS
        for cust in customs: text(cust, 120)
        comm = self.get_community(community)
        assert comm is not None
        comm['customs'] = copy.deepcopy(customs)
        entry = dict(
            at=at,
            kind="community_customs",
            summary=f"{community} updated its customs.",
            actors=[community],
            place=place or "",
            things=[],
            tags=["community", "customs"],
            visibility="public",
            witnesses=[],
            phrases=[community, "customs"],
            facts=dict(community=community, customs_count=len(customs))
        )
        return dict(ok=True, community=community, customs=customs, entry=entry)

    def _evaluate_trade(self, community, actor, offer_things, request_things, things_state, trader=None):
        ident(community); ident(actor)
        if trader is not None: ident(trader)
        comm = self.get_community(community)
        assert comm is not None
        assert isinstance(offer_things, list) and isinstance(request_things, list)
        assert len(offer_things) <= 16 and len(request_things) <= 16
        for t in offer_things: ident(t)
        for t in request_things: ident(t)
        assert len(offer_things) > 0 and len(request_things) > 0
        # No duplicates
        assert len(set(offer_things)) == len(offer_things)
        assert len(set(request_things)) == len(request_things)
        assert len(set(offer_things) & set(request_things)) == 0

        things_map = {t['id']: t for t in things_state.get('things', [])}
        kinds_map = {k['id']: k for k in things_state.get('kinds', [])}

        rel = self.get_relation_info(community, actor)
        if rel['trust'] < -40 or rel['stance'] == 'hostile':
            return dict(ok=False, reason="hostile_relation", offer_value=0, request_value=0)

        market = comm.get('market') or {}
        buys = market.get('buys') or []
        sells = market.get('sells') or []

        # Evaluate offer items
        offer_value = 0
        for tid in offer_things:
            t = things_map.get(tid)
            if t is None:
                return dict(ok=False, reason="offer_thing_not_found", details=tid)
            # Must be owned by actor or held by actor
            loc = t.get('location') or {}
            if loc.get('holder') != actor and t.get('owner') != actor:
                return dict(ok=False, reason="offer_thing_not_held", details=tid)
            k = kinds_map.get(t['kind'])
            if k is None:
                return dict(ok=False, reason="kind_not_found", details=t['kind'])
            rule = self._match_rule(buys, t, k)
            if buys and rule is None:
                return dict(ok=False, reason="unwanted_goods", details=tid)
            val = self._calc_item_value(t, k, rule, rel['buy_mult'], is_buy=True)
            offer_value += val

        # Evaluate requested items
        request_value = 0
        for tid in request_things:
            t = things_map.get(tid)
            if t is None:
                return dict(ok=False, reason="request_thing_not_found", details=tid)
            # Must be owned by community or trader, or held by trader
            loc = t.get('location') or {}
            valid_owner = (t.get('owner') == community) or (trader and (t.get('owner') == trader or loc.get('holder') == trader))
            if not valid_owner:
                return dict(ok=False, reason="request_thing_not_available", details=tid)
            k = kinds_map.get(t['kind'])
            if k is None:
                return dict(ok=False, reason="kind_not_found", details=t['kind'])
            rule = self._match_rule(sells, t, k)
            val = self._calc_item_value(t, k, rule, rel['sell_mult'], is_buy=False)
            request_value += val

        if offer_value >= request_value:
            return dict(ok=True, acceptable=True, offer_value=offer_value, request_value=request_value)

        # Insufficient value -> generate counter offer
        deficit = request_value - offer_value
        counter_offer = dict(deficit=deficit)

        # Look in actor's other held items for something that covers the deficit
        cand_found = None
        for t in things_state.get('things', []):
            if t['id'] in offer_things: continue
            loc = t.get('location') or {}
            if loc.get('holder') == actor or t.get('owner') == actor:
                k = kinds_map.get(t['kind'])
                if k is None: continue
                rule = self._match_rule(buys, t, k)
                if buys and rule is None: continue
                val = self._calc_item_value(t, k, rule, rel['buy_mult'], is_buy=True)
                if val >= deficit:
                    cand_found = t['id']
                    break
        if cand_found:
            counter_offer['add_offer'] = [cand_found]
        elif len(request_things) > 1:
            # Suggest removing the highest value requested thing
            max_tid = None
            max_v = -1
            for tid in request_things:
                t = things_map[tid]
                k = kinds_map[t['kind']]
                rule = self._match_rule(sells, t, k)
                v = self._calc_item_value(t, k, rule, rel['sell_mult'], is_buy=False)
                if v > max_v:
                    max_v = v
                    max_tid = tid
            if max_tid:
                counter_offer['remove_request'] = [max_tid]

        return dict(
            ok=False,
            acceptable=False,
            reason="insufficient_value",
            offer_value=offer_value,
            request_value=request_value,
            counter_offer=counter_offer
        )

    def op_appraise(self, community, actor, offer_things, request_things, things_state, trader=None):
        res = self._evaluate_trade(community, actor, offer_things, request_things, things_state, trader)
        if res.get('ok') and res.get('acceptable'):
            diff = res['offer_value'] - res['request_value']
            return dict(
                ok=True,
                acceptable=True,
                offer_value=res['offer_value'],
                request_value=res['request_value'],
                difference=diff
            )
        else:
            out = dict(
                ok=True,
                acceptable=False,
                reason=res.get('reason', 'unacceptable'),
                offer_value=res.get('offer_value', 0),
                request_value=res.get('request_value', 0)
            )
            if 'counter_offer' in res:
                out['counter_offer'] = res['counter_offer']
            if 'details' in res:
                out['details'] = res['details']
            return out

    def op_barter(self, at, community, actor, place, offer_things, request_things, things_state, trader=None):
        ident(place)
        eval_res = self._evaluate_trade(community, actor, offer_things, request_things, things_state, trader)
        if not (eval_res.get('ok') and eval_res.get('acceptable')):
            out = dict(
                ok=False,
                reason=eval_res.get('reason', 'trade_refused'),
                offer_value=eval_res.get('offer_value', 0),
                request_value=eval_res.get('request_value', 0),
            )
            if 'counter_offer' in eval_res:
                out['counter_offer'] = eval_res['counter_offer']
            if 'details' in eval_res:
                out['details'] = eval_res['details']
            return out

        # Atomic transfer
        target_owner_for_offer = trader if trader else community
        target_loc_for_offer = dict(holder=trader) if trader else dict(place=place)

        for t in things_state.get('things', []):
            if t['id'] in offer_things:
                t['owner'] = target_owner_for_offer
                t['location'] = copy.deepcopy(target_loc_for_offer)
            elif t['id'] in request_things:
                t['owner'] = actor
                t['location'] = dict(holder=actor)

        # Generous trade trust boost
        ov = eval_res['offer_value']
        rv = eval_res['request_value']
        if ov >= rv * 12 // 10 and ov - rv >= 10:
            comm = self.get_community(community)
            if comm:
                if 'relations' not in comm or comm['relations'] is None:
                    comm['relations'] = []
                for r in comm['relations']:
                    if r['target'] == actor:
                        cur = r.get('trust', STANCE_DEFAULTS.get(r['stance'], {}).get('trust', 0))
                        r['trust'] = min(100, cur + 2)
                        r['stance'] = stance_from_trust(r['trust'])
                        break

        entry = dict(
            at=at,
            kind="barter",
            summary=f"{actor} bartered with {trader or community} at {place}.",
            actors=sorted(list(set([actor, trader or community]))),
            place=place,
            things=sorted(offer_things + request_things),
            tags=["community", "barter", "trade"],
            visibility="public",
            witnesses=[],
            phrases=[community, actor, "barter"],
            facts=dict(
                community=community,
                offer_value=ov,
                request_value=rv,
                offered=offer_things,
                requested=request_things
            )
        )
        return dict(
            ok=True,
            offer_value=ov,
            request_value=rv,
            transferred_offer=offer_things,
            transferred_request=request_things,
            entry=entry
        )

    def workflow(self, req):
        keys(req, ['ops'], ['communities', 'things'])
        ops = req['ops']
        assert isinstance(ops, list) and len(ops) <= MAX_OPS

        if 'communities' in req:
            comm_list = req['communities']
            self.communities = {}
            if comm_list is not None:
                assert isinstance(comm_list, list) and len(comm_list) <= MAX_COMMUNITIES
                for c in comm_list:
                    validate_community(c)
                    assert c['id'] not in self.communities
                    self.communities[c['id']] = copy.deepcopy(c)

        things_state = req.get('things')
        if things_state is None:
            things_state = dict(version=1, materials=[], kinds=[], things=[])
        else:
            things_state = copy.deepcopy(things_state)

        results = []
        for op_call in ops:
            keys(op_call, ['op'], ['at', 'args'])
            op = op_call['op']
            at = uint(op_call.get('at', 0), 0, MAX_SAFE)
            args = op_call.get('args') or {}
            assert isinstance(args, dict)

            # Snapshots for rollback
            comm_snapshot = copy.deepcopy(self.communities)
            things_snapshot = copy.deepcopy(things_state)

            if op == 'check_relation':
                keys(args, ['community', 'target'])
                res = self.op_check_relation(args['community'], args['target'])
                results.append(res)
            elif op == 'set_relation':
                keys(args, ['community', 'target'], ['stance', 'trust', 'place'])
                res = self.op_set_relation(
                    at, args['community'], args['target'],
                    args.get('stance'), args.get('trust'), args.get('place')
                )
                results.append(res)
            elif op == 'add_member':
                keys(args, ['community', 'actor'], ['place'])
                res = self.op_add_member(at, args['community'], args['actor'], args.get('place'))
                results.append(res)
            elif op == 'remove_member':
                keys(args, ['community', 'actor'], ['place'])
                res = self.op_remove_member(at, args['community'], args['actor'], args.get('place'))
                results.append(res)
            elif op == 'set_customs':
                keys(args, ['community', 'customs'], ['place'])
                res = self.op_set_customs(at, args['community'], args['customs'], args.get('place'))
                results.append(res)
            elif op == 'appraise':
                keys(args, ['community', 'actor', 'offer_things', 'request_things'], ['trader', 'place'])
                res = self.op_appraise(
                    args['community'], args['actor'],
                    args['offer_things'], args['request_things'],
                    things_state, args.get('trader')
                )
                results.append(res)
            elif op == 'barter':
                keys(args, ['community', 'actor', 'place', 'offer_things', 'request_things'], ['trader'])
                try:
                    res = self.op_barter(
                        at, args['community'], args['actor'], args['place'],
                        args['offer_things'], args['request_things'],
                        things_state, args.get('trader')
                    )
                    if not res.get('ok'):
                        # Revert on trade failure
                        self.communities = comm_snapshot
                        things_state = things_snapshot
                    results.append(res)
                except Exception:
                    self.communities = comm_snapshot
                    things_state = things_snapshot
                    raise
            else:
                assert False, f"Unknown op: {op}"

        return dict(
            results=results,
            communities=self.to_value(),
            things=things_state
        )


def workflow(a):
    keys(a, ['ops'], ['communities', 'things'])
    cw = CommunityWorld(a.get('communities'))
    return cw.workflow(a)
