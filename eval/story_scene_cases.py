"""Independent bounded seed rotation/tale framing plus existing scoped location lookup."""
import argparse,copy,json,pathlib,subprocess,sys
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'))
import expression_cases as expression
import location_reference

def oracle(v):
 op=v['op'];a=v['args']
 if op=='seed':
  assert set(a)=={'candidates','recent','offset'};c=a['candidates'];r=a['recent'];o=a['offset'];assert type(o)is int and 0<=o<=1000000 and 1<=len(c)<=32 and len(r)<=32 and len(set(c))==len(c)
  for key in c+r:assert isinstance(key,str)and 1<=len(key)<=64 and all(ch.isascii()and(ch.isalnum()or ch in '_:-')for ch in key)
  order=[(o+i)%len(c)for i in range(len(c))];unseen=[i for i in order if c[i]not in r]
  i=unseen[0]if unseen else max(order,key=lambda i:r.index(c[i]));return dict(index=i,reused=not bool(unseen))
 assert op=='tale'and set(a)=={'opening','ending'}
 for key,prefix in [('opening','Imagine '),('ending','In this tale, ')]:
  assert a[key].startswith(prefix);expression.oracle(dict(op='validate',args=dict(mode='fiction',speech=a[key])))
 return dict(speech=[a['opening'],a['ending']])
def cases():
 c=['bell','leaf','boat'];rows=[]
 for name,recent,offset,want in [('first',[],0,0),('unheard_first',['bell'],0,1),('unheard_last',['bell','leaf'],0,2),('least_recent',['boat','bell','leaf'],0,1),('offset',[],2,2),('unknown_ignored',['other','leaf'],1,2)]:
  v=dict(op='seed',args=dict(candidates=c,recent=recent,offset=offset));assert oracle(v)['index']==want;rows.append((name,v,False))
 for name,args in [('duplicate',dict(candidates=['bell','bell'],recent=[],offset=0)),('empty',dict(candidates=[],recent=[],offset=0)),('overflow',dict(candidates=c,recent=[],offset=1000001)),('bad_key',dict(candidates=['bad key'],recent=[],offset=0)),('too_many',dict(candidates=[str(i)for i in range(33)],recent=[],offset=0))]:rows.append((name,dict(op='seed',args=args),True))
 tale=dict(opening='Imagine a bell afraid to ring alone.',ending='In this tale, a leaf tapped along until their little duet filled the room.')
 rows.append(('tale',dict(op='tale',args=tale),False))
 for name,patch in [('backwards',dict(opening='In this tale, a bell found its courage.')),('missing_ending',dict(ending='Imagine a bell too small to ring.')),('cutoff',dict(ending='In this tale, a bell that ')),('meta',dict(ending='In this tale, the host fixed it.'))]:rows.append((name,dict(op='tale',args=tale|patch),True))
 return rows
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:wanted=oracle(args);assert not error,name
  except(AssertionError,KeyError,TypeError,ValueError):assert error,name;wanted=None
  req=dict(kind='studio-expression',args=args)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=json.dumps(req),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stdout,r.stderr)
   if not error:assert json.loads(r.stdout)==wanted,name
  if a.write_conformance:
   d=ROOT/'conformance/expression'/('story_'+name);assert not d.exists();d.mkdir(parents=True)
   for k,v in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',req)]+([]if error else[('expected',wanted)]):(d/(k+'.json')).write_text(json.dumps(v,separators=(',',':'))+'\n')
 print(json.dumps(dict(workflows=len(cases()),real_model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
