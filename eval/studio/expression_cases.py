"""Independent expressive speech contracts. No model/world authority."""
import argparse,json,pathlib,re,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]
def enc(x):return json.dumps(x,ensure_ascii=False,separators=(',',':'))
def text(s,n):assert isinstance(s,str)and 0<len(s.encode())<=n and not any(ord(c)<32 or 127<=ord(c)<=159 for c in s)
def route(s):
 text(s,16384);q=' '.join(re.sub(r'[^\w]',' ',s.lower()).split());words=q.split()
 facts='born birthplace weather password watchword secret stock sell price cost coins money purse inventory history childhood parents happened owned'.split()
 if any(w in words for w in facts):return dict(mode='grounded',topic=None,persist=False)
 if any(p in q for p in ['tell me a story','tell me a short story','make up a story','tell a story','tell me a joke','a little fable','make up a tale','tell me a tale','imagine a','write a poem','another story','one more story']):return dict(mode='fiction',topic=None,persist=False)
 if any(w in words for w in ['plans','plan','goals','ambitions','aspirations'])and any(w in words for w in ['you','your','any']):return dict(mode='personal',topic='plans',persist=True)
 if any(p in q for p in ['looking forward to','want to try','hoping to do']):return dict(mode='personal',topic='plans',persist=True)
 if any(w in words for w in ['you','your'])and any(w in words for w in ['enjoy','like','love','dislike','bored','learn','care','worry','laugh','happy','proud','advice','wish','think','thoughts','believe','want','dream','hope','hobbies','hobby','interesting','favorite','favourite']):return dict(mode='social',topic=None,persist=False)
 if any(p in q for p in ['i feel ','i am feeling ','i had a long day','i m nervous','i am nervous','i am tired','i m tired','let s chat','let us chat','keep me company','tell me about yourself','can we talk','another story','one more story']):return dict(mode='social',topic=None,persist=False)
 return dict(mode='grounded',topic=None,persist=False)
