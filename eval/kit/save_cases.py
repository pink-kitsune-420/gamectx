"""Game data and save file cases with predeclared expectations; zero model calls (--cap 0).

`--write-conformance` writes conformance/save-file/<case>/ from the reference; otherwise the
Rust CLI (`target/debug/gamectx local -`) must match it byte for byte.
"""
import argparse, copy, subprocess
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
from save_reference import ROOT_DIR, enc, oracle, fingerprint
import roleplay_engine_cases as E

CODEX, SHEET = E.CODEX, E.SHEET
ENGINE = dict(version=1, character=E.CHAR, features=dict(props=dict(registry=E.PROPS)))
LORE = dict(version=1, entries=[dict(id='rye_recipe', subject='bakery', topic='recipes', text='Mira proves her rye overnight in the cellar where it stays cool.', tags=['bread'], known_by=['*'], requires=[])])
MEMORY = dict(next_id=2, records=[dict(id=1, turn=3, importance=6, text='The traveller paid for rye bread with a silver coin.', tags=['trade'])])
FRESH = dict(version=1, greetings=dict(version=1, revision=0, entries=[]), listeners=[], pending=[], asked=[])
HEARD = dict(FRESH, listeners=[dict(id='player_1', name='Arin', preferences=[dict(value='rye bread', polarity='like')], recent=['Arin; thank you for telling me your name.'], topic=None, revealed=['flour_debt'])])
DATA = [dict(path='world/codex', kind='codex', data=CODEX), dict(path='characters/baker/sheet', kind='sheet', data=SHEET),
        dict(path='characters/baker/engine', kind='engine-config', data=ENGINE), dict(path='world/props', kind='props', data=E.PROPS),
        dict(path='host/settings', kind='host', data=dict(difficulty='gentle', music=0.6))]
HISTORY = dict(version=1, next=2, entries=[dict(seq=1, at=38, kind='trade', summary='The traveller bought rye bread with a silver coin.', actors=['player_1', 'baker'],
                                                 place='bakery', things=[], tags=['trade'], visibility='witnessed', witnesses=[], phrases=['the silver coin'], facts={},
                                                 retracted=None, covers=None)])
THINGS_DATA = dict(version=1, materials=[dict(id='iron', label='Iron')],
                   kinds=[dict(id='sword', label='Sword', category='weapon', materials=[dict(material='iron', amount=1)], functions=[], weight=5, base_value=10, durability=100, stackable=False)],
                   things=[dict(id='s1', kind='sword', location=dict(place='forge'), condition=100, quantity=1)])
SLOT = [dict(path='characters/baker/engine', kind='engine', data=HEARD), dict(path='characters/baker/memory', kind='memory', data=MEMORY),
        dict(path='world/flags', kind='host', data=dict(ring_lost=True))]


def op(op_name, **a): return dict(op=op_name, args=a)
def pack(sections=DATA, title='Ember Bakery'): return op('pack_data', title=title, sections=sections)
def save(name='slot-1', saved_at=40, sections=SLOT, **kw): return op('save', name=name, saved_at=saved_at, sections=sections, **kw)


def built():
    r = oracle(dict(ops=[pack(), op('link'), save(label='Before the cellar'), save('autosave', 44, SLOT[1:]), op('fork', **{'from': 'slot-1'}, name='slot-2', saved_at=41), op('export_save')]))
    return r['results'][0]['ok'], r['results'][-1]['ok']


