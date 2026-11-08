"""Independent reasoning-control and unverified hypothesis contract. No inference."""
import argparse,copy,json,pathlib,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]
def enc(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def bounded(s,n):assert isinstance(s,str)and 0<len(s.encode())<=n and not any(ord(c)<32 or 127<=ord(c)<=159 for c in s)
def config(c):
 assert set(c)=={'version','mode','effort','max_tokens','hypotheses','online'}and c['version']==1 and c['mode']in ['off','auto']and c['effort']in ['low','medium','high']and type(c['max_tokens'])is int and 128<=c['max_tokens']<=4096 and type(c['hypotheses'])is bool and len(c['online'])<=16
 seen=[]
 for p in c['online']:
  assert set(p)=={'model','values'};bounded(p['model'],128);assert p['model']not in seen;seen.append(p['model']);assert 1<=len(p['values'])<=8 and len(set(p['values']))==len(p['values'])and all(v in ['none','minimal','low','medium','high','xhigh','max']for v in p['values'])
def controls(v):
 assert v is None or isinstance(v,list)and len(v)<=8
 if v is not None:
  assert len({enc(x)for x in v})==len(v)
  for x in v:
   assert type(x)is bool or isinstance(x,str)
   if isinstance(x,str):bounded(x,24);assert all(c.isascii()and(c.isalnum()or c=='_')for c in x)
def oracle(a):
 assert set(a)=={'op','args'};v=a['args']
 if a['op']=='plan':
  assert set(v)=={'config','backend','model','values','task','expanded','output_tokens'};c=v['config'];config(c);assert v['backend']in ['local_ollama','online_openai_compatible'];bounded(v['model'],128);controls(v['values']);assert v['task']in ['routine','planning','conflict','investigation']and type(v['expanded'])is bool and type(v['output_tokens'])is int and 1<=v['output_tokens']<=512
  values=v['values']if v['backend']=='local_ollama'else next((p['values']for p in c['online']if p['model']==v['model']),None)
  desired=c['mode']=='auto'and v['task']!='routine';chosen=None
  if c['mode']=='auto'and values is not None:
   if desired:chosen=c['effort']if c['effort']in values else True if any(type(x)is bool and x for x in values)else None
   else:chosen=False if any(type(x)is bool and not x for x in values)else 'none'if 'none'in values else None
  active=desired and chosen is not None
  return dict(active=active,reason='disabled'if c['mode']=='off'else 'routine'if not desired else 'enabled'if active else 'controls_unavailable',control=None if chosen is None else dict(field='think'if v['backend']=='local_ollama'else 'reasoning_effort',value=chosen),max_tokens=max(v['output_tokens'],c['max_tokens'])if active else v['output_tokens'],hypotheses=desired and c['hypotheses']and v['expanded'])
 assert a['op']=='hypotheses'and set(v)=={'notes','evidence'}and len(v['notes'])<=3 and len(v['evidence'])<=16
 for k in v['evidence']:bounded(k,64)
 for n in v['notes']:
  assert set(n)=={'claim','evidence','check'};bounded(n['claim'],180);bounded(n['check'],180);assert 1<=len(n['evidence'])<=4 and len(set(n['evidence']))==len(n['evidence'])and all(e in v['evidence']for e in n['evidence'])
 return dict(status='unverified',notes=v['notes'])
def cases():
 c=dict(version=1,mode='auto',effort='low',max_tokens=768,hypotheses=True,online=[])
 v=dict(config=c,backend='local_ollama',model='example-model',values=[True,False],task='planning',expanded=True,output_tokens=64)
 rows=[]
 def plan(name,error=False,**x):rows.append((name,dict(op='plan',args=copy.deepcopy(v|x)),error))
 plan('enabled');plan('routine',task='routine');plan('unknown',values=None);plan('unsupported',values=[False]);plan('levels',values=['low','medium','high']);plan('wrong_level',values=['high']);plan('mandatory_routine',values=['low'],task='routine');plan('focused',expanded=False);plan('off',config=c|dict(mode='off'));plan('invalid_control',True,values=[1]);plan('invalid_task',True,task='weather');plan('duplicate_control',True,values=[True,True]);plan('zero_budget',True,config=c|dict(max_tokens=0));plan('online_unknown',backend='online_openai_compatible');plan('online_pinned',backend='online_openai_compatible',config=c|dict(online=[dict(model='example-model',values=['low','none'])]));plan('online_other',backend='online_openai_compatible',config=c|dict(online=[dict(model='other',values=['low'])]));plan('online_routine',backend='online_openai_compatible',task='routine',config=c|dict(online=[dict(model='example-model',values=['low','none'])]));plan('bad_online',True,config=c|dict(online=[dict(model='example-model',values=['surprise'])]));plan('bad_bytes',True,values=['\n']);plan('budget_shared',output_tokens=512,config=c|dict(max_tokens=128))
 n=dict(claim='The hinge might be stuck.',evidence=['visible_scene'],check='Inspect the hinge before acting.')
 for name,notes,error in [('hypothesis',[n],False),('empty_notes',[],False),('unseen_evidence',[n|dict(evidence=['secret'])],True),('self_verified',[n|dict(verified=True)],True),('no_basis',[n|dict(evidence=[])],True),('too_many',[n]*4,True),('oversize',[n|dict(claim='a'*181)],True),('control_text',[n|dict(check='a\nb')],True)]:rows.append((name,dict(op='hypotheses',args=dict(notes=notes,evidence=['visible_scene'])),error))
 return rows
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:expected=oracle(copy.deepcopy(args));assert not error,name
  except(AssertionError,KeyError,TypeError):assert error,name;expected=None
  request=dict(kind='studio-deliberation',args=args)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(request),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(expected),(name,r.stdout,expected)
  if a.write_conformance:
   d=ROOT/'conformance/deliberation'/name;d.mkdir(parents=True,exist_ok=True)
   for key,value in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',request)]+([]if error else[('expected',expected)]):
    f=d/(key+'.json');assert not f.exists();f.write_text(enc(value)+'\n')
 print(enc(dict(workflows=len(cases()),provider_calls=0,api_spend_usd=0)))