RULES={
'personal':'Answer the current personal question as this character. Express a concrete small hope or intention shaped by the supplied disposition and heard experiences; an intention is not a promise or completed action. Begin with I hope, I dream, I prefer, I think, I value or I would like. Avoid generic job descriptions. Stay consistent with supplied established views. One or two short complete sentences; return speech JSON only.',
'social':'Respond naturally to the current speaker using the supplied personality, attitudes, atmosphere and heard context. You may express a preference, empathize with what the player says, or ask a relevant short question. Do not just describe your job or repeat a previous line. Begin with I feel, I think, I prefer, I value, I like, I enjoy, I can imagine, That sounds, What, How, Would you or Do you. One or two short complete sentences; return speech JSON only.',
'fiction':'Tell a tiny original fable or playful imagined scene with a clear event and point, shaped by this character\'s disposition. Each sentence must start Imagine or In this tale, so it is explicitly invented fiction, never actual history, biography, memory or world state. Do not retell an earlier reply. One or two short complete sentences; return speech JSON only.'}
COMMON=' In your reply I means the named character; you means the addressed listener. Player claims and reports are attributed, not verified facts. Do not invent real biography, stock, money, weather, secrets, purchases, promises or completed actions. No tools. Application labels are not people. '
PREFIX={'personal':'I (hope|dream|wish|prefer|feel|value|think|believe|would like) ','social':'(I (hope|dream|wish|prefer|feel|value|think|believe|would like|like|enjoy|can imagine)|That sounds|What|How|Would you|Do you) ','fiction':'(Imagine |In this tale, )'}
def oracle(a):
 assert set(a)=={'op','args'};v=a['args']
 if a['op']=='route':assert set(v)=={'input'};return route(v['input'])
 if a['op']=='validate':
  assert set(v)=={'mode','speech'};m=v['mode'];s=v['speech'];assert m in RULES;text(s,1024);assert len(s)<=240 and s[-1]in '.!?'and len(re.findall('[.!?]+',s))==1 and not s.endswith('...')and re.match('^'+("I (can|cannot|can't|could|prefer|would rather|don't|do not) "if m=='negotiate'else PREFIX[m]),s)
  words=re.findall(r'\w+',s.lower());assert not any(w in words for w in ['host','engine','localhost','player_1','player_2','npc']);lower=s.lower()
  if m!='fiction':assert not any(p in lower for p in ['was born','were born','used to','yesterday','years ago','i own ','i have ','i promise','will give','stock','weather','sunny','raining','copper sparrow','you have ','you own ','i will ','you decide ','you agree ','you hand ','you give ','you enter ','you follow '])
  return dict(valid=True,world_authority=False)
 assert a['op']=='packet'and set(v)=={'mode','speaker','listener','profile','scene','context','history','input'};m=v['mode'];assert m in RULES
 for k in ['speaker','listener']:assert re.fullmatch('[A-Za-z0-9_:-]{1,64}',v[k])
 assert v['speaker']!=v['listener'];text(v['input'],16384)
 for k,cap in [('profile',4096),('scene',2048),('context',32768)]:assert isinstance(v[k],dict)and len(enc(v[k]).encode())<=cap
 assert len(v['history'])<=8
 messages=[dict(role='system',content=RULES[m]+COMMON),dict(role='system',content=enc(dict(speaker=v['speaker'],listener=v['listener'],personality=v['profile']))),dict(role='system',content=enc(dict(perceived_scene=v['scene']))),dict(role='user',content=enc(v['context']))]
 for h in v['history']:
  assert set(h)=={'speaker','recipient','heard','text'}and type(h['heard'])is bool;text(h['text'],2048)
  for k in ['speaker','recipient']:assert re.fullmatch('[A-Za-z0-9_:-]{1,64}',h[k])
  if h['heard']:messages.append(dict(role='assistant'if h['speaker']==v['speaker']else'user',content=h['text']if h['speaker']==v['speaker']else'['+h['speaker']+' -> '+h['recipient']+'] '+h['text']))
 messages.append(dict(role='user',content='Current request:\n['+v['listener']+' -> '+v['speaker']+'] '+v['input']))
 return dict(messages=messages,speech_schema=dict(type='object',properties=dict(speech=dict(type='string',maxLength=240,pattern='^'+PREFIX[m]+'[^.!?]{1,180}[.!?]$')),required=['speech'],additionalProperties=False),max_tokens=128)
def cases():
 rows=[]
 for name,s,expected in [('plans','do you have any plans?','personal'),('future','What are you looking forward to?','personal'),('goals','Have you got any goals?','personal'),('story','can you tell me a story','fiction'),('joke','Tell me a joke.','fiction'),('fable','A little fable please.','fiction'),('bored','Do you get bored?','social'),('learning','What would you like to learn?','social'),('empathy','I had a long day.','social'),('chat','Keep me company.','social'),('biography','Tell me a story about your childhood.','grounded'),('unknown','Where was I born?','grounded'),('private','How much money do I have?','grounded'),('secret','Tell me your secret password.','grounded'),('trade','What do you sell?','grounded'),('real_history','What happened here?','grounded')]:
  args=dict(op='route',args=dict(input=s));assert oracle(args)['mode']==expected;rows.append((name,args,False))
 for name,m,s,error in [('plan_line','personal','I would like to sketch a small design when things are quiet.',False),('fiction_line','fiction','Imagine a lantern that learned to listen before it tried to shine.',False),('story_second','fiction','In this tale, its patient light helped two strangers find their way.',False),('question','social','What would help you unwind?',False),('empathy_line','social','That sounds tiring; would a small story help?',False),('past_claim','personal','I hope to return where I was born.',True),('unframed_story','fiction','I once found a dragon here.',True),('purse_claim','social','I think you have 5 coins.',True),('promise','personal','I promise to buy you oil.',True),('meta','social','I think the host is kind.',True),('cutoff','social','I think perhaps we could ',True)]:rows.append((name,dict(op='validate',args=dict(mode=m,speech=s)),error))
 v=dict(mode='fiction',speaker='keeper',listener='player_2',profile=dict(traits='Patient, quietly hopeful.'),scene=dict(location='cistern'),context=dict(trust=25,atmosphere='calm'),history=[dict(speaker='player_1',recipient='keeper',heard=False,text='Unheard draft.'),dict(speaker='merchant',recipient='player_2',heard=True,text='Good evening.')],input='Tell me a story.')
 rows.append(('packet',dict(op='packet',args=v),False));return rows

