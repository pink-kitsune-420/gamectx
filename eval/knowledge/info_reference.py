"""Model-callable informational notes: reported claims never become host facts."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from conversation_reference import fields,need,uint,scope,strict
from dungeon_reference import encode

def sources_check(sources):
    need(isinstance(sources,list) and len(sources)<=16);ids=[]
    for s in sources:
        fields(s,['id','speaker','text']);scope(s['id']);scope(s['speaker']);need(s['id'] not in ids);ids.append(s['id'])
        need(isinstance(s['text'],str) and 0<len(s['text'])<=512 and not any(ord(c)<32 or 127<=ord(c)<=159 for c in s['text']))
class Notes:
    def __init__(self,key):scope(key);self.key=key;self.notes=[];self.sequence=0
    def call(self,name,text,sources):
        need(len(text.encode())<=4096);a=strict(text);sources_check(sources)
        if name=='info_lookup':
            fields(a,['subject','attribute']);scope(a['subject']);scope(a['attribute'])
            return {'scope':self.key,'status':'reported','notes':[n for n in self.notes if n['subject']==a['subject'] and n['attribute']==a['attribute']][-16:]}
        if name=='info_remember':
            fields(a,['subject','attribute','value','source']);scope(a['subject']);scope(a['attribute']);scope(a['source']);need(len(encode(a['value']).encode())<=1024)
            source=next((s for s in sources if s['id']==a['source']),None);need(source is not None and len(self.notes)<64 and self.sequence<2**32-1)
            n={'id':self.sequence+1,'subject':a['subject'],'attribute':a['attribute'],'value':copy.deepcopy(a['value']),'status':'reported','source':copy.deepcopy(source)}
            self.sequence+=1;self.notes.append(n);return copy.deepcopy(n)
        if name=='info_forget':
            fields(a,['id']);uint(a['id'],2**32-1);need(any(n['id']==a['id'] for n in self.notes));self.notes=[n for n in self.notes if n['id']!=a['id']];return {'forgotten':a['id']}
        raise ValueError('unknown tool')
def process(a):
    fields(a,['scope','calls']);need(isinstance(a['calls'],list) and len(a['calls'])<=128);n=Notes(a['scope']);out=[]
    for c in a['calls']:
        try:fields(c,['name','text','sources']);out.append({'ok':True,'result':n.call(c['name'],c['text'],c['sources'])})
        except (ValueError,TypeError,KeyError):out.append({'ok':False})
    return {'scope':n.key,'notes':n.notes,'results':out}
