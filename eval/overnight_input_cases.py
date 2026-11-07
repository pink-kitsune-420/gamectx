"""Predeclared small English command grammar: questions/ambiguity never execute."""
import re,argparse,json,pathlib,subprocess
CASES=[('buy oil','buy','item:oil'),('Please buy the oil.','buy','item:oil'),("I'd like to purchase hinge oil",'buy','item:oil'),('purchase oil please','buy','item:oil'),('go to the gallery','go','room:gallery'),('walk to gallery please','go','room:gallery'),('inspect the oil','inspect','item:oil'),('buy it','buy','item:oil'),('buy that one','buy','item:oil'),('buy oill','clarify',None),('buy oil and chalk','clarify',None),('buy two oil','clarify',None),('can I buy oil?','dialogue',None),('do not buy oil','dialogue',None),('buy oil if it is free','clarify',None),('tell me about him','dialogue',None),('buy unlisted ruby','clarify',None),('good morning','dialogue',None)]
# Host lists current legal choices and focus; never scan arbitrary prose for a verb.
def resolve(text,choices,labels,focus=None):
 s=' '.join(text.lower().strip().rstrip('.!').split())
 if '?' in s or any(w in s.split() for w in ['not','never',"don't",'dont','without']):return dict(status='dialogue',decision=None)
 for prefix in ['please ',"i'd like to ",'i would like to ']:
  if s.startswith(prefix):s=s[len(prefix):];break
 if s.endswith(' please'):s=s[:-7]
 aliases={'purchase':'buy','walk':'go','travel':'go','examine':'inspect'}
 first,_,arg=s.partition(' ');action=aliases.get(first,first)
 if action not in ['go','buy','take','inspect','unlock','talk','attack','heal','look','wait']:return dict(status='dialogue',decision=None)
 if action=='go' and arg.startswith('to '):arg=arg[3:]
 if arg.startswith('the '):arg=arg[4:]
 candidates=[]
 for c in choices:
  if c['action']!=action:continue
  target=c['target'];names=[target.split(':')[-1].replace('_',' ')] if target else ['']
  names+=labels.get(target,[])
  if target=='door:vault':names+=['gate','door']
  if arg in ['it','that one','this','him','her','them','there'] and focus==target:names+=[arg]
  if arg in names or action=='look' and not arg:candidates.append(c)
 return dict(status='resolved',decision=candidates[0]) if len(candidates)==1 else dict(status='clarify',decision=None)
def fixtures():
 choices=[dict(action=a,target=t) for a,t in [('buy','item:oil'),('buy','item:chalk'),('inspect','item:oil'),('go','room:gallery')]];labels={'item:oil':['hinge oil']}
 out=[]
 for i,(text,action,target) in enumerate(CASES):
  req=dict(input=text,choices=choices,labels=[dict(id=k,aliases=v) for k,v in labels.items()],focus='item:oil')
  wanted=dict(status=action if action in ['dialogue','clarify'] else 'resolved',decision=None if action in ['dialogue','clarify'] else dict(action=action,target=target))
  assert resolve(text,choices,labels,'item:oil')==wanted
  out.append(('phrase_'+str(i),req,wanted,False))
 for text,choices,want in [('look',[dict(action='look',target='room:gallery')],dict(action='look',target='room:gallery')),('wait',[dict(action='wait',target=None)],dict(action='wait',target=None))]:
  out.append((text,dict(input=text,choices=choices,labels=[],focus=None),dict(status='resolved',decision=want),False))
 for phrase,action,target in [('talk him','talk','npc:merchant'),('talk her','talk','npc:keeper'),('walk there','go','room:gallery')]:
  decision=dict(action=action,target=target);out.append((phrase.replace(' ','_'),dict(input=phrase,choices=[decision],labels=[],focus=target),dict(status='resolved',decision=decision),False))
 for name,field,value in [('control','input','buy oil\n'),('duplicate_choices','choices',choices+[choices[0]]),('bad_focus','focus',4)]:
  req=dict(out[0][1]);req[field]=value;out.append((name,req,None,True))
 return out
def prototype(q):
 assert set(q)==set('input choices labels focus'.split())
 def scope(s):return isinstance(s,str) and 0<len(s)<=64 and not any(ord(c)<32 or ord(c)==127 for c in s)
 assert isinstance(q['input'],str) and 0<len(q['input'].encode())<=1024 and not any(ord(c)<32 or ord(c)==127 for c in q['input'])
 assert isinstance(q['choices'],list) and len(q['choices'])<=128
 seen=[]
 for c in q['choices']:
  assert set(c)=={'action','target'} and scope(c['action']) and (c['target'] is None or scope(c['target'])) and c not in seen;seen.append(c)
 assert q['focus'] is None or scope(q['focus'])
 assert isinstance(q['labels'],list) and len(q['labels'])<=128
 labels={}
 for l in q['labels']:
  assert set(l)=={'id','aliases'} and scope(l['id']) and l['id'] not in labels and isinstance(l['aliases'],list) and len(l['aliases'])<=8
  assert all(isinstance(s,str) and 0<len(s)<=128 and not any(ord(c)<32 or ord(c)==127 for c in s) for s in l['aliases']);labels[l['id']]=l['aliases']
 return resolve(q['input'],q['choices'],labels,q['focus'])
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 root=pathlib.Path(__file__).resolve().parents[1]
 enc=lambda v:json.dumps(v,ensure_ascii=False,separators=(',',':'))
 for name,args,wanted,error in fixtures():
  try:actual=prototype(args);assert not error;assert actual==wanted
  except (AssertionError,TypeError,KeyError):assert error
  req=dict(kind='studio-command',args=args)
  if not a.prototype_only:
   cp=subprocess.run([str(root/'target/debug/gamectx'),'local','-'],input=enc(req),capture_output=True,text=True);assert bool(cp.returncode)==error,(name,cp.stderr)
   if not error:assert cp.stdout.strip()==enc(wanted),(name,cp.stdout,wanted)
  if a.write_conformance:
   d=root/'conformance/deep-command'/name;d.mkdir(parents=True,exist_ok=True)
   for f,v in [('case',dict(op='local',**({'error':True} if error else {}))),('input',req)]+([] if error else [('expected',wanted)]): (d/(f+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(prototype_cases=len(fixtures()),model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