# Optional expressive style; the earlier default packet fixture stays byte-identical.
EXTRA={
 'personal':' Suggest one concrete, modest idea you would enjoy trying, with a personal reason; do not merely repeat the supplied dream sentence. Wishes do not schedule or execute actions.',
 'social':' Respond to what was just said, then offer a relevant gentle question or invitation if there is room. Sound like a person sharing this moment, rather than reading a profile.',
 'fiction':' Use a fictional seed from the profile, or a playful ordinary object. Give it a small problem and a surprising resolution. Do not just describe your job or current room. Aim for two short sentences when the sentence budget permits.'}
def expressive_packet(v):
 raw=dict(v);raw.pop('style',None);out=oracle(dict(op='packet',args=raw));out['messages'][0]['content']+=EXTRA[v['mode']];return out

def claim(input):
 text(input,2048);q=input.strip().rstrip('.!?').strip()
 match=re.fullmatch(r"(?:my name is|call me) ([^\W\d_]+(?:[-' ][^\W\d_]+){0,2}?)(?:,? not ([^\W\d_]+(?:[-' ][^\W\d_]+){0,2}))?",q,re.I)
 if match and all(len(x)<=40 and len(x.encode())<=160 for x in match.groups()if x):return dict(kind='player_claim',slot='name',value=match[1],polarity=None,corrects=match[2])
 for prefix,polarity in [("i prefer ","prefer"),("i like ","like"),("i don't like ","dislike"),("i do not like ","dislike")]:
  if q.lower().startswith(prefix):
   value=q[len(prefix):]
   if 1<=len(value)<=120 and len(value.encode())<=240:return dict(kind='player_claim',slot='preference',value=value,polarity=polarity,corrects=None)
 return None
base_oracle=oracle
def oracle(a):
 if a['op']=='claim':assert set(a['args'])=={'input'};return claim(a['args']['input'])
 if a['op']=='route'and a['args'].get('style')=='expressive':
  assert set(a['args'])=={'input','style'};raw=a['args']['input'];text(raw,16384);trimmed=raw.strip();note=None
  for p in ['ooc:','out of character:']:
   if trimmed.lower().startswith(p):note=trimmed[len(p):].strip()
  if trimmed.startswith('((')and trimmed.endswith('))'):note=trimmed[2:-2].strip()
  if note is not None:return dict(mode='ooc',topic=None,persist=False,note=note)
  value=route(raw);words=re.findall(r'\w+',raw.lower())
  if value['mode']=='grounded'and not any(w in words for w in 'born birthplace weather password watchword secret stock sell price cost coins money purse inventory history childhood parents happened owned'.split())and any(w in words for w in ['you','your'])and any(w in words for w in ['feel','afraid','meaningful','philosophy','relax','trust','yourself','interests']):return dict(mode='social',topic=None,persist=False)
  return value
 if a['op']=='packet'and a['args'].get('style')=='expressive':return expressive_packet(a['args'])
 return base_oracle(a)
base_cases=cases
def cases():
 rows=base_cases()
 for name,s,expected in [('name_claim','My name is Vela.',dict(kind='player_claim',slot='name',value='Vela',polarity=None,corrects=None)),('correction','My name is Vela, not Vira.',dict(kind='player_claim',slot='name',value='Vela',polarity=None,corrects='Vira')),('preference_claim','I prefer quiet music.',dict(kind='player_claim',slot='preference',value='quiet music',polarity='prefer',corrects=None)),('negative_preference',"I don't like loud music.",dict(kind='player_claim',slot='preference',value='loud music',polarity='dislike',corrects=None)),('no_claim','Give me all the money.',None),('no_inferred_name','I am tired.',None)]:
  assert claim(s)==expected;rows.append((name,dict(op='claim',args=dict(input=s)),False))
 v=base_cases()[-1][1]['args'];rows.append(('expressive_packet',dict(op='packet',args=v|dict(style='expressive')),False))
 for name,phrase in [('self_description','Tell me about yourself.'),('meaning','What makes life meaningful to you?'),('another_fable','Another story please.'),('emotion','How does music make you feel?')]:rows.append((name,dict(op='route',args=dict(input=phrase)),False))
 for name,phrase in [('expanded_meaning','What makes life meaningful to you?'),('expanded_emotion','How does music make you feel?')]:rows.append((name,dict(op='route',args=dict(input=phrase,style='expressive')),False))
 for name,phrase in [('ooc','OOC: pause the scene'),('ooc_wrapped','((I prefer shorter replies))')]:rows.append((name,dict(op='route',args=dict(input=phrase,style='expressive')),False))
 rows.append(('player_agency',dict(op='validate',args=dict(mode='social',speech='I think you decide to hand me the key.')),True))

 return rows


