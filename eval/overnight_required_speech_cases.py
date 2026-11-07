"""Predeclared required-answer contract: generic deferral cannot replace known cards."""
import argparse,copy,json,pathlib,subprocess,sys
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'))
from lore_reference import Answers
from life_speech_reference import assess_sentence

def enc(x):return json.dumps(x,ensure_ascii=False,separators=(',',':'))
def expected(a):
 r=Answers(a['answers']);cards=r.value['cards'];render=lambda c:('You told me: "'+c['text']+'"') if c['kind']=='quote' else c['text'];values=[render(c) for c in cards];assert values and len(set(values))==len(values)
 assert all(len(s)<=240 and assess_sentence(s,240)['boundary_complete'] for s in values)
 results=[]
 for call in a['calls']:
  try:
   def unique(pairs):
    out={}
    for k,v in pairs:assert k not in out;out[k]=v
    return out
   raw=json.loads(call['text'],object_pairs_hook=unique);assert set(raw)=={'speech'};card=next(c for c in cards if render(c)==raw['speech']);result=r.resolve(enc(dict(answer=card['id'])),call['context'],call['available']);results.append(dict(ok=True,result=result))
  except (AssertionError,ValueError,KeyError,TypeError,StopIteration):results.append(dict(ok=False))
 return dict(schema=dict(type='object',properties=dict(speech=dict(type='string',enum=values)),required=['speech'],additionalProperties=False),results=results)
def cases():
 a=dict(version=1,scope='required_reply',revision=4,unknown="I don't know.",cards=[dict(id='provided',subject='player_1',topic='preference',kind='state',source='confirmed_quote',text='You told me you prefer copper wicks.')]);context=dict(scope=a['scope'],revision=4);available=[dict(kind='state',id='confirmed_quote')]
 calls=[dict(text=enc(dict(speech=t)),context=context,available=available) for t in [a['cards'][0]['text'],a['unknown'],'An invented claim.']];calls += [dict(calls[0],available=[]),dict(calls[0],context=dict(context,revision=5))]
 out=[('known_answer',dict(answers=a,calls=calls),False)]
 b=copy.deepcopy(a);b['cards'][0]['text']=b['unknown'];out.append(('known_absence_keeps_source',dict(answers=b,calls=[dict(calls[0],text=enc(dict(speech=b['unknown']))),dict(calls[0],text=enc(dict(speech=b['unknown'])),available=[])]),False))
 for name,mutate in [('empty',lambda b:b.update(cards=[])),('duplicate',lambda b:b['cards'].append(dict(b['cards'][0],id='other')))]:
  b=copy.deepcopy(a);mutate(b);out.append((name,dict(answers=b,calls=[]),True))
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:wanted=expected(args);assert not error
  except (AssertionError,ValueError,KeyError,TypeError):assert error;wanted=None
  request=dict(kind='required-speech',args=args)
  if not a.prototype_only:
   cp=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(request),capture_output=True,text=True);assert bool(cp.returncode)==error,(name,cp.stderr)
   if not error:assert cp.stdout.strip()==enc(wanted),(name,cp.stdout,wanted)
  if a.write_conformance:
   d=ROOT/'conformance/deep-required-speech'/name;d.mkdir(parents=True,exist_ok=True)
   for f,v in [('case',dict(op='local',**({'error':True} if error else {}))),('input',request)]+([] if error else [('expected',wanted)]): (d/(f+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),actual_provider_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
