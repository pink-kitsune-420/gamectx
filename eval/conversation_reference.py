"""Generic dialogue and explicit confirmation prototypes; no model or game dependencies."""
import copy
import json
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_reference import words,valid_text
from dungeon_reference import fields,need,uint,encode
DEFAULT={'scopes':32,'records':32,'history':8,'text_chars':240,'tickets':65536}

def scope(s):need(isinstance(s,str) and 0<len(s)<=64 and len(s.encode())<=256 and not any(ord(c)<32 or 127<=ord(c)<=159 for c in s))
def strict(s):
    def unique(items):
        out={}
        for k,v in items:need(k not in out);out[k]=v
        return out
    return json.loads(s,object_pairs_hook=unique,parse_constant=lambda _:(_ for _ in ()).throw(ValueError()))
def same(a,b):
    if type(a) is not type(b):return False
    if isinstance(a,dict):return a.keys()==b.keys() and all(same(a[k],b[k]) for k in a)
    if isinstance(a,list):return len(a)==len(b) and all(same(x,y) for x,y in zip(a,b))
    return a==b

class Conversation:
    def __init__(self,limits):
        fields(limits,list(DEFAULT));self.limits=copy.deepcopy(limits)
        for key,maximum in [('scopes',64),('records',256),('history',128),('text_chars',1024),('tickets',1000000)]:need(type(limits[key]) is int and 1<=limits[key]<=maximum)
        self.sequence=0;self.pending=None;self.scopes={}
    def state(self,key):
        scope(key);v=self.scopes.get(key,{'tone':'neutral','history':[],'memories':[]})
        return copy.deepcopy({'scope':key,'tone':v['tone'],'history':v['history'],'memories':v['memories']})
    def prepare(self,a):
        fields(a,['scope','speaker','revision','input','facts','recall','history']);key=a['scope'];scope(key);scope(a['speaker']);uint(a['revision'],2**32-1)
        need(isinstance(a['input'],str));text=a['input'].strip();valid_text(text,self.limits['text_chars'])
        need(isinstance(a['facts'],dict) and len(a['facts'])<=32 and len(encode(a['facts']).encode())<=32768)
        for k in a['facts']:scope(k)
        uint(a['recall'],min(32,self.limits['records']));uint(a['history'],self.limits['history'])
        need(self.sequence<self.limits['tickets'] and (key in self.scopes or len(self.scopes)<self.limits['scopes']))
        v=self.state(key);wanted=words(text);parts=[words(m['text']) for m in v['memories']]
        weights=[1/(sum(w in p for p in parts) or 1) for w in wanted]
        order=sorted(range(len(parts)),key=lambda i:(-sum(x for w,x in zip(wanted,weights) if w in parts[i]),-v['memories'][i]['turn'],-v['memories'][i]['id']))[:a['recall']]
        recalled=sorted([v['memories'][i] for i in order],key=lambda m:m['id'])
        self.sequence+=1
        self.pending={'id':'chat_'+str(self.sequence),'revision':a['revision'],'scope':key,'speaker':a['speaker'],'input':text,'facts':copy.deepcopy(a['facts']),
                      'memories':copy.deepcopy(recalled),'history':copy.deepcopy(v['history'][-a['history']:] if a['history'] else []),'tone':v['tone']}
        return copy.deepcopy(self.pending)
    def finish(self,a):
        fields(a,['scope','revision','ticket','text']);p=self.pending
        need(p is not None and a['scope']==p['scope'] and a['revision']==p['revision'] and type(a['revision']) is int and a['ticket']==p['id'])
        need(isinstance(a['text'],str) and len(a['text'].encode())<=16384);r=strict(a['text']);fields(r,['speech','tone','fact_ids','memory_ids'])
        r['speech']=r['speech'].strip();valid_text(r['speech'],self.limits['text_chars']);need(r['tone'] in ['neutral','warm','wary'])
        need(isinstance(r['fact_ids'],list) and len(r['fact_ids'])<=8 and all(isinstance(k,str) and k in p['facts'] for k in r['fact_ids']) and len(set(r['fact_ids']))==len(r['fact_ids']))
        need(isinstance(r['memory_ids'],list) and len(r['memory_ids'])<=2 and all(type(i) is int and any(m['id']==i for m in p['memories']) for i in r['memory_ids']) and len(set(r['memory_ids']))==len(r['memory_ids']))
        v=copy.deepcopy(self.scopes.get(p['scope'],{'tone':'neutral','history':[],'memories':[],'next_id':1}))
        v['tone']=r['tone'];v['history']=(v['history']+[{'id':p['id'],'turn':self.sequence,'user':p['input'],'assistant':r['speech']}])[-self.limits['history']:]
        id=v['next_id'];v['next_id']+=1;v['memories']=(v['memories']+[{'id':id,'turn':self.sequence,'text':p['input']}])[-self.limits['records']:]
        self.scopes[p['scope']]=v;self.pending=None
        return {'speaker':p['speaker'],'speech':r['speech'],'tone':r['tone'],'fact_ids':r['fact_ids'],'memory_ids':r['memory_ids']}
    def forget(self,key):
        scope(key);self.scopes.pop(key,None)
        if self.pending and self.pending['scope']==key:self.pending=None
        return {'forgotten':key}

