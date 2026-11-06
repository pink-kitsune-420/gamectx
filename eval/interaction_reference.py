"""Optional conversation tools over host evidence, references and confirmations."""
import copy,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from conversation_reference import fields,need,uint,scope,strict,confirmation
from references_reference import resolve
from info_reference import Notes,sources_check
from lore_reference import strings
from dungeon_reference import encode
MAX=2**32-1

def body(s):need(isinstance(s,str) and 0<len(s)<=512 and not any(ord(c)<32 or 127<=ord(c)<=159 for c in s))
def fit(rows,limit,cap):
    chosen=[]
    for row in reversed(rows):
        if len(chosen)==limit:break
        trial=[row]+chosen
        if len(encode({'records':trial}).encode())<=cap:chosen=trial
    return {'records':copy.deepcopy(chosen)}
class Interactions:
    def __init__(self,key,actor,participants):
        scope(key);scope(actor);strings(participants,16);need(actor in participants)
        self.key=key;self.actor=actor;self.participants=participants[:];self.events=[];self.notes=Notes(key);self.progress=[];self.offers=[];self.pending=None;self.used=[];self.references=None
    def event(self,e):
        fields(e,['id','turn','subject','kind','value','audience'])
        for k in ['id','subject','kind']:scope(e[k])
        uint(e['turn'],MAX);strings(e['audience'],16);need(e['audience'] and all(p in self.participants for p in e['audience']) and len(encode(e['value']).encode())<=1024 and len(self.events)<128)
        need(not any(x['id']==e['id'] for x in self.events) and (not self.events or self.events[-1]['turn']<=e['turn']))
        self.events.append(copy.deepcopy(e));return {'published':e['id']}
    def set_progress(self,a):
        fields(a,['id','expected','state','source']);uint(a['id'],MAX);uint(a['expected'],MAX);sources_check([a['source']]);need(a['state'] in ['accepted','fulfilled','cancelled'])
        note=next((n for n in self.notes.notes if n['id']==a['id'] and n['attribute']=='promise'),None);need(note is not None)
        old=next((p for p in self.progress if p['id']==a['id']),None);revision=old['revision'] if old else 0;state=old['state'] if old else 'reported'
        need(a['expected']==revision and revision<MAX and ((state=='reported' and a['state'] in ['accepted','cancelled']) or (state=='accepted' and a['state'] in ['fulfilled','cancelled'])))
        p={'id':a['id'],'state':a['state'],'revision':revision+1,'source':copy.deepcopy(a['source'])}
        if old:self.progress[self.progress.index(old)]=p
        else:self.progress.append(p)
        return copy.deepcopy(p)
    def set_offers(self,rows):
        need(isinstance(rows,list) and len(rows)<=16);ids=[]
        for row in rows:
            fields(row,['recipient','offer']);scope(row['recipient']);need(row['recipient'] in self.participants and row['recipient']!=self.actor)
            o=row['offer'];confirmation({'offer':o,'calls':[]});fields(o,['id','scope','revision','decision','terms']);scope(o['id']);need(o['scope']==self.key and o['id'] not in ids);ids.append(o['id']);uint(o['revision'],MAX)
            fields(o['decision'],['action','target']);scope(o['decision']['action']);need(o['decision']['target'] is None or isinstance(o['decision']['target'],str));need(isinstance(o['terms'],dict) and len(encode(o).encode())<=8192)
        self.offers=copy.deepcopy(rows);return {'offers':len(rows)}
    def call(self,name,text,sources,revision,cap,allowed):
        uint(revision,MAX);need(64<=cap<=8192 and len(text.encode())<=4096);a=strict(text)
        if name=='events_recent':
            fields(a,['since','subject','limit']);uint(a['since'],MAX);uint(a['limit'],8);need(a['limit']>0 and a['since']<=revision)
            if a['subject'] is not None:scope(a['subject'])
            rows=[{k:e[k] for k in ['id','turn','subject','kind','value']} for e in self.events if self.actor in e['audience'] and a['since']<=e['turn']<=revision and (a['subject'] is None or a['subject']==e['subject'])]
            result=fit(rows,a['limit'],cap)
        elif name=='promise_remember':
            fields(a,['promisor','beneficiary','task','source']);need(a['promisor'] in self.participants and a['beneficiary'] in self.participants and a['promisor']!=a['beneficiary']);body(a['task'])
            result=self.notes.call('info_remember',encode({'subject':a['promisor'],'attribute':'promise','value':{'beneficiary':a['beneficiary'],'task':a['task']},'source':a['source']}),sources)
        elif name=='promise_lookup':
            fields(a,['participant','limit']);uint(a['limit'],8);need(a['limit']>0 and (a['participant'] is None or a['participant'] in self.participants));rows=[]
            for n in self.notes.notes:
                if n['attribute']=='promise' and (a['participant'] is None or a['participant'] in [n['subject'],n['value']['beneficiary']]):
                    progress=next((p for p in self.progress if p['id']==n['id']),{'id':n['id'],'state':'reported','revision':0,'source':None});rows.append({**n,'progress':progress})
            result=fit(rows,a['limit'],cap)
        elif name=='relationship_note':
            fields(a,['other','note','source']);need(a['other'] in self.participants and a['other']!=self.actor);body(a['note'])
            result=self.notes.call('info_remember',encode({'subject':self.actor,'attribute':'relationship','value':{'other':a['other'],'note':a['note']},'source':a['source']}),sources)
        elif name=='relationship_lookup':
            fields(a,['other','limit']);uint(a['limit'],8);need(a['limit']>0 and a['other'] in self.participants)
            result=fit([n for n in self.notes.notes if n['attribute']=='relationship' and n['value']['other']==a['other']],a['limit'],cap)
        elif name=='dialogue_resolve':
            fields(a,['kind']);need(self.references is not None and self.references['scope']==self.key and self.references['revision']==revision)
            result=resolve({**self.references,'kind':a['kind']})
        elif name=='interaction_offers':
            fields(a,['limit']);uint(a['limit'],8);need(a['limit']>0)
            result=fit([o for o in self.offers if o['offer']['revision']==revision and o['offer']['id'] not in self.used and o['offer']['decision'] in allowed],a['limit'],cap)
        elif name=='interaction_offer':
            fields(a,['id']);scope(a['id']);need((self.pending is None or self.pending['finished']) and a['id'] not in self.used and len(self.used)<256)
            row=next((o for o in self.offers if o['offer']['id']==a['id']),None);need(row is not None and row['offer']['revision']==revision and row['offer']['decision'] in allowed)
            self.pending={**copy.deepcopy(row),'finished':False};self.used.append(a['id']);result={'status':'proposed',**copy.deepcopy(row)}
        else:raise ValueError('unknown tool')
        need(len(encode(result).encode())<=cap);return result
    def respond(self,a):
        fields(a,['recipient','revision','confirm','terms','allowed']);need(self.pending is not None and not self.pending['finished'] and self.pending['recipient']==a['recipient']);p=self.pending
        # Existing confirmation consumes one attempt, even if stale or revoked.
        current=any(o['recipient']==p['recipient'] and o['offer']==p['offer'] for o in self.offers)
        out=confirmation({'offer':p['offer'],'calls':[{'scope':self.key,'revision':a['revision'],'confirm':a['confirm'],'terms':a['terms'],'available':a['allowed'] if current else []}]})
        p['finished']=out['finished'];need(out['results'][0]['ok']);return out['results'][0]['result']
    def state(self):return {'scope':self.key,'actor':self.actor,'participants':self.participants,'events':self.events,'notes':self.notes.notes,'progress':self.progress,'used':self.used,'pending':self.pending}
