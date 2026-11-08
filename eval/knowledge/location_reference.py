"""Read-only host-bound location snapshots and deterministic quote phrasing."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from conversation_reference import fields,need,uint,scope,strict
from dungeon_reference import encode
from conversation_policy_reference import quote

def body(s,n):need(isinstance(s,str) and 0<len(s)<=n and not any(ord(c)<32 or 127<=ord(c)<=159 for c in s))
class Locations:
 def __init__(self,v):
  fields(v,['version','revision','places']);need(type(v['version']) is int and v['version']==1 and len(encode(v).encode())<=1048576);uint(v['revision'],2**32-1);need(isinstance(v['places'],list) and len(v['places'])<=128);ids=[]
  for p in v['places']:
   fields(p,['id','label','summary','description','details']);scope(p['id']);scope(p['label']);(body(p['summary'],512) if p['summary'] is not None else None);(body(p['description'],2048) if p['description'] is not None else None);need(isinstance(p['details'],dict) and len(p['details'])<=32 and len(encode(p['details']).encode())<=16384 and p['id'] not in ids);ids.append(p['id'])
  self.v=copy.deepcopy(v)
 def call(self,text,current,revision,cap,available):
  scope(current);uint(revision,2**32-1);need(revision==self.v['revision'] and 64<=cap<=32768 and len(text.encode())<=4096);a=strict(text);fields(a,['detail']);need(a['detail'] in ['brief','normal','full'])
  p=next((p for p in self.v['places'] if p['id']==current and p['id'] in available),None)
  keys=['id','label','summary']+(['description'] if a['detail']!='brief' else [])+(['details'] if a['detail']=='full' else [])
  r={'revision':revision,'location':{k:copy.deepcopy(p[k]) for k in keys} if p else None};need(len(encode(r).encode())<=cap);return r

def process(a):
 fields(a,['definition','text','current','revision','max_bytes','available']);need(isinstance(a['available'],list) and len(a['available'])<=128)
 for id in a['available']:scope(id)
 return Locations(a['definition']).call(a['text'],a['current'],a['revision'],a['max_bytes'],a['available'])


def phrase(a):
 fields(a,['quote']);body(a['quote'],1024);need(len(a['quote'].encode())<=4096 and a['quote'].strip())
 return quote(a['quote'])