def conversation(a):
    fields(a,['limits','operations']);s=Conversation(a['limits']);need(isinstance(a['operations'],list) and len(a['operations'])<=256);out=[]
    for op in a['operations']:
        fields(op,['op','args']);before=copy.deepcopy(s.__dict__)
        try:
            if op['op']=='schema':
                fields(op['args'],[]);need(s.pending is not None)
                facts=list(s.pending['facts']);ids=[m['id'] for m in s.pending['memories']]
                item={'type':'integer',**({'enum':ids} if ids else {})}
                r={'type':'object','properties':{'speech':{'type':'string','maxLength':s.limits['text_chars']},'tone':{'type':'string','enum':['neutral','warm','wary']},
                    'fact_ids':{'type':'array','items':{'type':'string',**({'enum':facts} if facts else {})},'maxItems':8 if facts else 0},'memory_ids':{'type':'array','items':item,'maxItems':min(len(ids),2)}},
                    'required':['speech','tone','fact_ids','memory_ids'],'additionalProperties':False}
            elif op['op']=='prepare':r=s.prepare(op['args'])
            elif op['op']=='finish':r=s.finish(op['args'])
            elif op['op'] in ['state','forget']:
                fields(op['args'],['scope']);r=getattr(s,op['op'])(op['args']['scope'])
            else:raise ValueError()
            out.append({'ok':True,'result':r})
        except (ValueError,KeyError,TypeError):need(before==s.__dict__);out.append({'ok':False})
    return {'results':out}

def confirmation(a):
    fields(a,['offer','calls']);o=a['offer'];fields(o,['id','scope','revision','decision','terms']);scope(o['id']);scope(o['scope']);uint(o['revision'],2**32-1)
    fields(o['decision'],['action','target']);action=o['decision']['action'];need(isinstance(action,str) and len(action)<=32 and action[0:1].islower() and action.isascii() and all(c.islower() or c.isdigit() or c=='_' for c in action))
    if o['decision']['target'] is not None:scope(o['decision']['target'])
    need(isinstance(o['terms'],dict) and len(encode(o).encode())<=8192 and isinstance(a['calls'],list) and len(a['calls'])<=8)
    finished=False;out=[]
    for call in a['calls']:
        fields(call,['scope','revision','confirm','terms','available'])
        try:
            scope(call['scope']);uint(call['revision'],2**32-1);need(type(call['confirm']) is bool and isinstance(call['available'],list) and len(call['available'])<=32)
            need(not finished);finished=True
            if not call['confirm']:r={'status':'cancelled'}
            else:
                need(call['scope']==o['scope'] and call['revision']==o['revision'] and same(call['terms'],o['terms']) and o['decision'] in call['available'])
                r={'status':'confirmed','decision':o['decision'],'terms':o['terms']}
            out.append({'ok':True,'result':r})
        except (ValueError,KeyError,TypeError):out.append({'ok':False})
    return {'results':out,'finished':finished}