def process(a):
    fields(a,['scope','actor','participants','operations']);need(len(encode(a).encode())<=1048576);need(isinstance(a['operations'],list) and len(a['operations'])<=128);s=Interactions(a['scope'],a['actor'],a['participants']);out=[]
    for op in a['operations']:
        fields(op,['op','args']);v=op['args'];before=copy.deepcopy(s)
        try:
            if op['op']=='clear':
                fields(v,[]);s.events=[];s.notes.notes=[];s.progress=[];s.offers=[];s.pending=None;s.references=None;r={'cleared':True}
            elif op['op']=='event':r=s.event(v)
            elif op['op']=='progress':r=s.set_progress(v)
            elif op['op']=='offers':r=s.set_offers(v)
            elif op['op']=='references':resolve(v);need(v['scope']==s.key);s.references=copy.deepcopy(v);r={'ready':True}
            elif op['op']=='respond':r=s.respond(v)
            elif op['op']=='call':
                fields(v,['name','text','sources','revision','max_bytes','allowed']);r=s.call(v['name'],v['text'],v['sources'],v['revision'],v['max_bytes'],v['allowed'])
            else:raise ValueError('unknown operation')
            out.append({'ok':True,'result':r})
        except (ValueError,TypeError,KeyError):
            if op['op']!='respond':s=before
            out.append({'ok':False})
    return {'state':s.state(),'results':out}
