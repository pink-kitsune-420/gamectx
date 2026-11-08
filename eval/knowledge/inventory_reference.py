"""Independent integer inventory prototype. Offline; no provider calls."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from conversation_reference import fields,need,uint,scope,strict
from dungeon_reference import encode
from lore_reference import strings
from limits_reference import Limits,limited
MAX=2**32-1
SPEC=[('items',128,65536),('holdings',256,1000000),('bytes',262144,67108864)]
DEFAULT={'items':128,'holdings':256,'bytes':262144}
class Inventory:
    def __init__(self,v,lim=DEFAULT):
        self.lim=lim
        fields(v,['version','revision','items','holdings']);need(v['version']==1 and type(v['version']) is int and len(encode(v).encode())<=lim['bytes']);uint(v['revision'],MAX)
        need(isinstance(v['items'],list) and len(v['items'])<=lim['items'] and isinstance(v['holdings'],list) and len(v['holdings'])<=lim['holdings'])
        ids=set()
        for i in v['items']:
            fields(i,['id','label','tags']+(['unique'] if 'unique' in i else []));need('unique' not in i or type(i['unique']) is bool);scope(i['id']);scope(i['label']);strings(i['tags'],16);need(i['id'] not in ids);ids.add(i['id'])
        seen=set()
        for h in v['holdings']:
            fields(h,['owner','item','quantity']);scope(h['owner']);need(h['item'] in ids);uint(h['quantity'],MAX);need(h['quantity']>0 and (h['owner'],h['item']) not in seen);seen.add((h['owner'],h['item']))
        units={}
        for h in v['holdings']:units[h['item']]=units.get(h['item'],0)+h['quantity']
        for i in v['items']:
            if i.get('unique'):need(units.get(i['id'],0)<=1)
        self.v=copy.deepcopy(v)
    def rows(self,owners,tags):
        strings(owners,32);strings(tags,16)
        return [{'id':i['id'],'label':i['label'],'tags':i['tags'],'owner':h['owner'],'count':h['quantity']} for h in self.v['holdings'] if h['owner'] in owners for i in self.v['items'] if i['id']==h['item'] and all(t in i['tags'] for t in tags)]
    def transfer(self,revision,moves,allow):
        uint(revision,MAX);need(revision==self.v['revision'] and isinstance(moves,list) and 1<=len(moves)<=32 and revision<MAX);draft=copy.deepcopy(self.v)
        for m in moves:
            fields(m,['item','from','to','quantity']);scope(m['item']);scope(m['from']);scope(m['to']);uint(m['quantity'],MAX);need(m['quantity']>0 and m['from']!=m['to'] and m in allow)
            src=next((h for h in draft['holdings'] if h['item']==m['item'] and h['owner']==m['from']),None);need(src is not None and src['quantity']>=m['quantity'])
            dst=next((h for h in draft['holdings'] if h['item']==m['item'] and h['owner']==m['to']),None)
            if dst:need(dst['quantity']+m['quantity']<=MAX);dst['quantity']+=m['quantity']
            else:draft['holdings'].append({'owner':m['to'],'item':m['item'],'quantity':m['quantity']})
            src['quantity']-=m['quantity']
        draft['holdings']=[h for h in draft['holdings'] if h['quantity']];draft['revision']+=1
        need(len(draft['holdings'])<=self.lim['holdings']);self.v=draft  # moves conserve units, so uniqueness holds
        return copy.deepcopy(self.v)
    def propose(self,owner,text,allow):
        need(len(text.encode())<=4096);a=strict(text);fields(a,['revision','item','to','quantity'])
        return self.transfer(a['revision'],[{'item':a['item'],'from':owner,'to':a['to'],'quantity':a['quantity']}],allow)
def process(a):
    lim=Limits(SPEC);return limited(a,lim,lambda b:run(b,lim.values))
def run(a,lim):
    need(len(encode(a).encode())<=4*lim['bytes'])
    fields(a,['snapshot','calls']);need(isinstance(a['calls'],list) and len(a['calls'])<=64);inv=Inventory(a['snapshot'],lim);out=[]
    for c in a['calls']:
        try:
            if c['op']=='rows':fields(c,['op','owners','tags']);v=inv.rows(c['owners'],c['tags'])
            elif c['op']=='lookup':
                fields(c,['op','text','owners']);a=strict(c['text']);fields(a,['item']);scope(a['item']);v={'revision':inv.v['revision'],'rows':[r for r in inv.rows(c['owners'],[]) if r['id']==a['item']]}
            elif c['op']=='transfer':fields(c,['op','revision','moves','allow']);v=inv.transfer(c['revision'],c['moves'],c['allow'])
            elif c['op']=='propose':fields(c,['op','owner','text','allow']);v=inv.propose(c['owner'],c['text'],c['allow'])
            else:raise ValueError('unknown op')
            out.append({'ok':True,'result':v})
        except (ValueError,TypeError,KeyError):out.append({'ok':False})
    return {'snapshot':inv.v,'results':out}
