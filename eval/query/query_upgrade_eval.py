#!/usr/bin/env python3
"""Query plan/shape prototype and fresh paired guidance test. Offline by default."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,hashlib,json,math,random,re,subprocess,sys,tempfile,datetime
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import release_model_eval as r
q=r.q
GUIDANCE=('Return all matching rows for a list: do not add LIMIT unless the question requests a top-N or one row. '
          'Use only filters requested by the question; field meanings do not add filters. '
          'For a top-N total use SELECT SUM(column) FROM table ORDER BY column DESC LIMIT N, substituting listed names. '
          'Never put SELECT inside SELECT or IN. Do not add eligibility filters to an all-row total.')
FRESH=(('cargo',12101,24),('cargo',12102,48),('teams',12201,16),('teams',12202,32),('paths',12301,20),('paths',12302,40))

class Raw(float):
    def __new__(cls,text):v=super().__new__(cls,text);v.raw=text;return v
    def __neg__(self):return Raw('-'+self.raw)
def dump(v):
    if isinstance(v,Raw):return v.raw
    if isinstance(v,dict):return '{'+','.join(json.dumps(k,ensure_ascii=False)+':'+dump(x) for k,x in v.items())+'}'
    if isinstance(v,(list,tuple)):return '['+','.join(dump(x) for x in v)+']'
    return q.dumps(v)
def raw_tokens(text):
    q.tokenize(text);out=[];position=0
    while text[position:].strip():
        match=q.TOKEN.match(text,position);n,ident,a,b,sym=match.groups()
        out.append(('num',Raw(n)) if n is not None else ('id',ident) if ident is not None else ('str',a if a is not None else b) if a is not None or b is not None else ('sym','!=' if sym=='<>' else sym));position=match.end()
    return out

def plan(sql):
    items,table,_,order,limit=q.parse_sql(sql);p=q.Parser(raw_tokens(sql));p.expect_keyword('select')
    while True:
        p.expr()
        if p.keyword('as'):p.ident()
        if not p.sym(','):break
    p.expect_keyword('from');p.ident();filters=[]
    if p.keyword('where'):
        while True:
            col=p.ident()
            if p.keyword('in'):
                p.sym('(');values=[p.literal()[1]]
                while p.sym(','):values.append(p.literal()[1])
                p.sym(')');op='in';value=values
            else:op=p.peek()[1];p.i+=1;value=p.literal()[1]
            filters.append(dict(column=col,op=op,value=value))
            if not p.keyword('and'):break
    columns=[]
    for e in items:q.columns_of(e,columns)
    columns.extend(f['column'] for f in filters)
    if order:columns.append(order[0])
    return dict(table=table,aggregate=any(q.has_agg(e) for e in items),columns=list(dict.fromkeys(columns)),filters=filters,order_by=dict(column=order[0],descending=order[1]) if order else None,limit=limit)

def shape(v):
    if not isinstance(v,dict) or set(v)!={'table','filter_columns','limit','aggregate'}:raise ValueError('shape fields')
    names=[v['table']]+v['filter_columns']
    if len(v['filter_columns'])>16 or any(not isinstance(n,str) or not re.fullmatch('[A-Za-z_][A-Za-z0-9_]{0,63}',n) for n in names):raise ValueError('shape names')
    if type(v['aggregate']) is not bool or v['limit'] is not None and (type(v['limit']) is not int or not 0<=v['limit']<2**32):raise ValueError('shape flags')
    return dict(table=v['table'],filter_columns=sorted(v['filter_columns']),limit=v['limit'],aggregate=v['aggregate'])
def require(p,s):
    s=shape(s)
    if p['table']!=s['table'] or p['aggregate']!=s['aggregate'] or p['limit']!=s['limit'] or sorted(f['column'] for f in p['filters'])!=s['filter_columns']:raise ValueError('query shape differs from host requirements')

def process(v):
    p=plan(v['sql'])
    if 'shape' in v:require(p,v['shape'])
    if v.get('op','plan')=='plan':return p
    rows=v['rows']
    if len(rows)>v['max_rows']:raise ValueError('query input row limit')
    result=q.run_sql(v['sql'],{p['table']:rows})
    if len(dump(result).encode())>v['result_bytes']:raise ValueError('query result byte limit')
    return dict(plan=p,result=result)

def fixtures():
    base=dict(sql="SELECT id FROM cargo WHERE units >= 5 AND mass < 3")
    yield 'list-plan',base,False
    yield 'top-n-plan',dict(sql='SELECT SUM(mass), COUNT(*) FROM cargo ORDER BY mass DESC LIMIT 3'),False
    yield 'literal-spellings',dict(sql='SELECT id AS label FROM cargo WHERE mass >= -1.234567890123 AND kind IN (food,\'tool\')'),False
    yield 'empty-bounded',dict(op='run',sql='SELECT COUNT(*) FROM cargo',rows=[],max_rows=0,result_bytes=1),False
    contract=dict(table='cargo',filter_columns=['units','mass'],limit=None,aggregate=False)
    yield 'exact-host-shape',dict(base,shape=contract),False
    yield 'reordered-filters',dict(sql='SELECT id FROM cargo WHERE mass < 3 AND units >= 5',shape=contract),False
    yield 'repeated-filter-column',dict(sql='SELECT id FROM cargo WHERE mass > 1 AND mass < 3',shape=dict(table='cargo',filter_columns=['mass','mass'],limit=None,aggregate=False)),False
    yield 'bounded-result',dict(op='run',sql='SELECT SUM(mass) FROM cargo',rows=[dict(mass=3),dict(mass=2)],max_rows=2,result_bytes=1),False
    for name,extra in [('limit',' LIMIT 1'),('filter',' AND access = open')]:yield 'error-unrequested-'+name,dict(sql=base['sql']+extra,shape=contract),True
    yield 'error-missing-filter',dict(sql='SELECT id FROM cargo WHERE mass < 3',shape=contract),True
    yield 'error-aggregate-shape',dict(sql='SELECT COUNT(*) FROM cargo WHERE units >= 5 AND mass < 3',shape=contract),True
    yield 'error-input-budget',dict(op='run',sql='SELECT COUNT(*) FROM cargo',rows=[dict(n=1)],max_rows=0,result_bytes=32),True
    yield 'error-output-budget',dict(op='run',sql='SELECT id FROM cargo',rows=[dict(id='long_identifier')],max_rows=1,result_bytes=2),True
    yield 'error-subquery',dict(sql='SELECT SUM(mass) FROM (SELECT mass FROM cargo ORDER BY mass DESC LIMIT 3)'),True
    yield 'error-shape-fields',dict(base,shape=dict(contract,extra=True)),True

def offline_measure():
    saved=json.loads((r.ROOT/'eval/query/release-model-results.json').read_text());states={d['seed']:d for d in saved['states']};rejected=0;accepted=0;correct_rejected=0
    for row in saved['rows']:
        if row['mode']!='query' or not row['attempts']:continue
        oracle=next(x for x in states[row['seed']]['questions'] if x['question']==row['question']);p=plan(oracle['oracle']);s=dict(table=p['table'],filter_columns=[f['column'] for f in p['filters']],limit=p['limit'],aggregate=p['aggregate'])
        try:candidate=plan(row['attempts'][-1]['sql']);require(candidate,s)
        except (ValueError,q.QueryError):rejected+=1;correct_rejected+=bool(row['correct'])
        else:accepted+=1
    values=[10,9,9,9];top=sorted(values,reverse=True)[:3];membership=sum(v for v in values if v in top)
    return dict(api_spend_usd=0,recorded_query_shapes=dict(accepted=accepted,rejected=rejected,correct_rejected=correct_rejected,total=accepted+rejected),unsafe_in_rewrite=dict(values=values,top_three_sum=sum(top),in_membership_sum=membership),limitations='Host-supplied oracle shape diagnostic; not natural-language intent inference or a new model accuracy measurement.')

def bundles(binary,folder):
    out=[]
    for family,seed,size in FRESH:
        d=r.state(family,seed,size);sub=folder/str(seed);sub.mkdir();path=sub/'tables.json';path.write_text(r.dump(d['tables']));meta=sub/'metadata.json';meta.write_text(r.dump(d['metadata']))
        description=r.native(binary,['query','--schema',str(path),'--metadata',str(meta)])
        for question in d['questions']:assert r.correct(question['expected'],json.loads(r.native(binary,['query',str(path),question['oracle']])))
        d.update(prompt=r.SYSTEM+'\nQuery tables: '+description,path=path);out.append(d)
    return out

def models(args,states):
    key=(Path.home()/'.config/gamectx-eval/venice.key').read_text().strip();prices=r.m.get_prices(args.base_url,key)
    for model in r.MODELS:
        if model not in prices or any(not math.isfinite(prices[model][k]) or prices[model][k]<0 for k in ('input','output')):raise ValueError('missing valid pricing')
    prior=json.loads(args.resume.read_text()) if args.resume else None
    ledger=dict(spent=prior['spend_usd'] if prior else 0.,calls=list(prior['calls']) if prior else []);rows=list(prior['rows']) if prior else []
    data=dict(date=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),eval_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),guidance=GUIDANCE,tokens_only=args.tokens_only,cap_usd=args.cap,completed=False,stopped=None,states=[{k:v for k,v in d.items() if k!='path'} for d in states],rows=rows,prices={m:prices[m] for m in r.MODELS})
    if prior:
        for key_ in ('source_commit','guidance','tokens_only','states','prices'):
            if data[key_]!=prior[key_]:raise ValueError('resume inputs/pricing changed')
        data.update(resumed_from=args.resume.name,initial_eval_sha256=prior.get('initial_eval_sha256',prior['eval_sha256']))
    completed={(x['model'],x['seed'],x['arm'],x['question']) for x in rows}
    def checkpoint():data.update(spend_usd=ledger['spent'],calls=ledger['calls']);args.output.write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    try:
        for model in r.MODELS:
            for d in states:
                for n,question in enumerate(d['questions'][:1] if args.tokens_only else d['questions']):
                    for arm in (('baseline','guided') if n%2==0 else ('guided','baseline')):
                        if (model,d['seed'],arm,question['question']) in completed:continue
                        calls=[];attempts=[];answer=None;first=executed=False
                        messages=[dict(role='system',content=d['prompt']+('\n'+GUIDANCE if arm=='guided' else '')),dict(role='user',content=question['question']+r.SQL_RULE)]
                        text,call=r.ask(messages,model,1 if args.tokens_only else 320,args,key,prices,ledger,checkpoint);calls.append(call)
                        if not args.tokens_only:
                            while True:
                                parsed=r.extract(text);sql=parsed.get('sql','') if isinstance(parsed,dict) and set(parsed)=={'sql'} and isinstance(parsed['sql'],str) else ''
                                fb=r.feedback(args.gamectx,d['path'],[a['sql'] for a in attempts]+[sql])[-1];attempts.append(dict(sql=sql,reply=text,feedback=fb));messages.append(dict(role='assistant',content=text))
                                if 'result' in fb:
                                    executed=r.correct(question['expected'],fb['result']);first=executed and len(attempts)==1
                                    messages.append(dict(role='user',content=r.dump(fb)+r.ANSWER_RULE));text,call=r.ask(messages,model,320,args,key,prices,ledger,checkpoint);calls.append(call);parsed=r.extract(text);answer=parsed.get('answer') if isinstance(parsed,dict) and set(parsed)=={'answer'} else None;break
                                if not fb['retry']:break
                                messages.append(dict(role='user',content=r.dump(fb)+r.SQL_RULE));text,call=r.ask(messages,model,320,args,key,prices,ledger,checkpoint);calls.append(call)
                        rows.append(dict(model=model,seed=d['seed'],family=d['family'],arm=arm,question=question['question'],expected=question['expected'],first_correct=first,executed_correct=executed,correct=False if args.tokens_only else r.correct(question['expected'],answer),answer=answer,attempts=attempts,calls=calls));checkpoint()
                    if not args.tokens_only and (n+1)%4==0:print(model,d['seed'],f'{n+1}/8',f'${ledger["spent"]:.8f}',flush=True)
                print(model,d['seed'],'complete',f'${ledger["spent"]:.8f}',flush=True)
        data['completed']=True
    except StopIteration as error:data['stopped']=str(error)
    except Exception as error:data['stopped']=type(error).__name__;raise
    finally:checkpoint();print(f'Spend ${ledger["spent"]:.12f}; completed={data["completed"]}',flush=True)
    for model in r.MODELS:
        for arm in ('baseline','guided'):
            rs=[x for x in rows if x['model']==model and x['arm']==arm];calls=[c for x in rs for c in x['calls']]
            print(model,arm,sum(x['correct'] for x in rs),'/',len(rs),'input',sum(c['usage'].get('prompt_tokens',0) for c in calls)/len(rs),flush=True)

def check(path,binary,states):
    saved=json.loads(path.read_text());by={d['seed']:d for d in states};count=0
    for row in saved['rows']:
        if row['attempts']:
            fb=r.feedback(binary,by[row['seed']]['path'],[a['sql'] for a in row['attempts']]);assert r.dump(fb)==r.dump([a['feedback'] for a in row['attempts']]);session=r.w.Session();assert r.dump([session.submit(a['sql'],by[row['seed']]['tables']) for a in row['attempts']])==r.dump(fb);count+=len(fb)
    print(count,'fresh recorded feedback envelopes match Rust/Python')

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--cap',type=float,required=True);ap.add_argument('--tokens-only',action='store_true');ap.add_argument('--resume',type=Path);ap.add_argument('--models',action='store_true',help='run the frozen paired model experiment');ap.add_argument('--check',type=Path);ap.add_argument('--fixtures',action='store_true');ap.add_argument('--output',type=Path);ap.add_argument('--gamectx',default=str(r.ROOT/'target/release/gamectx'));ap.add_argument('--base-url',default='https://api.venice.ai/api/v1');args=ap.parse_args()
    if not math.isfinite(args.cap) or args.cap<0:ap.error('finite nonnegative --cap required')
    if args.base_url!='https://api.venice.ai/api/v1':ap.error('only Venice endpoint allowed')
    if args.models or args.check:
        with tempfile.TemporaryDirectory() as tmp:
            states=bundles(args.gamectx,Path(tmp))
            if args.check:check(args.check,args.gamectx,states)
            else:
                if args.output is None or args.output.exists():ap.error('new output path required')
                models(args,states)
    elif args.fixtures:
        for label,v,error in fixtures():
            folder=r.ROOT/'conformance/query-plan'/label;folder.mkdir(parents=True,exist_ok=True);(folder/'case.json').write_text(dump(dict(op='query-plan',**({'error':True} if error else {})))+'\n');(folder/'input.json').write_text(dump(v)+'\n')
            try:out=process(v)
            except (ValueError,q.QueryError):
                if not error:raise
            else:
                if error:raise AssertionError(label)
                (folder/'expected.json').write_text(dump(out)+'\n')
    else:
        result=offline_measure();(r.ROOT/'eval/query/query-upgrade-offline.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
