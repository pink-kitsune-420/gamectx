"""Independent bounded lore selection and source-rendered answer prototypes."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy,json,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from conversation_reference import fields,need,uint,scope,strict
from dungeon_reference import encode
from dungeon_slots_reference import tokens

def strings(v,maximum):
    need(isinstance(v,list) and len(v)<=maximum and len(set(v))==len(v))
    for s in v:scope(s)
    return v

def body(v,maximum):need(isinstance(v,str) and 0<len(v)<=maximum and not any(ord(c)<32 or 127<=ord(c)<=159 for c in v))
SPEC=[('entries',512,65536),('bytes',262144,268435456)]
class Lore:
    def __init__(self,v,lim=None):
        lim=lim or {'entries':512,'bytes':262144}
        fields(v,['version','entries']);need(type(v['version']) is int and v['version'] in [1,2] and len(encode(v).encode())<=lim['bytes'])
        need(isinstance(v['entries'],list) and len(v['entries'])<=lim['entries']);ids=set()
        for e in v['entries']:
            payload='text' if v['version']==1 else 'value'
            fields(e,['id','subject','topic',payload,'tags','known_by','requires'])
            for k in ['id','subject','topic']:scope(e[k])
            need(e['id'] not in ids);ids.add(e['id'])
            if v['version']==1:body(e['text'],1024)
            else:need(len(encode(e['value']).encode())<=4096)
            strings(e['tags'],16);strings(e['known_by'],16);strings(e['requires'],16)
            need(e['known_by'] and ('*' not in e['known_by'] or e['known_by']==['*']))
        self.value=copy.deepcopy(v)
    def select(self,a):
        fields(a,['actor','grants','query','subjects','required','limit','max_bytes']);scope(a['actor']);strings(a['grants'],64);strings(a['subjects'],32);strings(a['required'],32)
        need(isinstance(a['query'],str) and len(a['query'].encode())<=2048);uint(a['limit'],32);uint(a['max_bytes'],32768);need(a['max_bytes']>=2)
        visible=[e for e in self.value['entries'] if ('*' in e['known_by'] or a['actor'] in e['known_by']) and all(g in a['grants'] for g in e['requires']) and (not a['subjects'] or e['subject'] in a['subjects'])]
        need(all(any(e['id']==id for e in visible) for id in a['required']))
        selected=[e for e in visible if e['id'] in a['required']];wanted=set(tokens(a['query']))
        def packed(rows):return [{k:e[k] for k in ['id','subject','topic',('text' if self.value['version']==1 else 'value')]} for e in rows]
        def fits(rows):return len(rows)<=a['limit'] and len(encode(packed(rows)).encode())<=a['max_bytes']
        need(fits(selected))
        scored=[(len(wanted.intersection(tokens(' '.join([e['id'],e['subject'],e['topic'],(e['text'] if self.value['version']==1 else encode(e['value'])),*e['tags']])))),i,e) for i,e in enumerate(visible) if e['id'] not in a['required']]
        for score,i,e in sorted(scored,key=lambda x:(-x[0],x[1])):
            if score and fits(selected+[e]):selected.append(e)
        packet=packed(selected);chosen=[e['id'] for e in selected]
        return {'packet':packet,'receipt':{'bytes':len(encode(packet).encode()),'selected':chosen,'omitted':[e['id'] for e in visible if e['id'] not in chosen]}}
class Answers:
    def __init__(self,v):
        fields(v,['version','scope','revision','unknown','cards']);need(v['version']==1 and type(v['version']) is int and len(encode(v).encode())<=65536)
        scope(v['scope']);uint(v['revision'],2**32-1);body(v['unknown'],2048);need(isinstance(v['cards'],list) and len(v['cards'])<=32);ids=[]
        for c in v['cards']:
            fields(c,['id','subject','topic','kind','source','text'])
            for k in ['id','subject','topic','source']:scope(c[k])
            need(c['id']!='unknown' and c['id'] not in ids and c['kind'] in ['lore','state','quote']);ids.append(c['id']);body(c['text'],1024)
        self.value=copy.deepcopy(v)
        self.value['cards']=[c for c in v['cards'] if c['kind']!='lore' or not any(s['kind']=='state' and s['subject']==c['subject'] and s['topic']==c['topic'] for s in v['cards'])]
    def offer(self):return [{k:c[k] for k in ['id','subject','topic','kind','text']} for c in self.value['cards']]
    def schema(self):return {'type':'object','properties':{'answer':{'type':'string','enum':[c['id'] for c in self.value['cards']]+['unknown']}},'required':['answer'],'additionalProperties':False}
    def resolve(self,text,context,available):
        fields(context,['scope','revision']);need(context['scope']==self.value['scope'] and type(context['revision']) is int and context['revision']==self.value['revision']);need(len(text.encode())<=4096)
        r=strict(text);fields(r,['answer'])
        if r['answer']=='unknown':return {'status':'unknown','text':self.value['unknown'],'sources':[]}
        card=next((c for c in self.value['cards'] if c['id']==r['answer']),None);need(card is not None)
        source={'kind':card['kind'],'id':card['source']};need(source in available)
        text=('You told me: "'+card['text']+'"') if card['kind']=='quote' else card['text']
        return {'status':'answered','text':text,'sources':[source]}
def process(v):
    if v['kind']=='lore':
        from limits_reference import Limits,limited
        lim=Limits(SPEC)
        def run(a):fields(a,['world','select']);return Lore(a['world'],lim.values).select(a['select'])
        return limited(v['args'],lim,run)
    a=v['args'];fields(a,['definition','calls']);answers=Answers(a['definition']);need(isinstance(a['calls'],list) and len(a['calls'])<=8);results=[]
    for call in a['calls']:
        fields(call,['text','context','available']);need(isinstance(call['available'],list) and len(call['available'])<=32)
        try:results.append({'ok':True,'result':answers.resolve(call['text'],call['context'],call['available'])})
        except (ValueError,TypeError,KeyError):results.append({'ok':False})
    return {'offer':answers.offer(),'schema':answers.schema(),'results':results}