def cases():
    data, sv = built()
    tampered = copy.deepcopy(sv); tampered['states'][0]['sections'][0]['data']['next_id'] = 9
    stale = copy.deepcopy(data); stale['title'] = 'Ember Bakery 1.1'
    stale['digest'] = oracle(dict(ops=[pack(title='Ember Bakery 1.1')]))['results'][0]['ok']['digest']
    bad_engine = copy.deepcopy(SLOT); bad_engine[0] = dict(bad_engine[0], data=dict(HEARD, listeners=[dict(HEARD['listeners'][0], id='baker')]))
    other = copy.deepcopy(ENGINE); other['character'] = dict(id='miller', name='Ilen')
    sheet_unknown = copy.deepcopy(SHEET); sheet_unknown['factions'] = [dict(faction='millers_union')]
    # (name, args, predeclared expectation: list of (result index, field subset of ok/error))
    return [
        ('pack_canonical', dict(ops=[pack(), pack(list(reversed(DATA)))]), [(0, dict(ok=dict(digest=data['digest']))), (1, dict(ok=dict(digest=data['digest'])))]),
        ('pack_and_link', dict(ops=[pack(), op('link')]), [(1, dict(ok=dict(game=dict(title='Ember Bakery', digest=data['digest']))))]),
        ('slots_and_fork', dict(ops=[pack(), op('link'), save(label='Before the cellar'), save('autosave', 44, SLOT[1:]), op('fork', **{'from': 'slot-1'}, name='slot-2', saved_at=41), op('export_save')]),
         [(4, dict(ok=dict(name='slot-2', parent='slot-1', saved_at=41)))]),
        ('fork_keeps_sections', dict(ops=[save(), op('fork', **{'from': 'slot-1'}, name='slot-2', saved_at=41)]), []),
        ('save_replaces_slot', dict(ops=[save(), save(saved_at=50, sections=SLOT[1:]), op('export_save')]), []),
        ('remove_state', dict(ops=[save(), save('autosave', 44), op('remove_state', name='slot-1'), op('remove_state', name='slot-1')]),
         [(2, dict(ok=dict(states=['autosave']))), (3, dict(error='remove_state'))]),
        ('load_round_trip', dict(ops=[op('load_data', **data), op('load_save', **sv), op('export_save')]), [(1, dict(ok=dict(game_matches=True)))]),
        ('load_save_alone', dict(ops=[op('load_save', **sv)]), [(0, dict(ok=dict(game_matches=None)))]),
        ('load_against_changed_game', dict(ops=[op('load_data', **stale), op('load_save', **sv)]), [(1, dict(ok=dict(game_matches=False)))]),
        ('load_tampered_rejected', dict(ops=[op('load_save', **tampered)]), [(0, dict(error='load_save'))]),
        ('load_data_digest_mismatch', dict(ops=[op('load_data', **dict(data, digest='0000000000000000'))]), [(0, dict(error='load_data'))]),
        ('engine_state_must_fit_config', dict(ops=[pack(), save(sections=bad_engine), save()]), [(1, dict(error='save')), (2, dict(ok=dict(name='slot-1')))]),
        ('engine_state_without_config', dict(ops=[pack(DATA[:2]), save()]), [(1, dict(error='save'))]),
        ('put_and_remove_data', dict(ops=[pack(), op('put_data', path='world/lore', kind='lore', data=LORE), op('remove_data', path='world/lore'), op('remove_data', path='world/codex')]),
         [(2, dict(ok=dict(digest=data['digest']))), (3, dict(error='remove_data'))]),
        ('bad_path', dict(ops=[pack([dict(path='world', kind='host', data={})]), pack([dict(path='characters/baker', kind='host', data={})]),
                               pack([dict(path='world/sheet', kind='sheet', data=SHEET)]), pack([dict(path='elsewhere/x', kind='host', data={})]),
                               pack([dict(path='world/a b', kind='host', data={})])]), [(i, dict(error='pack_data')) for i in range(5)]),
        ('bad_kind', dict(ops=[pack([dict(path='world/x', kind='engine', data=FRESH)]), save(sections=[dict(path='world/x', kind='codex', data=CODEX)])]),
         [(0, dict(error='pack_data')), (1, dict(error='save'))]),
        ('bad_codex', dict(ops=[pack([dict(path='world/codex', kind='codex', data=dict(CODEX, version=2))])]), [(0, dict(error='pack_data'))]),
        ('bad_sheet_faction', dict(ops=[pack([DATA[0], dict(DATA[1], data=sheet_unknown)])]), [(0, dict(error='pack_data'))]),
        ('bad_engine_character', dict(ops=[pack([dict(path='characters/baker/engine', kind='engine-config', data=other)])]), [(0, dict(error='pack_data'))]),
        ('bad_duplicate_path', dict(ops=[pack([DATA[4], DATA[4]])]), [(0, dict(error='pack_data'))]),
        ('bad_state_meta', dict(ops=[save(name='bad name'), save(parent='slot-1'), save(label=''), save(saved_at=-1), op('fork', **{'from': 'nope'}, name='x', saved_at=1)]),
         [(i, dict(error=o)) for i, o in enumerate(['save', 'save', 'save', 'save', 'fork'])]),
        ('timeline_section', dict(ops=[save(sections=SLOT + [dict(path='world/timeline', kind='timeline', data=HISTORY)]), op('export_save')]), [(0, dict(ok=dict(name='slot-1')))]),
        ('bad_timeline_section', dict(ops=[save(sections=[dict(path='world/timeline', kind='timeline', data=dict(HISTORY, next=1))])]), [(0, dict(error='save'))]),
        ('things_section', dict(ops=[save(sections=SLOT + [dict(path='world/things', kind='things', data=THINGS_DATA)]), op('export_save')]), [(0, dict(ok=dict(name='slot-1')))]),
        ('bad_things_section', dict(ops=[save(sections=[dict(path='world/things', kind='things', data=dict(THINGS_DATA, version=2))])]), [(0, dict(error='save'))]),
        ('fork_name_taken', dict(ops=[save(), save('autosave', 44), op('fork', **{'from': 'slot-1'}, name='autosave', saved_at=45)]), [(2, dict(error='fork'))]),
    ]


