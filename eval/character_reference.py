"""Provider-neutral task/context selection prototype; byte counter is fixture-only."""
import copy,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from conversation_reference import fields,need,uint,scope
from dungeon_reference import encode

def choose(rows,cap):
 required=[(i,r) for i,r in enumerate(rows) if r['required']];need(len(required)<=cap)
 optional=sorted(((i,r) for i,r in enumerate(rows) if not r['required']),key=lambda x:(-x[1]['priority'],x[0]))[:cap-len(required)]
 return sorted(required+optional)
def plan(a):
 fields(a,['task','input','facts','tools','memories','dialogue','attention','budget']);scope(a['task']);need(isinstance(a['input'],str) and 0<len(a['input'].encode())<=4096)
 limits=a['attention'];fields(limits,['tools','memories','exchanges'])
 for k,m in [('tools',32),('memories',64),('exchanges',16)]:uint(limits[k],m)
 sections=[{'name':'base','value':{'task':a['task'],'input':a['input'],'facts':a['facts']},'priority':1000000,'required':True}]
 tools=[]
 for t in a['tools']:
  fields(t,['name','task','schema','priority','required']);scope(t['name']);scope(t['task']);need(type(t['required']) is bool and type(t['priority']) is int and -1000000<=t['priority']<=1000000)
  if t['task']!=a['task']:need(not t['required']);continue
  need(not any(x['name']==t['name'] for x in tools));tools.append(t)
 for i,r in choose(tools,limits['tools']):sections.append({'name':'tool_'+str(i),'value':r['schema'],'priority':r['priority'],'required':r['required']})
 for r in a['memories']:
  fields(r,['value','priority','required']);need(type(r['required']) is bool and type(r['priority']) is int and -1000000<=r['priority']<=1000000)
 for i,r in choose(a['memories'],limits['memories']):sections.append({'name':'memory_'+str(i),'value':r['value'],'priority':r['priority'],'required':r['required']})
 n=min(len(a['dialogue']),limits['exchanges'])
 for i,e in enumerate(a['dialogue']):
  fields(e,['user','assistant']);need(isinstance(e['user'],str) and isinstance(e['assistant'],str) and len(e['user'].encode())<=4096 and len(e['assistant'].encode())<=4096)
  if i>=len(a['dialogue'])-n:sections.append({'name':'exchange_'+str(i),'value':e,'priority':100+i,'required':i==len(a['dialogue'])-1})
 need(len(sections)<=128)
 budget=a['budget'];fields(budget,['context','output','safety']);uint(budget['context'],1048576);uint(budget['output'],1048576);uint(budget['safety'],1048576);need(budget['context']>0 and budget['output']+budget['safety']<=budget['context'])
 omitted=[]
 while True:
  base=sections[0]['value'];frame={**base,'tools':[s['value'] for s in sections if s['name'].startswith('tool_')],'memories':[s['value'] for s in sections if s['name'].startswith('memory_')],'dialogue':[s['value'] for s in sections if s['name'].startswith('exchange_')]}
  size=len(encode(frame).encode())
  if size+budget['output']+budget['safety']<=budget['context']:break
  optional=[(s['priority'],-i,i) for i,s in enumerate(sections) if not s['required']];need(optional);i=min(optional)[2];omitted.append(sections[i]['name']);sections.pop(i)
 return {'request':frame,'selected_tools':[t['name'] for t in tools if any(s['name'].startswith('tool_') and s['value']==t['schema'] for s in sections)],'input_bytes':size,'omitted':omitted}

def schema(name):
 string={'type':'string','minLength':1,'maxLength':64}
 if name=='world_status':
  description='Read selected current world states such as time, weather, lighting or event flags. Values come from the game; null is unknown. This advances no clock and changes no state.';props={'keys':{'type':'array','minItems':1,'maxItems':8,'items':string}};required=['keys']
 elif name in ['location_lookup','look_around']:
  description='Look up the host-bound current location, its summary, description and visible scene details. This moves nobody and invents no facts.' if name=='location_lookup' else 'Read the latest host-observed surroundings of this character. Only visible objects, people and observable weather may be included; this grants no remote vision.';props={'detail':{'type':'string','enum':['brief','normal','full']}};required=['detail']
 elif name=='info_lookup':description='Look up reported information about a subject and attribute. Claims are unverified.';props={'subject':string,'attribute':string};required=['subject','attribute']
 else:raise ValueError('prototype schema not listed')
 return {'type':'function','function':{'name':name,'description':description,'parameters':{'type':'object','properties':props,'required':required,'additionalProperties':False}}}
