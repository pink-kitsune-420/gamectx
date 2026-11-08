"""Independent conversation state prototype; quoted memories never become world facts."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy
import json
import math
import re
import sys
import pathlib
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import dungeon_reference as game

def words(s):
    out=[]
    for w in re.findall(r'[^\W_]+(?:_[^\W_]+)*|_+',s.lower(),re.UNICODE):
        if all('0'<=c<='9' for c in w):continue
        for p in [w]+([w[:-1]] if len(w)>3 and w.endswith('s') else []):
            if p not in out:out.append(p)
    return out

def valid_text(s,limit):game.need(isinstance(s,str) and 0<len(s.strip()) and len(s)<=limit and not any(ord(c)<32 or 127<=ord(c)<=159 for c in s))
class Social:
    def __init__(self):self.sequence=0;self.pending=None;self.scopes={}
    def scope(self,g,speaker):
        game.need(speaker in ['keeper','merchant','narrator'] and bool(g.choices()))
        game.need(speaker=='narrator' or g.room==('cistern' if speaker=='keeper' else 'workshop'))
        return speaker+(':'+g.room if speaker=='narrator' else '')
    def state(self,g,speaker):
        scope=self.scope(g,speaker);v=self.scopes.get(scope,{'tone':'neutral','history':[],'memories':[]})
        return {'scope':scope,'tone':v['tone'],'history':v['history'],'memories':v['memories']}
    def facts(self,g,speaker):
        scene=g.public();scene.pop('choices')
        out={'scene':scene,'goal':game_goal()}
        if speaker!='narrator':out.update(gate_requirements='A bronze key and hinge oil unlock the vault gate.',key_location='cistern' if g.seed%2==0 else 'workshop')
        if speaker=='merchant':out['trade_rules']='Prices are fixed. Only a validated purchase transfers an item.'
        if speaker=='keeper':out['ownership']='The keeper seal belongs to the keeper; it is not a gift.'
        return out
    def prepare(self,g,speaker,input):
        scope=self.scope(g,speaker);input=input.strip();valid_text(input,240);game.need(self.sequence<256)
        v=self.state(g,speaker);wanted=words(input);parts=[words(m['text']) for m in v['memories']]
        weights=[1/(sum(w in p for p in parts) or 1) for w in wanted]
        order=sorted(range(len(parts)),key=lambda i:(-sum(x for w,x in zip(wanted,weights) if w in parts[i]),-v['memories'][i]['turn'],-v['memories'][i]['id']))[:2]
        recalled=sorted([v['memories'][i] for i in order],key=lambda m:m['id'])
        self.sequence+=1
        self.pending={'id':'chat_'+str(self.sequence),'revision':g.revision,'scope':scope,'speaker':speaker,'input':input,
                      'facts':self.facts(g,speaker),'memories':copy.deepcopy(recalled),'history':copy.deepcopy(v['history'][-2:]),'tone':v['tone']}
        return copy.deepcopy(self.pending)
    def finish(self,g,ticket,text):
        p=self.pending;game.need(p is not None and ticket==p['id'] and p['revision']==g.revision and self.scope(g,p['speaker'])==p['scope'])
        game.need(isinstance(text,str) and len(text.encode())<=4096)
        def unique(items):
            out={}
            for k,v in items:game.need(k not in out);out[k]=v
            return out
        reply=json.loads(text,object_pairs_hook=unique);game.fields(reply,['speech','tone','fact_ids','memory_ids'])
        reply['speech']=reply['speech'].strip();valid_text(reply['speech'],240);game.need(reply['tone'] in ['neutral','warm','wary'])
        game.need(isinstance(reply['fact_ids'],list) and len(reply['fact_ids'])<=8 and len(set(reply['fact_ids']))==len(reply['fact_ids']) and all(k in p['facts'] for k in reply['fact_ids']))
        game.need(isinstance(reply['memory_ids'],list) and len(reply['memory_ids'])<=2 and all(type(i) is int for i in reply['memory_ids']) and len(set(reply['memory_ids']))==len(reply['memory_ids']) and all(any(m['id']==i for m in p['memories']) for i in reply['memory_ids']))
        v=copy.deepcopy(self.scopes.get(p['scope'],{'tone':'neutral','history':[],'memories':[]}))
        v['tone']=reply['tone'];v['history']=(v['history']+[{'id':p['id'],'turn':self.sequence,'user':p['input'],'assistant':reply['speech']}])[-8:]
        next_id=(v['memories'][-1]['id']+1) if v['memories'] else 1
        v['memories']=(v['memories']+[{'id':next_id,'turn':self.sequence,'text':p['input']}])[-32:]
        self.scopes[p['scope']]=v;self.pending=None
        return {'speaker':p['speaker'],'speech':reply['speech'],'tone':reply['tone'],'fact_ids':reply['fact_ids'],'memory_ids':reply['memory_ids']}
    def forget(self,g,speaker):
        scope=self.scope(g,speaker);self.scopes.pop(scope,None)
        if self.pending and self.pending['scope']==scope:self.pending=None
        return {'forgotten':scope}

def game_goal():return 'Recover the glass star from the vault and leave through the daylight stair.'

def replay(v):
    game.fields(v,['seed','operations']);g=game.Game(v['seed']);s=Social();out=[]
    game.need(isinstance(v['operations'],list) and len(v['operations'])<=128)
    for op in v['operations']:
        game.fields(op,['op','args']);a=op['args'];before=copy.deepcopy(s.__dict__)
        try:
            if op['op']=='prepare':game.fields(a,['speaker','input']);r=s.prepare(g,**a)
            elif op['op']=='finish':game.fields(a,['ticket','text']);r=s.finish(g,**a)
            elif op['op']=='state':game.fields(a,['speaker']);r=s.state(g,**a)
            elif op['op']=='forget':game.fields(a,['speaker']);r=s.forget(g,**a)
            elif op['op']=='act':game.fields(a,['decision']);r=g.act(g.revision,a['decision'])
            else:raise ValueError()
            out.append({'ok':True,'result':copy.deepcopy(r)})
        except (ValueError,TypeError,KeyError):
            game.need(before==s.__dict__);out.append({'ok':False})
    return {'results':out,'world':g.public()}
