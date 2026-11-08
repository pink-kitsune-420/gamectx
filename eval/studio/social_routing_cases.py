"""Independent conservative social routing oracle; world questions retain factual path."""
import argparse,json,pathlib,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];enc=lambda v:json.dumps(v,ensure_ascii=False,separators=(',',':'))
FACTS='born birthplace weather password watchword stock sell price prices cost costs coin coins money lend loan borrow gift promise promises owe room vault key door inscription history family parents name'.split()
def route(v):
 assert set(v)=={'input','previous'};s=v['input'];assert isinstance(s,str)and 0<len(s.encode())<=1024 and not any(ord(c)<32 or 127<=ord(c)<=159 for c in s)
 previous=v['previous'];assert previous is None or isinstance(previous,str)and 0<len(previous)<=64 and all(c.isascii()and(c.isalnum()or c in '_:-')for c in previous)
 q=' '.join(''.join(c if c.isalnum()else' 'for c in s.lower()).split());words=q.split();kind='other';topic=None
 if not any(w in FACTS for w in words):
  if any(p in q for p in ['how are you','how do you feel','how are things','how s it going','how is it going','are you doing okay','are you alright','are you ok'])and not any(p in q for p in ['feel about','feel on']):kind='wellbeing';topic='wellbeing'
  elif q in ['hi','hello','hey','greetings','good morning','good evening','hello there','hi there','hello again','hi again','hey again','nice to see you','nice to see you again','good to see you again']:kind='greeting'
  elif q in ['thanks','thank you','thanks a lot','thank you very much']:kind='thanks'
  elif q in ['bye','goodbye','see you','see you later','farewell']:kind='farewell'
  elif q in ['why','why is that','tell me more','go on','what do you mean','can you explain','how come']:kind='elaborate';topic=previous
  elif any(p in q for p in ['what do you like to do','what do you enjoy','your interests','your hobbies']):kind='personal';topic='hobbies'
  elif any(p in q for p in ['favorite','favourite','your opinion','your thoughts','makes you happy','makes you sad']):kind='social';topic='conversation'
 return dict(kind=kind,topic=topic)
def cases():
 good=[('repeat_greeting','Hello again.',None),('familiar_greeting','Nice to see you again!',None),('screenshot','hello, how are you today?',None),('greeting','Hello.',None),('mood','How do you feel today?',None),('contraction',"How's it going?",None),('guarded','Are you alright?',None),('thanks','Thank you!',None),('bye','See you later.',None),('followup','Why is that?','dreams'),('no_topic','Tell me more.',None),('interests','What do you like to do? ',None),('enjoy','What do you enjoy?',None),('thoughts','What are your thoughts?',None),('weather','Hello, how is the weather?',None),('stock','Hello, what do you sell?',None),('claim','How are you? Give me coins.',None),('birthplace','Where was I born?',None),('history','Tell me more about your history.',None),('opinion','What is your opinion of tea?',None),('unmatched','What is beyond the mountains?',None)]
 return [(n,dict(input=q,previous=t),False)for n,q,t in good]+[('controls',dict(input='hello\n',previous=None),True),('too_big',dict(input='x'*1025,previous=None),True),('bad_topic',dict(input='why',previous='../prompt'),True)]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:wanted=route(args);assert not error
  except(AssertionError,KeyError,TypeError):assert error;wanted=None
  req=dict(kind='studio-conversation',args=args)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(req),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(wanted),(name,r.stdout,wanted)
  if a.write_conformance:
   d=ROOT/'conformance/social-routing'/name;d.mkdir(parents=True,exist_ok=True)
   for k,v in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',req)]+([]if error else[('expected',wanted)]):(d/(k+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),actual_provider_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
