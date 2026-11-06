"""Independent small English dialogue policy; host facts remain authoritative."""
from dungeon_slots_reference import tokens,contains

def phrases(w,seqs):return any(contains(w,s.split()) for s in seqs)
def past(w):return phrases(w,['did i','did you']) or any(x in w for x in ['told','said','yesterday','previously','tomorrow'])
def identity(s):
 w=tokens(s);return not past(w) and phrases(w,['your name','you called','call you','who are you','introduce yourself'])
def biography(s):
 w=tokens(s);return not past(w) and any(x in w for x in ['story','background','job','work','yourself'])
def opinion(s):return phrases(tokens(s),['what do you think','how do you feel','do you like','would you like'])
def weather_report(s):
 w=tokens(s);return any(x in w for x in ['hot','cold','warm','sunny','rainy','raining','snowing']) and any(x in w for x in ['day','today','weather','outside'])
def location(s):return phrases(tokens(s),['where are we','where am i','what is this place','where is this','describe this place'])
def world(s):return phrases(tokens(s),['what do you know about this world','what can you tell me about this world','tell me about this world','what is this world'])
def overview(s):
 w=tokens(s);fill=set('hello hi hey please thanks now here on about topics anything else'.split())
 patterns=['what do you have information on','what do you know','what can you help with','what can you tell me','what information do you have','anything else','what else']
 for p in patterns:
  n=p.split()
  for i in range(len(w)-len(n)+1):
   if w[i:i+len(n)]==n and all(x in fill for x in w[:i]+w[i+len(n):]):return True
 return False
def stock_extra(s):return phrases(tokens(s),['do you have anything else','anything else for sale','any other stock'])
def meaningful(s):
 stop=set('a an the is are was were am do does did i my me you your we our it its this that what who when where why how about on in of to for with can could should would will have has had please hello hi hey know information anything else tell say'.split())
 return ' '.join(w for w in tokens(s) if w not in stop)
def unknown(speaker):return {'merchant':"I couldn't tell you about that. My trade is workshop supplies and repairs.",'keeper':"I couldn't tell you about that. I know the cistern and the old vault.",'narrator':"Nothing in the current scene tells us that."}[speaker]
def quote(s):
 t=s.strip();lower=t.lower()
 for prefix,replacement in [('my name is ','You told me your name is '),('my home is ','You told me your home is '),('i live in ','You told me you live in ')]:
  if lower.startswith(prefix):
   tail=t[len(prefix):].rstrip('.')
   if tail and len(tail)<=80 and all(c.isalnum() or c in " '-" for c in tail):return replacement+tail+'.'
 return 'I remember you saying: "'+s+'"'

def letter_request(s):
 import re
 m=re.fullmatch(r'\s*how many letters (?:are )?in\s+["“\x27]?([^"”\x27?\n]{1,80})["”\x27]?\s*\??\s*',s,re.IGNORECASE)
 if m:return m.group(1).strip()
 m=re.fullmatch(r'\s*([^\n]{1,80}?) has [0-9]+ letters[.!]?\s*',s,re.IGNORECASE)
 return m.group(1).strip() if m else None

def time_question(s):return phrases(tokens(s),['what time is it','what day is it','what is the time','what day are we on','current world time'])

def time_report(s):return s.strip().lower().startswith(('the time is ','my watch says ','the clock says ','it is noon','it is midnight')) and '?' not in s
