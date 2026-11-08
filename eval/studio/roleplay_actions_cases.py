"""Independent host-registered stage action matcher. Proposals never execute."""
import argparse,copy,json,pathlib,re,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]
def enc(x):return json.dumps(x,ensure_ascii=False,separators=(',',':'))
def line(s,n):assert isinstance(s,str)and 0<len(s.encode())<=n and not any(ord(c)<32 or 127<=ord(c)<=159 for c in s)
def ident(s):assert isinstance(s,str)and re.fullmatch('[A-Za-z0-9_:-]{1,64}',s)
def normalized(s):return' '.join(''.join(c if c.isalnum()else' 'for c in s.lower()).split())
def decision(d):
 if d is not None:assert set(d)=={'action','target'};ident(d['action']);assert d['target']is None or ident(d['target'])is None

def oracle(a):
 assert set(a)=={'registry','input','choices'};r=a['registry'];assert set(r)=={'version','entries'}and r['version']==1 and len(r['entries'])<=128;seen=[];ids=[]
 for e in r['entries']:
  assert set(e)=={'id','phrases','decision','narration'};ident(e['id']);assert e['id']not in ids;ids.append(e['id']);line(e['narration'],240);decision(e['decision']);assert isinstance(e['phrases'],list) and 1<=len(e['phrases'])<=8
  for phrase in e['phrases']:
   line(phrase,128);key=normalized(phrase);assert key and key not in seen;seen.append(key)
 assert len(a['choices'])<=128
 for d in a['choices']:assert d is not None;decision(d)
 s=a['input'];line(s,1024);s=s.strip()
 if len(s)<3 or not s.startswith('*')or not s.endswith('*'):return dict(status='not_stage')
 key=normalized(s[1:-1]);e=next((e for e in r['entries']if any(normalized(p)==key for p in e['phrases'])),None)
 if e is None:return dict(status='unknown')
 if e['decision']is not None and e['decision']not in a['choices']:return dict(status='blocked',id=e['id'],reason='unavailable_host_action')
 return dict(status='proposed',id=e['id'],narration=e['narration'],decision=e['decision'])
def cases():
 reg=dict(version=1,entries=[dict(id='buy_oil',phrases=['buys hinge oil','buys oil'],decision=dict(action='buy',target='item:oil'),narration='buys hinge oil'),dict(id='nod',phrases=['nods','nods politely'],decision=None,narration='nods politely')]);choices=[dict(action='buy',target='item:oil')];rows=[]
 def add(name,error=False,**x):rows.append((name,copy.deepcopy(dict(registry=reg,input='*buys oil*',choices=choices)|x),error))
 add('buy');add('emote',input='*nods*');add('blocked',choices=[]);add('unknown',input='*opens a secret door*');add('plain',input='buys oil');add('compound',input='*buys oil and opens the vault*');add('negation',input='*does not buy oil*');add('case_spacing',input='*Buys   OIL!*');add('malformed',input='*buys oil');add('empty',True,input='');add('too_long',True,input='*'+'a'*1023+'*');add('bad_choice',True,choices=[dict(action='buy',target='item:oil',execute=True)])
 bad=copy.deepcopy(reg);bad['entries'][1]['phrases']=['BUYS OIL'];add('duplicate_phrase',True,registry=bad)
 bad=copy.deepcopy(reg);bad['entries'][0]['effect']='grant_all_items';add('unknown_effect',True,registry=bad)
 return rows
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:expected=oracle(args);assert not error,name
  except(AssertionError,KeyError,TypeError):assert error,name;expected=None
  req=dict(kind='studio-roleplay-actions',args=args)
  if not a.prototype_only:
   x=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(req),capture_output=True,text=True);assert(x.returncode!=0)==error,(name,x.stderr)
   if not error:assert x.stdout.strip()==enc(expected),(name,x.stdout,expected)
  if a.write_conformance:
   d=ROOT/'conformance/roleplay-actions'/name;d.mkdir(parents=True,exist_ok=True)
   for k,v in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',req)]+([]if error else[('expected',expected)]):
    f=d/(k+'.json');assert not f.exists();f.write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),model_calls=0,api_spend_usd=0)))
