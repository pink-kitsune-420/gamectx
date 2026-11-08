#!/usr/bin/env python3
"""Frozen multi-family alpha.3 model evaluation. Explicit caps; two small models only.
No paid calls with --check/--dry-run. --tokens-only must precede accuracy runs.
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,collections,datetime,hashlib,json,math,random,subprocess,sys,tempfile,urllib.request
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import query_meaning_eval as e
w=e.w;q=e.q;m=e.m
ROOT=e.e.b.ROOT
MODELS=('llama-3.2-3b','mistral-small-3-2-24b-instruct')
MODES=('json','packed','query')
STATES=(('cargo',11101,24),('cargo',11102,48),('teams',11201,16),('teams',11202,32),('paths',11301,20),('paths',11302,40))
SYSTEM='Answer structured questions about a game state using only the supplied data and field meanings. Return exactly one JSON object, no prose. Numeric answers must be numbers; ID answers must be JSON lists. Packed table rows align with their columns.'
SQL_RULE=' Return exactly {"sql":"SELECT ..."}. Use one listed table, AND/IN comparisons, ORDER BY, LIMIT and COUNT/SUM/AVG/MIN/MAX. No joins, OR, subqueries or GROUP BY. ORDER BY/LIMIT run before aggregates.'
ANSWER_RULE=' Return exactly {"answer":<result>}, preserving numbers and ID lists.'

def dump(v):return q.dumps(v)
def column(description,kind='continuous',unit=None,values=None):return dict(description=description,unit=unit,kind=kind,values=values or {})
def state(family,seed,size):
    rng=random.Random(seed);rows=[];tests=[]
    if family=='cargo':
        for i in range(size):
            category=rng.choice(['food','tool','ore']);quality=['common','rare','unique'][i%3];units=rng.choice([1,2,3,5,8,12]);mass=rng.randrange(1,201)/20
            rows.append(dict(id=f'cargo_{seed}_{i}',category=category,quality=quality,units=units,unit_mass=mass,stack_mass=q.num_json(mass*units)))
        fields={'category':column('Native item category.','categorical'), 'quality':column('Rarity category, independent of quantity.','categorical',values={'unique':'The literal unique category; it does not mean units = 1.'}), 'units':column('Units in this row; one row is one stack.','quantity','items'), 'unit_mass':column('Mass of one unit, independent of stack quantity.',unit='kg/item'), 'stack_mass':column('Mass of this complete stack, units * unit_mass.',unit='kg/stack')}
        tests=[('filter','List IDs of rare tools.','SELECT id FROM cargo WHERE quality = \'rare\' AND category = \'tool\'',[r['id'] for r in rows if r['quality']=='rare' and r['category']=='tool']),
               ('numeric_filter','List IDs with at least five units and unit mass below 3 kg.','SELECT id FROM cargo WHERE units >= 5 AND unit_mass < 3',[r['id'] for r in rows if r['units']>=5 and r['unit_mass']<3]),
               ('count','How many rows have quality unique?','SELECT COUNT(*) FROM cargo WHERE quality = \'unique\'',sum(r['quality']=='unique' for r in rows)),
               ('total','What is the total carried mass of all stacks?','SELECT SUM(stack_mass) FROM cargo',sum(r['stack_mass'] for r in rows)),
               ('total','How many food units are carried in total?','SELECT SUM(units) FROM cargo WHERE category = \'food\'',sum(r['units'] for r in rows if r['category']=='food')),
               ('top_n','What is the sum of unit_mass for the three rows with largest unit_mass?','SELECT SUM(unit_mass) FROM cargo ORDER BY unit_mass DESC LIMIT 3',sum(sorted((r['unit_mass'] for r in rows),reverse=True)[:3])),
               ('total','What is the combined mass of complete rare stacks?','SELECT SUM(stack_mass) FROM cargo WHERE quality = \'rare\'',sum(r['stack_mass'] for r in rows if r['quality']=='rare')),
               ('count','How many food or ore stacks have more than two units?','SELECT COUNT(*) FROM cargo WHERE category IN (\'food\',\'ore\') AND units > 2',sum(r['category'] in ('food','ore') and r['units']>2 for r in rows))]
    elif family=='teams':
        for i in range(size):
            members=rng.randrange(1,9);supply=rng.randrange(1,41)/2
            rows.append(dict(id=f'team_{seed}_{i}',role=['medic','scout','guard'][i%3],standing=['ally','neutral','hostile'][(i//3)%3],members=members,unit_supply=supply,group_supply=q.num_json(members*supply),readiness=rng.randrange(21)/20))
        fields={'role':column('Role of the team.','categorical'),'standing':column('Current diplomatic relation.','categorical'), 'members':column('Number of members in the team.','quantity','people'), 'unit_supply':column('Supply carried by one member, not the whole team.',unit='supply/person'), 'group_supply':column('Whole-team supply, members * unit_supply.',unit='supply/team'), 'readiness':column('Native readiness score from zero to one.',unit='fraction')}
        medics=[r['readiness'] for r in rows if r['role']=='medic']
        tests=[('filter','List IDs of allied guards with readiness above 0.5.','SELECT id FROM teams WHERE standing = \'ally\' AND role = \'guard\' AND readiness > 0.5',[r['id'] for r in rows if r['standing']=='ally' and r['role']=='guard' and r['readiness']>.5]),
               ('numeric_filter','List hostile team IDs with at least four members.','SELECT id FROM teams WHERE standing = \'hostile\' AND members >= 4',[r['id'] for r in rows if r['standing']=='hostile' and r['members']>=4]),
               ('total','How many allied members are there in total?','SELECT SUM(members) FROM teams WHERE standing = \'ally\'',sum(r['members'] for r in rows if r['standing']=='ally')),
               ('total','What is the whole-team supply total for teams with readiness at least 0.6?','SELECT SUM(group_supply) FROM teams WHERE readiness >= 0.6',sum(r['group_supply'] for r in rows if r['readiness']>=.6)),
               ('average','What is the average readiness of medic teams?','SELECT AVG(readiness) FROM teams WHERE role = \'medic\'',sum(medics)/len(medics)),
               ('extreme','What is the smallest per-member supply value?','SELECT MIN(unit_supply) FROM teams',min(r['unit_supply'] for r in rows)),
               ('top_n','What is the sum of group_supply for the three teams with largest group_supply?','SELECT SUM(group_supply) FROM teams ORDER BY group_supply DESC LIMIT 3',sum(sorted((r['group_supply'] for r in rows),reverse=True)[:3])),
               ('count','How many scout teams are neutral?','SELECT COUNT(*) FROM teams WHERE role = \'scout\' AND standing = \'neutral\'',sum(r['role']=='scout' and r['standing']=='neutral' for r in rows))]
    elif family=='paths':
        for i in range(size):
            toll=rng.randrange(11)/2;party=rng.randrange(1,9)
            rows.append(dict(id=f'path_{seed}_{i}',zone=['ridge','marsh','ruins'][i%3],access=['open','locked','sealed'][(i//3)%3],minutes=rng.randrange(1,121),toll_per_member=toll,party_size=party,party_toll=q.num_json(toll*party),risk=rng.randrange(21)/20))
        fields={'zone':column('Native region label.','categorical'),'access':column('Current access state; only open paths are traversable.','categorical'),'minutes':column('Travel duration for this route.',unit='minutes'),'toll_per_member':column('Toll charged to one member.',unit='coins/person'),'party_size':column('Members in the party for this route.','quantity','people'),'party_toll':column('Total toll for the entire party, party_size * toll_per_member.',unit='coins/party'),'risk':column('Native route risk score from zero to one.',unit='fraction')}
        ridge=[r['minutes'] for r in rows if r['zone']=='ridge']
        tests=[('numeric_filter','List open path IDs with risk at most 0.3.','SELECT id FROM paths WHERE access = \'open\' AND risk <= 0.3',[r['id'] for r in rows if r['access']=='open' and r['risk']<=.3]),
               ('filter','List open path IDs with zero toll per member.','SELECT id FROM paths WHERE access = \'open\' AND toll_per_member = 0',[r['id'] for r in rows if r['access']=='open' and r['toll_per_member']==0]),
               ('total','What is the total whole-party toll across open routes?','SELECT SUM(party_toll) FROM paths WHERE access = \'open\'',sum(r['party_toll'] for r in rows if r['access']=='open')),
               ('extreme','What is the largest travel duration among open routes?','SELECT MAX(minutes) FROM paths WHERE access = \'open\'',max(r['minutes'] for r in rows if r['access']=='open')),
               ('average','What is the average travel duration for ridge routes?','SELECT AVG(minutes) FROM paths WHERE zone = \'ridge\'',sum(ridge)/len(ridge)),
               ('top_n','What is the sum of party_toll for the three routes with largest party_toll?','SELECT SUM(party_toll) FROM paths ORDER BY party_toll DESC LIMIT 3',sum(sorted((r['party_toll'] for r in rows),reverse=True)[:3])),
               ('count','How many locked paths have risk at least 0.5?','SELECT COUNT(*) FROM paths WHERE access = \'locked\' AND risk >= 0.5',sum(r['access']=='locked' and r['risk']>=.5 for r in rows)),
               ('total','What is the sum of party_size across open ridge or ruins routes?','SELECT SUM(party_size) FROM paths WHERE access = \'open\' AND zone IN (\'ridge\',\'ruins\')',sum(r['party_size'] for r in rows if r['access']=='open' and r['zone'] in ('ridge','ruins')))]
    else:raise ValueError('unknown family')
    return dict(family=family,seed=seed,size=size,tables={family:rows},metadata={family:dict(description='Native game rows; all columns describe the same row.',columns=fields)},questions=[dict(kind=k,question=t,oracle=sql,expected=want) for k,t,sql,want in tests])

def correct(want,got):
    if isinstance(want,list):return isinstance(got,list) and all(isinstance(x,str) for x in got) and sorted(got)==sorted(want)
    return isinstance(got,(int,float)) and not isinstance(got,bool) and math.isfinite(got) and abs(got-want)<=1e-6*max(1,abs(want))

def extract(text):
    text=text.strip()
    if text.startswith('```') and text.endswith('```'):text=text[text.find('\n')+1:-3].strip()
    def pairs(items):
        out={}
        for k,v in items:
            if k in out:raise ValueError('duplicate model JSON key')
            out[k]=v
        return out
    try:return json.loads(text,object_pairs_hook=pairs,parse_constant=lambda v:(_ for _ in ()).throw(ValueError(v)))
    except (ValueError,TypeError):return None

def native(binary,args,stdin=None):
    p=subprocess.run([binary,*args],input=stdin,text=True,capture_output=True)
    if p.returncode:raise ValueError('Rust CLI rejected evaluation input: '+p.stderr[:250])
    return p.stdout.strip()

def prepare(binary,directory):
    bundles=[]
    for family,seed,size in STATES:
        data=state(family,seed,size);sub=directory/str(seed);sub.mkdir(parents=True,exist_ok=True)
        path=sub/'tables.json';path.write_text(dump(data['tables']));meta=sub/'metadata.json';meta.write_text(dump(data['metadata']))
        described=json.loads(native(binary,['query','--schema',str(path),'--metadata',str(meta)]))
        assert dump(described)==dump(e.e.metadata(data['tables'],data['metadata']))
        manifest=dict(sections=[dict(name=family,pack='table',value=data['tables'][family])])
        packed=json.loads(native(binary,['pack','-'],dump(manifest)))
        for question in data['questions']:
            result=json.loads(native(binary,['query',str(path),question['oracle']]))
            assert correct(question['expected'],result),(seed,question,result)
            assert correct(question['expected'],q.run_sql(question['oracle'],data['tables']))
        prompts={mode:SYSTEM+'\nGame state: '+dump(dict(meanings=data['metadata'],tables=data['tables'] if mode=='json' else packed)) for mode in ('json','packed')}
        prompts['query']=SYSTEM+'\nQuery tables: '+dump(described)
        data.update(prompts=prompts,tables_path=path)
        bundles.append(data)
    return bundles

# One network attempt per call. Failed/unknown calls retain their complete reservation.
# No hidden retries; cap accounting is checkpointed before dispatch and after every reply.
def ask(messages,model,max_tokens,args,key,prices,ledger,checkpoint):
    price=prices[model]
    reserve=((sum(len(x['content'].encode()) for x in messages)+512)*price['input']+max_tokens*price['output'])/1e6
    if ledger['spent']+reserve>args.cap:raise StopIteration('spending cap')
    ledger['spent']+=reserve;checkpoint()
    body=dict(model=model,messages=messages,temperature=0,max_completion_tokens=max_tokens,venice_parameters=dict(include_venice_system_prompt=False,enable_web_search='off'))
    request=urllib.request.Request(args.base_url+'/chat/completions',data=json.dumps(body).encode(),headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'})
    with urllib.request.urlopen(request,timeout=60) as response:reply=json.load(response)
    usage=reply.get('usage',{});known=all(type(usage.get(k)) is int and usage[k]>=0 for k in ('prompt_tokens','completion_tokens'))
    cost=(usage['prompt_tokens']*price['input']+usage['completion_tokens']*price['output'])/1e6 if known else reserve
    ledger['spent']+=cost-reserve
    content=reply['choices'][0]['message'].get('content') or ''
    call=dict(usage=usage,cost_usd=cost,reserved_usd=reserve,usage_known=known,reply=content)
    ledger['calls'].append(call);checkpoint()
    if cost>reserve or ledger['spent']>args.cap:raise StopIteration('reported usage exceeded reservation; stop new calls')
    return content,call

def feedback(binary,path,sqls):
    calls=''.join(dump(dict(sql=sql))+'\n' for sql in sqls)
    proc=subprocess.run([binary,'query','--session',str(path)],input=calls,text=True,capture_output=True)
    replies=[json.loads(line) for line in proc.stdout.splitlines()]
    # Closing stdin after the first retriable reply is an expected nonterminal EOF,
    # not a tool failure. Reconstruct both attempts on the next invocation.
    pending=(len(sqls)==len(replies)==1 and replies[-1].get('retry') is True and proc.returncode==1 and proc.stderr.strip()=='gamectx: query session ended before a terminal reply')
    if len(replies)!=len(sqls) or proc.returncode and not pending:raise ValueError('unexpected Rust session failure')
    return replies

def summary(data):
    out=[]
    for model in MODELS:
        for mode in MODES:
            rows=[r for r in data['rows'] if r['model']==model and r['mode']==mode]
            if not rows:continue
            calls=[c for r in rows for c in r['calls']]
            complete=all(c['usage_known'] for c in calls)
            out.append(dict(model=model,mode=mode,questions=len(rows),correct=sum(r['correct'] for r in rows),first_query_correct=sum(r.get('first_correct',False) for r in rows),executed_correct=sum(r.get('executed_correct',False) for r in rows),retries=sum(max(0,len(r['attempts'])-1) for r in rows),answer_copy_errors=sum(r.get('executed_correct',False) and not r['correct'] for r in rows),calls=len(calls),avg_input=sum(c['usage'].get('prompt_tokens',0) for c in calls)/len(rows) if complete else None,avg_output=sum(c['usage'].get('completion_tokens',0) for c in calls)/len(rows) if complete else None,spend_usd=sum(c['cost_usd'] for c in calls)))
    return out

def evaluate(args,bundles):
    key=(Path.home()/'.config/gamectx-eval/venice.key').read_text().strip()
    if not key:raise ValueError('empty Venice credential file')
    prices=m.get_prices(args.base_url,key)
    for model in MODELS:
        if model not in prices or any(not math.isfinite(prices[model][k]) or prices[model][k]<0 for k in ('input','output')):raise ValueError('missing or invalid model pricing')
    prior=json.loads(args.resume.read_text()) if args.resume else None
    ledger=dict(spent=prior['spend_usd'] if prior else 0.0,calls=list(prior['call_ledger']) if prior else [])
    rows=list(prior['rows']) if prior else []
    data=dict(date=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),eval_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),release='0.1.0-alpha.3',tokens_only=args.tokens_only,cap_usd=args.cap,models=MODELS,modes=MODES,prices={k:prices[k] for k in MODELS},completed=False,stopped=None,spend_usd=0,rows=rows,states=[{k:v for k,v in d.items() if k!='tables_path'} for d in bundles])
    if prior:
        for key_ in ('source_commit','release','tokens_only','prices','states'):
            if data[key_]!=prior[key_]:raise ValueError('resume protocol/inputs/pricing changed')
        data.update(resumed_from=args.resume.name,initial_eval_sha256=prior.get('initial_eval_sha256',prior['eval_sha256']),prior_spend_usd=prior['spend_usd'])
    completed={(row['model'],row['seed'],row['mode'],row['question']) for row in rows}
    def checkpoint():
        data.update(spend_usd=ledger['spent'],call_ledger=ledger['calls']);args.output.write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    try:
        for model in MODELS:
            for d in bundles:
                for n,question in enumerate(d['questions'][:1] if args.tokens_only else d['questions']):
                    # Rotate format order to avoid always favoring one arm's request position.
                    modes=MODES[n%3:]+MODES[:n%3]
                    for mode in modes:
                        if (model,d['seed'],mode,question['question']) in completed:continue
                        calls=[];attempts=[];answer=None;first=executed=False
                        messages=[dict(role='system',content=d['prompts'][mode]),dict(role='user',content=question['question']+(SQL_RULE if mode=='query' else ANSWER_RULE))]
                        text,call=ask(messages,model,1 if args.tokens_only else 320,args,key,prices,ledger,checkpoint);calls.append(call)
                        if not args.tokens_only:
                            if mode!='query':
                                parsed=extract(text);answer=parsed['answer'] if isinstance(parsed,dict) and set(parsed)=={'answer'} else None
                            else:
                                while True:
                                    parsed=extract(text);sql=parsed['sql'] if isinstance(parsed,dict) and set(parsed)=={'sql'} and isinstance(parsed['sql'],str) else ''
                                    sqls=[a['sql'] for a in attempts]+[sql];reply=feedback(args.gamectx,d['tables_path'],sqls)[-1]
                                    attempts.append(dict(reply=text,sql=sql,feedback=reply));messages.append(dict(role='assistant',content=text))
                                    if 'result' in reply:
                                        executed=correct(question['expected'],reply['result']);first=executed and len(attempts)==1
                                        messages.append(dict(role='user',content=dump(reply)+ANSWER_RULE));text,call=ask(messages,model,320,args,key,prices,ledger,checkpoint);calls.append(call)
                                        parsed=extract(text);answer=parsed['answer'] if isinstance(parsed,dict) and set(parsed)=={'answer'} else None
                                        break
                                    if not reply['retry']:break
                                    messages.append(dict(role='user',content=dump(reply)+SQL_RULE));text,call=ask(messages,model,320,args,key,prices,ledger,checkpoint);calls.append(call)
                        rows.append(dict(model=model,seed=d['seed'],family=d['family'],size=d['size'],mode=mode,kind=question['kind'],question=question['question'],expected=question['expected'],answer=answer,reply=text,attempts=attempts,first_correct=first,executed_correct=executed,correct=False if args.tokens_only else correct(question['expected'],answer),calls=calls))
                        checkpoint()
                    if not args.tokens_only and (n+1)%4==0:print(f'{model} {d["family"]}/{d["seed"]} {n+1}/8; spend ${ledger["spent"]:.8f}',flush=True)
                print(f'{model} state {d["seed"]} complete; spend ${ledger["spent"]:.8f}',flush=True)
        data['completed']=True
    except StopIteration as error:data['stopped']=str(error)
    except Exception as error:
        # Never persist provider error bodies, request headers, credentials or private paths.
        data['stopped']='request/evaluation failure: '+type(error).__name__
        raise
    finally:
        data['summary']=summary(data);checkpoint();print(f'Spend ${ledger["spent"]:.10f}; completed={data["completed"]}; output={args.output.name}',flush=True)
    for row in data['summary']:print(json.dumps(row),flush=True)

def check_saved(args,bundles):
    data=json.loads(args.output.read_text());by_seed={d['seed']:d for d in bundles};checked=0
    for row in data['rows']:
        d=by_seed[row['seed']]
        if row['attempts']:
            actual=feedback(args.gamectx,d['tables_path'],[a['sql'] for a in row['attempts']])
            assert dump(actual)==dump([a['feedback'] for a in row['attempts']])
            session=w.Session()
            assert dump([session.submit(a['sql'],d['tables']) for a in row['attempts']])==dump(actual)
            checked+=len(actual)
        if not data['tokens_only']:assert row['correct']==correct(row['expected'],row['answer'])
    assert data['summary']==summary(data)
    print(f'{len(bundles)} states/48 oracles and {checked} recorded Rust/Python feedback envelopes verified')

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--cap',type=float,required=True);ap.add_argument('--tokens-only',action='store_true');ap.add_argument('--check',action='store_true');ap.add_argument('--dry-run',action='store_true');ap.add_argument('--resume',type=Path,help='reuse completed cases; --cap includes prior cumulative spend');ap.add_argument('--output',type=Path,required=True);ap.add_argument('--gamectx',default=str(ROOT/'target/release/gamectx'));ap.add_argument('--base-url',default='https://api.venice.ai/api/v1');args=ap.parse_args()
    if not math.isfinite(args.cap) or args.cap<0:ap.error('finite nonnegative --cap required')
    if args.base_url!='https://api.venice.ai/api/v1':ap.error('only the configured Venice endpoint is supported')
    if args.output.exists() and not args.check:ap.error('refusing to overwrite an existing run')
    with tempfile.TemporaryDirectory() as tmp:
        bundles=prepare(args.gamectx,Path(tmp))
        if args.check:check_saved(args,bundles)
        elif args.dry_run:
            for d in bundles:print(d['family'],d['seed'],d['size'],{k:len(v.encode()) for k,v in d['prompts'].items()})
        else:evaluate(args,bundles)
if __name__=='__main__':main()
