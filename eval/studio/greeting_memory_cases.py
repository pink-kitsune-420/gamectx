"""Independent bounded encounter/greeting oracle, host receipts only, zero model calls."""
import argparse,copy,json,pathlib,re,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];enc=lambda v:json.dumps(v,ensure_ascii=False,separators=(',',':'))
def ident(s):assert isinstance(s,str)and re.fullmatch('[A-Za-z0-9_:-]{1,64}',s)
def number(n,lo=0,hi=4294967295):assert type(n)is int and lo<=n<=hi
def config(c):assert set(c)=={'version','long_gap_turns'}and c['version']==1;number(c['long_gap_turns'],1,1000000)
def pair(v):ident(v['actor']);ident(v['listener']);assert v['actor']!=v['listener']
def receipt(v):
 assert set(v)=={'actor','listener','scene','visit','at','token','greeting','introduced','text','heard','expected'};pair(v);ident(v['scene'])
 for k in ['visit','at','token']:number(v[k],1 if k=='token'else 0)
 assert type(v['greeting'])is bool and type(v['introduced'])is bool and(not v['introduced']or v['greeting'])
 assert isinstance(v['text'],str)and 1<=len(v['text'])<=240 and len(v['text'].encode())<=1024 and not any(ord(c)<32 or 127<=ord(c)<=159 for c in v['text']);number(v['heard'],0,len(v['text']))
 if v['expected']is not None:number(v['expected'],1)
def recover(v):
 assert set(v)=={'version','revision','entries'}and v['version']==1;number(v['revision']);assert len(v['entries'])<=256;seen=[]
 for e in v['entries']:
  assert set(e)=={'actor','listener','first_at','last_at','token','scene','visit','exchanges','greetings','introduced','legacy','receipt'};pair(e);key=(e['actor'],e['listener']);assert key not in seen;seen.append(key)
  for k in ['first_at','last_at','token','exchanges','greetings']:number(e[k],1 if k in ['token','exchanges']else 0)
  assert e['first_at']<=e['last_at']and e['greetings']<=e['exchanges']and type(e['introduced'])is bool and type(e['legacy'])is bool;assert e['legacy']or not e['introduced']or e['greetings']>0
  if e['receipt']is None:assert e['legacy']and e['scene']is None and e['visit']is None and e['exchanges']==1 and e['greetings']==0
  else:
   r=e['receipt'];receipt(r);assert r['heard']==len(r['text'])
   for k in ['actor','listener','scene','visit','token']:assert e[k]==r[k]
   assert e['last_at']==r['at']and(not r['introduced']or e['introduced'])and(not r['greeting']or e['greetings']>0)
 return copy.deepcopy(v)
def oracle(a):
 assert set(a)=={'config','state','ops'};config(a['config']);state=recover(a['state']);out=[];assert len(a['ops'])<=256
 def find(v):return next((e for e in state['entries']if e['actor']==v['actor']and e['listener']==v['listener']),None)
 for op in a['ops']:
  assert set(op)=={'op','args'};v=op['args'];e=find(v)if op['op']!='configure'else None
  if op['op']=='configure':config(v);a['config']=copy.deepcopy(v);out.append(dict(configured=True));continue
  if op['op']=='plan':
   assert set(v)=={'actor','listener','scene','visit','at','trust','atmosphere','place'};pair(v);ident(v['scene']);number(v['visit']);number(v['at']);number(v['trust'],-100,100)
   assert v['atmosphere']in ['calm','busy','tense','emergency']and v['place']in ['workplace','social','private','other','hazard']
   if e:
    assert v['at']>=e['last_at']
    if e['visit']is not None:assert v['visit']>=e['visit']
   meeting='first'if e is None else'familiar'if e['scene']is None else'elsewhere'if e['scene']!=v['scene']else'returning'if e['visit']!=v['visit']else'after_gap'if v['at']-e['last_at']>=a['config']['long_gap_turns']else'continuing'
   out.append(dict(meeting=meeting,needs_introduction=e is None or not e['introduced'],expected=None if e is None else e['token'],relationship='guarded'if v['trust']<0 else'warm'if v['trust']>=25 else'reserved',atmosphere=v['atmosphere'],place=v['place'],scene=v['scene']));continue
  if op['op']=='seed':
   assert set(v)=={'actor','listener','at','token','introduced'};pair(v);number(v['at']);number(v['token'],1);assert type(v['introduced'])is bool and e is None and len(state['entries'])<256
   state['entries'].append(dict(actor=v['actor'],listener=v['listener'],first_at=v['at'],last_at=v['at'],token=v['token'],scene=None,visit=None,exchanges=1,greetings=0,introduced=v['introduced'],legacy=True,receipt=None))
  else:
   assert op['op']=='confirm';receipt(v)
   if v['heard']!=len(v['text']):out.append(dict(recorded=False,reason='partial',revision=state['revision']));continue
   if e and e['receipt']==v:out.append(dict(recorded=False,reason='duplicate',revision=state['revision']));continue
   assert v['expected']==(None if e is None else e['token'])
   if e:assert v['token']>e['token']and v['at']>=e['last_at']and(e['visit']is None or v['visit']>=e['visit'])
   else:
    assert len(state['entries'])<256;e=dict(actor=v['actor'],listener=v['listener'],first_at=v['at'],last_at=v['at'],token=v['token'],scene=None,visit=None,exchanges=0,greetings=0,introduced=False,legacy=False,receipt=None);state['entries'].append(e)
   e.update(last_at=v['at'],token=v['token'],scene=v['scene'],visit=v['visit'],exchanges=e['exchanges']+1,greetings=e['greetings']+int(v['greeting']),introduced=e['introduced']or v['introduced'],receipt=copy.deepcopy(v))
  number(state['revision']+1);state['revision']+=1;out.append(dict(recorded=True,revision=state['revision']))
 return dict(results=out,state=state,config=a['config'])
