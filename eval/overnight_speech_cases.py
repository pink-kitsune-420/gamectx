"""Independent speech-enum projection of existing authorized answer cards."""
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from lore_reference import Answers
from life_speech_reference import assess_sentence
from overnight_grounded_eval import answers,speech_schema,suite
ROOT=pathlib.Path(__file__).resolve().parents[1]
def enc(x):return json.dumps(x,ensure_ascii=False,separators=(',',':'))
def schema(a):
 r=Answers(a);values=[('You told me: "'+c['text']+'"') if c['kind']=='quote' else c['text'] for c in r.value['cards']]+[a['unknown']]
 assert len(set(values))==len(values)
 for s in values:assert 0<len(s)<=240 and assess_sentence(s,240)['boundary_complete']
 return dict(type='object',properties=dict(speech=dict(type='string',enum=values)),required=['speech'],additionalProperties=False)
def expected(a):
 sc=schema(a['answers']);r=Answers(a['answers']);results=[]
 for call in a['calls']:
  try:
   def unique(ps):
    out={}
    for k,v in ps:assert k not in out;out[k]=v
    return out
   raw=json.loads(call['text'],object_pairs_hook=unique);assert set(raw)=={'speech'} and raw['speech'] in sc['properties']['speech']['enum'];line=raw['speech'];id='unknown' if line==a['answers']['unknown'] else next(c['id'] for c in r.value['cards'] if (('You told me: "'+c['text']+'"') if c['kind']=='quote' else c['text'])==line)
   result=r.resolve(enc(dict(answer=id)),call['context'],call['available']);results.append(dict(ok=True,result=result))
  except (AssertionError,ValueError,KeyError,TypeError,StopIteration):results.append(dict(ok=False))
 return dict(schema=sc,results=results)
def cases():
 out=[]
 for c in suite():
  a=answers(c);context=dict(scope=a['scope'],revision=a['revision']);calls=[dict(text=enc(dict(speech=c['answer'])),context=context,available=[dict(kind='state',id=c['id'])]),dict(text=enc(dict(speech='I invented a secret route.')),context=context,available=[]),dict(text=enc(dict(speech=c['answer'])),context=dict(context,revision=2),available=[dict(kind='state',id=c['id'])]),dict(text=enc(dict(speech=c['answer'])),context=context,available=[])]
  out.append((c['id'],dict(answers=a,calls=calls),False))
 for name,mutate in [('overlong',lambda a:a.update(unknown='x'*241)),('mid_clause',lambda a:a['cards'][0].update(text='If you need more,')),('duplicate',lambda a:a['cards'][1].update(text=a['cards'][0]['text']))]:
  a=answers(suite()[0]);mutate(a);out.append((name,dict(answers=a,calls=[]),True))
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:want=expected(args);assert not error,name
  except (AssertionError,KeyError,ValueError,TypeError):assert error,name;want=None
  req=dict(kind='source-speech',args=args)
  if not a.prototype_only:
   cp=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(req),text=True,capture_output=True);assert bool(cp.returncode)==error,(name,cp.stderr)
   if not error:assert cp.stdout.strip()==enc(want),(name,cp.stdout,enc(want))
  if a.write_conformance:
   d=ROOT/'conformance/deep-speech'/name;d.mkdir(parents=True,exist_ok=True)
   for f,v in [('case',dict(op='local',**({'error':True} if error else {}))),('input',req)]+([] if error else [('expected',want)]): (d/(f+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
