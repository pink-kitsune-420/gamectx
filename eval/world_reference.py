"""Provider-independent, read-only world states; clocks/calendars are host data."""
import copy,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from conversation_reference import fields,need,uint,scope,strict
from lore_reference import strings
from dungeon_reference import encode
class World:
 def __init__(self,v):
  fields(v,['version','id','revision','states']);need(type(v['version']) is int and v['version']==1 and len(encode(v).encode())<=524288);scope(v['id']);uint(v['revision'],2**32-1);need(isinstance(v['states'],list) and len(v['states'])<=64);keys=[]
  for s in v['states']:
   fields(s,['key','value']);scope(s['key']);need(s['key'] not in keys and len(encode(s['value']).encode())<=4096);keys.append(s['key'])
  self.v=copy.deepcopy(v)
 def call(self,text,revision,cap,allowed):
  need(len(text.encode())<=4096 and revision==self.v['revision'] and type(revision) is int and 64<=cap<=32768);a=strict(text);fields(a,['keys']);strings(a['keys'],8);need(a['keys'])
  values={s['key']:s['value'] for s in self.v['states']}
  out={'id':self.v['id'],'revision':revision,'states':{k:copy.deepcopy(values[k]) for k in a['keys'] if k in allowed and k in values}};need(len(encode(out).encode())<=cap);return out

def process(a):
 fields(a,['definition','text','revision','max_bytes','available']);strings(a['available'],64)
 return World(a['definition']).call(a['text'],a['revision'],a['max_bytes'],a['available'])