def cases():
 frame=dict(actor='merchant',listener='player_1',scene='workshop',visit=2,at=3,trust=15,atmosphere='calm',place='workplace')
 heard=dict(actor='merchant',listener='player_1',scene='workshop',visit=2,at=3,token=5,greeting=True,introduced=True,text='Hello, I am Ilen.',heard=17,expected=None);heard['heard']=len(heard['text'])
 plan=lambda **x:dict(op='plan',args=frame|x);confirm=lambda **x:dict(op='confirm',args=heard|x)
 def args(ops):return dict(config=dict(version=1,long_gap_turns=12),state=dict(version=1,revision=0,entries=[]),ops=ops)
 rows=[('first',[plan()],False),('confirmed',[confirm(),plan(at=4)],False),('partial',[confirm(heard=4),plan()],False),('unheard',[confirm(heard=0),plan()],False),('returning',[confirm(),plan(visit=6,at=8)],False),('elsewhere',[confirm(),plan(scene='tavern',visit=7,at=8,place='social')],False),('gap',[confirm(),plan(at=15)],False),('active_conversation',[confirm(),confirm(at=14,token=9,greeting=False,introduced=False,text='I can listen.',heard=13,expected=5),plan(at=15)],False),('unintroduced',[confirm(introduced=False),plan(at=4)],False),('other_player',[confirm(),plan(listener='player_2')],False),('other_actor',[confirm(),plan(actor='keeper',scene='cistern')],False),('guarded',[plan(trust=-20)],False),('warm',[plan(trust=40)],False),('emergency',[plan(atmosphere='emergency',place='hazard')],False),('private',[plan(place='private')],False),('idempotent',[confirm(),confirm()],False),('legacy',[dict(op='seed',args=dict(actor='merchant',listener='player_1',at=0,token=1,introduced=False)),plan()],False),('stale',[confirm(),confirm(token=6)],True),('clock_back',[confirm(),plan(at=2)],True),('visit_back',[confirm(),plan(visit=1)],True),('bad_receipt',[confirm(heard=200)],True),('bad_gap',[plan()],True)]
 out=[]
 for name,ops,error in rows:
  v=args(ops)
  if name=='bad_gap':v['config']['long_gap_turns']=0
  out.append((name,v,error))
 state=oracle(args([confirm()]))['state']
 out.append(('recover_return',dict(config=dict(version=1,long_gap_turns=12),state=state,ops=[plan(visit=6,at=8)]),False))
 out.append(('recover_duplicate',dict(config=dict(version=1,long_gap_turns=12),state=state,ops=[confirm()]),False))
 out.append(('unperceived_fact',args([plan(global_sunlight=True)]),True))
 full=dict(version=1,revision=256,entries=[dict(actor='merchant',listener=f'p_{i}',first_at=0,last_at=0,token=1,scene=None,visit=None,exchanges=1,greetings=0,introduced=False,legacy=True,receipt=None)for i in range(256)])
 out.append(('capacity',dict(config=dict(version=1,long_gap_turns=12),state=full,ops=[dict(op='seed',args=dict(actor='merchant',listener='overflow',at=0,token=1,introduced=False))]),True))
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,params,error in cases():
  try:wanted=oracle(copy.deepcopy(params));assert not error
  except(AssertionError,KeyError,TypeError):assert error;wanted=None
  req=dict(kind='studio-greetings',args=params)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(req),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(wanted),(name,r.stdout,wanted)
  if a.write_conformance:
   d=ROOT/'conformance/greeting-memory'/name;d.mkdir(parents=True,exist_ok=True)
   for k,v in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',req)]+([]if error else[('expected',wanted)]):
    file=d/(k+'.json');assert not file.exists();file.write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),actual_provider_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
