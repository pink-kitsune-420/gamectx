"""Original independent oracle for delivery-confirmed, listener-scoped mental stances."""
import argparse,json,pathlib,re,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]
enc=lambda v:json.dumps(v,ensure_ascii=False,separators=(',',':'))
def line(s):
 assert isinstance(s,str) and 1<=len(s)<=240 and len(s.encode())<=1024 and not any(ord(c)<32 or ord(c)==127 for c in s)
 assert any(s.lower().startswith(p) for p in ['i hope ','i dream ','i wish ','i prefer ','i feel ','i value ','i think ','i believe ','i would like '])
 assert s[-1] in '.!' and not any(x in s[:-1] for x in '.!?')
 assert not any(w in s.lower() for w in ['was born','were born','used to','yesterday','years ago','i own ','i have ','i promise','will give','stock','weather','sunny','raining','copper sparrow'])
def row(v):
 assert set(v)=={'actor','listener','topic','stance','basis','source','at'}
 for k in ['actor','listener','topic']:assert isinstance(v[k],str)and re.fullmatch('[a-zA-Z0-9_:-]{1,64}',v[k])
 assert v['actor']!=v['listener'];line(v['stance']);assert re.fullmatch('[0-9a-f]{64}',v['basis'])
 assert v['source']in ['profile','model']and type(v['at'])is int and 0<=v['at']<=4294967295
 return(v['actor'],v['listener'],v['topic'])
def oracle(v):
 assert set(v)=={'state','ops'};state=v['state'];assert set(state)=={'version','entries'}and state['version']==1 and len(state['entries'])<=128
 keys=[];entries=[]
 for e in state['entries']:
  key=row(e);assert key not in keys;keys.append(key);entries.append(e)
 assert len(v['ops'])<=128;out=[]
 for op in v['ops']:
  assert set(op)=={'entry','heard'};e=op['entry'];key=row(e);assert op['heard']==e['stance']
  if key in keys:assert entries[keys.index(key)]==e
  else:assert len(entries)<128;keys.append(key);entries.append(e)
  out.append(dict(stored=True,count=len(entries)))
 return dict(results=out,state=dict(version=1,entries=entries))
def cases():
 base=dict(actor='merchant',listener='player_1',topic='dreams',stance='I hope to make this counter feel welcoming.',basis='a'*64,source='model',at=4)
 op=lambda e,heard=None:dict(entry=e,heard=e['stance']if heard is None else heard)
 state=dict(version=1,entries=[]);args=lambda ops:dict(state=state,ops=ops)
 return [('confirmed',args([op(base)]),False),('reuse',args([op(base),op(base)]),False),('listener_isolation',args([op(base),op(base|dict(listener='player_2',stance='I dream of a quieter day.'))]),False),('actor_isolation',args([op(base),op(base|dict(actor='keeper'))]),False),('partial',args([op(base,'I hope')]),True),('changed_stance',args([op(base),op(base|dict(stance='I prefer to be alone.'))]),True),('factual_claim',args([op(base|dict(stance='I was born in a city.'))]),True),('hidden_lore',args([op(base|dict(stance='I think the password is copper sparrow.'))]),True),('cutoff',args([op(base|dict(stance='I hope that,'))]),True),('action',args([op(base|dict(stance='I hope you know I will give you coins.'))]),True),('unknown_field',args([op(base|dict(trust=100))]),True),('bad_basis',args([op(base|dict(basis='trusted'))]),True)]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:wanted=oracle(args);assert not error
  except(AssertionError,KeyError,TypeError):assert error;wanted=None
  req=dict(kind='studio-viewpoints',args=args)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(req),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(wanted),(name,r.stdout,wanted)
  if a.write_conformance:
   d=ROOT/'conformance/viewpoints'/name;d.mkdir(parents=True,exist_ok=True)
   for k,v in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',req)]+([]if error else[('expected',wanted)]):(d/(k+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),actual_provider_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
