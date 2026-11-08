#!/usr/bin/env python3
"""Targeted two-small-model eval of opt-in host meanings on three fresh item states.
Token-only checks first. Every invocation requires --cap; --dry-run/--check are offline.
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse
import json
import math
from pathlib import Path
import random
import subprocess
import sys
from types import SimpleNamespace
sys.path.insert(0,str(Path(__file__).resolve().parent))
import integration_eval as e
w=e.q
q=w.q
m=q.m


def bundle(seed):
    rng=random.Random(seed)
    rows=[]
    for i in range(24):
        count=rng.choice([1,1,2,3,8,12])
        rarity=rng.choice(['unique','common','rare'])
        if i<4:rarity,count=[('unique',8),('common',1),('unique',1),('rare',2)][i]
        mass=rng.randrange(1,201)/20
        rows.append(dict(id=f'item_{seed}_{i}',rarity=rarity,count=count,mass=mass,pack_mass=round(mass*count,6)))
    columns={
        'rarity':dict(description='Rarity label, independent of quantity.',unit=None,kind='categorical',values={'unique':'The literal rarity category unique; it does not mean count = 1.'}),
        'count':dict(description='Number of units in the stack. One row is one stack.',unit='items',kind='quantity',values={}),
        'mass':dict(description='Mass of one individual unit; not the entire stack.',unit='kg/item',kind='continuous',values={}),
        'pack_mass':dict(description='Mass of the whole stack: count * mass. Sum for total carried mass.',unit='kg/stack',kind='continuous',values={})}
    tables={'items':rows};meaning={'items':dict(description='Carried item stacks, one row per stack.',columns=columns)}
    unique=[r for r in rows if r['rarity']=='unique']
    questions=[
        ('Which unique items do I carry?',[r['id'] for r in unique]),
        ('How many unique item rows do I carry?',len(unique)),
        ('Which items have quantity one?',[r['id'] for r in rows if r['count']==1]),
        ('What is the total mass I carry?',sum(r['pack_mass'] for r in rows)),
        ('What is the combined mass of rare stacks?',sum(r['pack_mass'] for r in rows if r['rarity']=='rare')),
        ('What is the mass of one item for the heaviest individual item?',max(r['mass'] for r in rows)),
        ('Which unique items have count greater than two?',[r['id'] for r in unique if r['count']>2]),
        ('What is the total number of units in unique stacks?',sum(r['count'] for r in unique)),
        ('How many common stacks have only one unit?',sum(r['rarity']=='common' and r['count']==1 for r in rows)),
        ('What is the total mass of one unit from each unique stack?',sum(r['mass'] for r in unique))]
    return tables,meaning,questions


def correct(expected,actual):
    if isinstance(expected,list):return isinstance(actual,list) and sorted(expected)==sorted(actual)
    return isinstance(actual,(int,float)) and not isinstance(actual,bool) and abs(actual-expected)<=1e-6*max(1,abs(expected))


def check():
    binary=str(e.b.ROOT/'target/release/gamectx')
    for seed in (6101,6102,6103):
        tables,meaning,_=bundle(seed)
        p=subprocess.run([str(e.b.ROOT/'target/integration-check')],input=json.dumps(dict(kind='metadata',tables=tables,metadata=meaning)),capture_output=True,text=True,check=True)
        assert p.stdout.strip()==q.dumps(e.metadata(tables,meaning))
    saved=e.b.ROOT/'eval/query/query-meaning-results.json'
    checked=0
    if saved.exists():
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            for row in json.loads(saved.read_text())['rows']:
                tables,_,_=bundle(row['seed'])
                path=Path(directory)/'tables.json';path.write_text(json.dumps(tables))
                try:expected=q.dumps(q.run_sql(row['sql'],tables))
                except q.QueryError:expected=None
                actual=subprocess.run([binary,'query',str(path),row['sql']],capture_output=True,text=True)
                got=actual.stdout.strip() if actual.returncode==0 else None
                assert got==expected,(row['sql'],expected,got)
                checked+=1
    print(f'3 held-out metadata packets and {checked} recorded queries byte-identical')


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--cap',required=True,type=float)
    p.add_argument('--tokens-only',action='store_true')
    p.add_argument('--dry-run',action='store_true')
    p.add_argument('--check',action='store_true')
    p.add_argument('--output',type=Path)
    p.add_argument('--base-url',default='https://api.venice.ai/api/v1')
    args=p.parse_args()
    if not math.isfinite(args.cap) or args.cap<0:p.error('finite nonnegative --cap required')
    if args.check:check();return
    if args.dry_run:
        for seed in (6101,6102,6103):
            tables,meaning,questions=bundle(seed)
            print(seed,len(questions),len(q.dumps(w.schema(tables)).encode()),len(q.dumps(e.metadata(tables,meaning)).encode()))
        return
    key=(Path.home()/'.config/gamectx-eval/venice.key').read_text().strip()
    if not key:raise ValueError('empty credential file')
    prices=m.get_prices(args.base_url,key)
    account=dict(model='',spent=0.0)
    rows=[]
    output=args.output or e.b.ROOT/('eval/query/query-meaning-tokens.json' if args.tokens_only else 'eval/query/query-meaning-results.json')
    completed=True
    try:
        for model in m.DEFAULT_MODELS.split(','):
            account['model']=model
            for seed in ((6101,) if args.tokens_only else (6101,6102,6103)):
                tables,meaning,questions=bundle(seed)
                for mode in ('schema','meaning'):
                    description=w.schema(tables) if mode=='schema' else e.metadata(tables,meaning)
                    system=m.SYSTEM+'\nQuery tables: '+q.dumps(description)
                    for question,expected in (questions[:1] if args.tokens_only else questions):
                        messages=[dict(role='system',content=system),dict(role='user',content=question+q.SQL_RULE)]
                        text,call=w.ask(messages,args,key,prices,account,1 if args.tokens_only else 160)
                        sql='';result=None;error=None;ok=False
                        if not args.tokens_only:
                            data=m.extract_json(text);sql=data.get('sql','') if isinstance(data,dict) else ''
                            try:result=q.run_sql(sql,tables);ok=correct(expected,result)
                            except q.QueryError as exc:error=str(exc)
                        rows.append(dict(model=model,seed=seed,mode=mode,question=question,expected=expected,sql=sql,result=result,error=error,correct=ok,usage=call['usage'],cost_usd=call['cost_usd']))
                        output.write_text(json.dumps(dict(tokens_only=args.tokens_only,spend_usd=account['spent'],completed=False,rows=rows),indent=2)+'\n')
                print(f'{model} state {seed}: spent ${account["spent"]:.6f}',flush=True)
    except StopIteration:completed=False
    finally:
        output.write_text(json.dumps(dict(tokens_only=args.tokens_only,spend_usd=account['spent'],completed=completed,rows=rows),indent=2)+'\n')
        print(f'spent ${account["spent"]:.9f}',flush=True)
    if not args.tokens_only:
        for model in m.DEFAULT_MODELS.split(','):
            for mode in ('schema','meaning'):
                selected=[r for r in rows if r['model']==model and r['mode']==mode]
                print(model,mode,sum(r['correct'] for r in selected),'/',len(selected))


if __name__=='__main__':main()
