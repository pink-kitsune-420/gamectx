#!/usr/bin/env python3
"""Audit the frozen paired guidance experiment; write/check its derived summary offline."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import provenance
import argparse,collections,hashlib,json,math,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import query_upgrade_eval as u
ROOT=u.r.ROOT

def summarize():
    runs=[json.loads((ROOT/'eval'/'query'/name).read_text()) for name in ('query-guidance-tokens.json','query-guidance-results.json')]
    for token,data in zip((True,False),runs,strict=True):
        assert data['completed'] and not data['stopped'] and data['tokens_only']==token
        assert data['guidance']==u.GUIDANCE and provenance.matches({'eval/query_upgrade_eval.py':data['eval_sha256']})
        assert data['spend_usd']<=data['cap_usd']
        assert math.isclose(sum(c['cost_usd'] for c in data['calls']),data['spend_usd'],abs_tol=1e-12)
        expected={(m,seed,arm,q['question']) for m in u.r.MODELS for family,seed,size in u.FRESH for q in u.r.state(family,seed,size)['questions'][:1 if token else 8] for arm in ('baseline','guided')}
        actual={(x['model'],x['seed'],x['arm'],x['question']) for x in data['rows']}
        assert len(actual)==len(data['rows']) and actual==expected
        assert collections.Counter(json.dumps(c,sort_keys=True) for c in data['calls'])==collections.Counter(json.dumps(c,sort_keys=True) for row in data['rows'] for c in row['calls'])
        states={d['seed']:d for d in data['states']}
        for row in data['rows']:
            question=next(q for q in states[row['seed']]['questions'] if q['question']==row['question']);assert row['expected']==question['expected']
            if token:continue
            attempts=row['attempts'];assert len(attempts) in (1,2)
            executed=bool(attempts and 'result' in attempts[-1]['feedback'] and u.r.correct(row['expected'],attempts[-1]['feedback']['result']))
            assert row['executed_correct']==executed and row['first_correct']==(executed and len(attempts)==1)
            assert row['correct']==u.r.correct(row['expected'],row['answer'])
    tokens,data=runs
    def stats(rows):
        calls=[c for row in rows for c in row['calls']]
        return dict(exchanges=len(rows),first_correct=sum(x['first_correct'] for x in rows),executed_correct=sum(x['executed_correct'] for x in rows),final_correct=sum(x['correct'] for x in rows),average_input=sum(c['usage']['prompt_tokens'] for c in calls)/len(rows),average_output=sum(c['usage']['completion_tokens'] for c in calls)/len(rows),spend_usd=sum(c['cost_usd'] for c in calls))
    arms={a:stats([x for x in data['rows'] if x['arm']==a]) for a in ('baseline','guided')}
    paired={}
    for row in data['rows']:paired.setdefault((row['model'],row['seed'],row['question']),{})[row['arm']]=row['correct']
    return dict(token_spend_usd=tokens['spend_usd'],accuracy_spend_usd=data['spend_usd'],total_spend_usd=tokens['spend_usd']+data['spend_usd'],arms=arms,
        per_model={m:{a:stats([x for x in data['rows'] if x['model']==m and x['arm']==a]) for a in arms} for m in u.r.MODELS},
        per_family={f:{a:stats([x for x in data['rows'] if x['family']==f and x['arm']==a]) for a in arms} for f in ('cargo','teams','paths')},
        paired_changes=dict(improved=sum(not x['baseline'] and x['guided'] for x in paired.values()),regressed=sum(x['baseline'] and not x['guided'] for x in paired.values())),
        token_inputs={m:{a:sum(c['usage']['prompt_tokens'] for row in tokens['rows'] if row['model']==m and row['arm']==a for c in row['calls'])/6 for a in arms} for m in u.r.MODELS})

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--check',action='store_true');args=ap.parse_args();result=summarize();path=ROOT/'eval/query/query-guidance-summary.json'
    if args.check:assert json.loads(path.read_text())==result;print('Paired guidance summary, frozen evaluator, scores and spending verified')
    else:path.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