def check_extra(rows):
    """Expectations 2 and 3, checked on the reference outputs."""
    by = {n: oracle(copy.deepcopy(a)) for n, a, _ in rows}
    s1, s2 = [r['ok'] for r in by['fork_keeps_sections']['results']]
    assert s1['sections'] == s2['sections'] and s1['digest'] != s2['digest']
    a, b = [r['ok'] for r in by['save_replaces_slot']['results'][:2]]
    assert a['digest'] != b['digest'] and [s['name'] for s in by['save_replaces_slot']['results'][2]['ok']['states']] == ['slot-1']
    out = by['slots_and_fork']['results'][-1]['ok']
    assert [s['name'] for s in out['states']] == ['autosave', 'slot-1', 'slot-2'] and out['game']['digest'] == by['pack_and_link']['results'][0]['ok']['digest']


def main():
    p = argparse.ArgumentParser(); p.add_argument('--cap', required=True, type=float); p.add_argument('--prototype-only', action='store_true'); p.add_argument('--write-conformance', action='store_true')
    a = p.parse_args(); assert a.cap == 0, 'save file cases are offline only'
    rows = cases(); check_extra(rows)
    for name, args, wants in rows:
        got = oracle(copy.deepcopy(args))
        for i, want in wants:
            have = got['results'][i]
            for k, v in want.items():
                h = have.get(k)
                assert (h == v) if not isinstance(v, dict) else isinstance(h, dict) and {x: h.get(x) for x in v} == v, (name, i, have)
        req = dict(kind='save-file', args=args)
        if not a.prototype_only:
            x = subprocess.run([str(ROOT_DIR / 'target/debug/gamectx'), 'local', '-'], input=enc(req), capture_output=True, text=True)
            assert x.returncode == 0 and x.stdout.strip() == enc(got), (name, x.stderr, x.stdout[:400], enc(got)[:400])
        if a.write_conformance:
            d = ROOT_DIR / 'conformance/save-file' / name; d.mkdir(parents=True, exist_ok=True)
            for k, v in [('case', dict(op='local')), ('input', req), ('expected', got)]:
                f = d / (k + '.json')
                if f.exists(): assert f.read_text() == enc(v) + '\n', f  # existing cases stay byte-identical
                else: f.write_text(enc(v) + '\n')
    print(enc(dict(workflows=len(rows), model_calls=0, api_spend_usd=0)))


if __name__ == '__main__':
    main()
