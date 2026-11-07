"""Predeclared small English command grammar: questions/ambiguity never execute."""
import re
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
  if arg in ['it','that one','this'] and focus==target:names+=[arg]
  if arg in names:candidates.append(c)
 return dict(status='resolved',decision=candidates[0]) if len(candidates)==1 else dict(status='clarify',decision=None)
def main():
 choices=[dict(action=a,target=t) for a,t in [('buy','item:oil'),('buy','item:chalk'),('inspect','item:oil'),('go','room:gallery')]];labels={'item:oil':['hinge oil']}
 for text,action,target in CASES:
  r=resolve(text,choices,labels,'item:oil');assert r['status']==(action if action in ['dialogue','clarify'] else 'resolved'),(text,r)
  if r['decision']:assert r['decision']==dict(action=action,target=target)
 print({'prototype_cases':len(CASES),'model_calls':0,'api_spend_usd':0})
if __name__=='__main__':main()