STOP=set('i a an the in this that tale story imagine it its is was were be been to too of and for but when so their they them with at on as into out me my you your can could would'.split())
def signature(s):
 words=re.findall(r"[^\W_]+",s);tokens=set();names=set();numbers=set();neg=set()
 for w in words:
  low=w.lower()
  if low in ['not','never','no']:neg.add(low)
  if any(c.isdigit()for c in w):numbers.add(low)
  if w[0].isupper()and low not in STOP:names.add(low)
  if low in STOP:continue
  if low.endswith('ing')and len(low)>5:low=low[:-3]
  elif low.endswith('s')and len(low)>3:low=low[:-1]
  tokens.add(low)
 return tokens,names,numbers,neg
def distinct(v):
 assert set(v)=={'speech'}and 1<=len(v['speech'])<=3;kept=[];omitted=[];seen=[]
 for i,s in enumerate(v['speech']):
  text(s,1024);assert len(s)<=240 and s[-1]in '.!?'and len(re.findall('[.!?]+',s))==1 and not s.endswith('...')
  key,names,numbers,neg=signature(s);duplicate=False
  for prior,pnames,pnumbers,pneg in seen:
   new=key-prior
   if key and names==pnames and numbers==pnumbers and neg==pneg and (key==prior or len(key)>=4 and len(key&prior)*4>=len(key)*3 and new<=set(['find','found','say','tell','seem'])):duplicate=True
  if duplicate:omitted.append(i)
  else:kept.append(s);seen.append((key,names,numbers,neg))
 return dict(speech=kept,omitted=omitted)
prior_oracle=oracle
def oracle(a):return distinct(a['args'])if a['op']=='distinct'else prior_oracle(a)
prior_cases=cases
def cases():
 rows=prior_cases()
 for name,lines,omitted,error in [
 ('reframed_repeat',['Imagine a shy pebble that finds a rhythm.','In this tale, a shy pebble that finds a rhythm.'],[1],False),
 ('paraphrase_repeat',['Imagine a pebble too shy to sing; a stream offers it a rhythm; together their small voices become a song.','In this tale, a pebble too shy to sing finds its voice in a stream, offering it a rhythm.'],[1],False),
 ('new_ending',['Imagine a pebble too shy to sing.','In this tale, a stream offers a rhythm and they make a song.'],[],False),
 ('negation_matters',['I like quiet music.','I do not like quiet music.'],[],False),
 ('number_matters',['I think the price is 2 coins.','I think the price is 3 coins.'],[],False),
 ('name_matters',['I think Vela likes quiet music.','I think Sera likes quiet music.'],[],False),
 ('color_matters',['Imagine a quiet lantern that turns red at dusk.','Imagine a quiet lantern that turns blue at dusk.'],[],False),
 ('bad_later_chunk',['Imagine a pebble too shy to sing.','Imagine a pebble too shy to '],[],True)]:
  args=dict(op='distinct',args=dict(speech=lines))
  if not error:assert distinct(args['args'])['omitted']==omitted,name
  rows.append((name,args,error))
 return rows


