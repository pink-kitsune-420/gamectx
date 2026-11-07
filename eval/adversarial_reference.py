"""Independent proposed guards for the adversarial probes; no model calls."""
import re
ACTIONS={'take','inspect','go','look','wait','unlock','attack','heal'}
def tokens(s):return re.findall(r'[^\W_]+',s.lower(),re.UNICODE)
def literal_action(state,utterance):
 words=tokens(utterance)
 if not words or words[0] not in ACTIONS or '?' in utterance or any(ord(c)<32 for c in utterance):return None
 action=words[0]
 if not re.match(r'^\s*'+action+r'(?:\s|$)',utterance,re.I):return None
 tail=words[1:]
 if tail and tail[-1]=='please':tail=tail[:-1]
 prep={'go':'to','inspect':'at','look':'at','heal':'with'}.get(action)
 if tail and tail[0]==prep:tail=tail[1:]
 if tail and tail[0] in ['a','an','the']:tail=tail[1:]
 found=[]
 for pair in state['choices']:
  if pair['action']!=action:continue
  if pair['target'] is None:
   if not tail:found.append(pair)
   continue
  aliases=[pair['target'],pair['target'].split(':')[-1]]
  for item in state['items']:
   if item['id']==pair['target']:aliases.extend([item['label'],item['kind']])
  if any(tail==tokens(s) for s in aliases):found.append(pair)
 return found[0] if len(found)==1 else None

def conflict(records,now,allowed):
 spans=[]
 for o in records:
  t=o['time']
  if not t or not t['usable'] or t['source'] not in allowed or not o['at']<=now<o['expires']:continue
  age=now-o['at'];lo=t['low']+age*t['rate_low'];hi=t['high']+age*t['rate_high']
  if hi<=2**32-1:spans.append((lo,hi))
 return bool(spans) and max(lo for lo,_ in spans)>min(hi for _,hi in spans)

def row_time_valid(row,activity,now):
 if row['state']=='planned':return row['since']<=now
 if not row['start']<=row['since']<=now:return False
 if row['state']=='interrupted' and activity['resume']!='resume':return False
 if row['state']=='cancelled' and row['reason']=='preempted' and activity['resume']!='cancel':return False
 return True

def assessment(knowledge):
 sources=knowledge['sources']
 if knowledge['conflicting']:kind='conflicting'
 elif knowledge['status']=='unknown' or not sources:kind='unknown'
 elif any(s['precision']=='exact' for s in sources):kind='exact_reading'
 elif all(s['basis']=='report' for s in sources):kind='reported'
 else:kind='approximate'
 return dict(kind=kind,knowledge=knowledge)