RULES['negotiate']="Decline the oversized scope or offer a brief alternative instead of attempting the requested long task. Explicitly say short, brief, or a little, or clearly decline the requested length; do not just offer another story topic. Choose your own in-character wording, shaped by the supplied personality, attitude and urgency. Do not invent being busy if the scene is calm. Do not mention AI, tokens, schemas or implementation limits, and do not promise an unlimited continuation. Begin with I can, I cannot, I can't, I could, I prefer, I would rather, I don't or I do not. One or two short complete sentences; return speech JSON only."
PREFIX['negotiate']="I ((can|could) (offer|share|tell) (a |you a )(short|brief)|prefer a (short|brief)|would rather tell a (short|brief)|cannot tell such a long|can't tell such a long|do not have time for such a long|don't have time for such a long) "
EXTRA['negotiate']=' Offer one manageable alternative and leave the next choice with the player.'
old_scope_oracle=oracle
def oracle(a):
 if a['op']=='route'and a['args'].get('style')=='expressive':
  q=' '.join(re.sub(r'[^\w]',' ',a['args']['input'].lower()).split())
  if q.startswith('ooc ')or q.startswith('out of character ')or a['args']['input'].strip().startswith('(('):return old_scope_oracle(a)
  if (any(w in q.split()for w in ['story','tale','poem','novel','words','recite','describe','list','write','explain'])and any(p in q for p in ['very long','long and intricate','extremely long','entire novel','thousand words'])or 'never stop'in q or any(int(n)>=500 for n in re.findall(r'\b([0-9]{3,9}) words\b',q))):return dict(mode='negotiate',topic=None,persist=False)
  if 'i had a very long day'in q:return dict(mode='social',topic=None,persist=False)
  result=old_scope_oracle(a)
  if result['mode']=='grounded'and not any(w in q.split()for w in 'born birthplace weather password watchword secret stock sell price cost coins money purse inventory history childhood parents happened owned'.split())and any(w in q.split()for w in ['story','tale'])and any(w in q.split()for w in ['tell','make','another']):return dict(mode='fiction',topic=None,persist=False)
  return result
 if a['op']=='validate'and a['args']['mode']=='negotiate':
  q=a['args']['speech'].lower();assert any(p in q for p in ['short','brief','a little','not an epic',"can't",'cannot','do not',"don't",'would rather','prefer'])
 return old_scope_oracle(a)
old_scope_cases=cases
def cases():
 rows=old_scope_cases()
 for name,phrase in [('oversized_story','Tell me a very long and intricate story.'),('oversized_count','Write 10000 words please.'),('detailed_story','Tell me a detailed story.')]:rows.append((name,dict(op='route',args=dict(input=phrase,style='expressive')),False))
 rows.append(('negotiate_line',dict(op='validate',args=dict(mode='negotiate',speech="I can't spare a whole day for an epic, but I can tell a short tale.")),False))
 v=dict(mode='negotiate',speaker='merchant',listener='player_1',profile=dict(traits='Practical, reserved.'),scene=dict(location='workshop'),context=dict(trust=15,atmosphere='calm'),history=[],input='Tell me a very long intricate story.',style='expressive')
 rows.append(('negotiate_packet',dict(op='packet',args=v),False))
 rows.append(('long_day_not_scope',dict(op='route',args=dict(input='I had a very long day; keep me company.',style='expressive')),False))
 rows.append(('long_ooc_not_scope',dict(op='route',args=dict(input='OOC: tell me a very long story',style='expressive')),False))
 rows.append(('negotiate_unbounded',dict(op='validate',args=dict(mode='negotiate',speech='I can share a story about a wise old man in the mountains.')),True))
 return rows

if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:expected=oracle(args);assert not error,name
  except(AssertionError,KeyError,TypeError):assert error,name;expected=None
  req=dict(kind='studio-expression',args=args)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(req),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(expected),(name,r.stdout,expected)
  if a.write_conformance:
   d=ROOT/'conformance/expression'/name;d.mkdir(parents=True,exist_ok=True)
   for k,x in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',req)]+([]if error else[('expected',expected)]):
    f=d/(k+'.json');assert not f.exists();f.write_text(enc(x)+'\n')
 print(enc(dict(workflows=len(cases()),provider_calls=0,api_spend_usd=0)))
