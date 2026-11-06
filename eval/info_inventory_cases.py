"""Independent informational-tool, inventory and structured-lore conformance. API $0."""
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import inventory_reference as inventory
import info_reference as info
import lore_reference as lore
from dungeon_reference import encode
ROOT=pathlib.Path(__file__).resolve().parents[1]
def cases():
    out=[]
    def add(name,kind,args,error=False):out.append((name,{'kind':kind,'args':copy.deepcopy(args)},error))
    source={'id':'line:1','speaker':'player','text':'The oil belongs to Fen.'}
    remember={'name':'info_remember','text':encode({'subject':'item:oil','attribute':'owner','value':'Fen','source':'line:1'}),'sources':[source]}
    lookup={'name':'info_lookup','text':encode({'subject':'item:oil','attribute':'owner'}),'sources':[]}
    add('notes_reported','info',{'scope':'merchant:player','calls':[remember,lookup]})
    other=copy.deepcopy(remember);other['sources'][0]={'id':'line:2','speaker':'keeper','text':'The oil is mine.'};other['text']=encode({'subject':'item:oil','attribute':'owner','value':'keeper','source':'line:2'})
    add('notes_conflict','info',{'scope':'merchant:player','calls':[remember,other,lookup]})
    for name,text in [('unknown_source',encode({'subject':'oil','attribute':'owner','value':'Fen','source':'missing'})),('authority_injection',encode({'subject':'oil','attribute':'owner','value':'Fen','source':'line:1','status':'confirmed'})),('duplicate','{"subject":"oil","subject":"seal","attribute":"owner","value":"Fen","source":"line:1"}'),('oversize',encode({'subject':'oil','attribute':'owner','value':'x'*1025,'source':'line:1'}))]:
        r=copy.deepcopy(remember);r['text']=text;add('notes_'+name,'info',{'scope':'talk','calls':[r,lookup]})
    forget={'name':'info_forget','text':'{"id":1}','sources':[]}
    add('notes_forget','info',{'scope':'talk','calls':[remember,forget,lookup,forget]})
    for attr,val in [('location','workshop'),('preference','quiet'),('relationship',{'person':'keeper','relation':'friend'}),('quest_status',{'stage':'asked','complete':False})]:
        r=copy.deepcopy(remember);r['text']=encode({'subject':'player','attribute':attr,'value':val,'source':'line:1'});add('notes_'+attr,'info',{'scope':'talk','calls':[r]})
    add('notes_capacity','info',{'scope':'talk','calls':[remember]*65})
    snap={'version':1,'revision':0,'items':[{'id':'item:oil','label':'hinge oil','tags':['supply','repair']},{'id':'item:salve','label':'salve','tags':['heal']}],'holdings':[{'owner':'merchant','item':'item:oil','quantity':4},{'owner':'player','item':'item:salve','quantity':1}]}
    move={'item':'item:oil','from':'merchant','to':'player','quantity':2}
    transfer={'op':'transfer','revision':0,'moves':[move],'allow':[move]}
    rows={'op':'rows','owners':['player'],'tags':[]}
    s=copy.deepcopy(snap);s['items'][0]['unique']=True;s['holdings'][0]['quantity']=1;m={**move,'quantity':1};add('unique_trade','inventory',{'snapshot':s,'calls':[{'op':'transfer','revision':0,'moves':[m],'allow':[m]},{'op':'lookup','text':'{"item":"item:oil"}','owners':['merchant','player']}]})
    s['holdings'].append({'owner':'player','item':'item:oil','quantity':1});add('unique_duplicate','inventory',{'snapshot':s,'calls':[]},True)
    add('lookup','inventory',{'snapshot':snap,'calls':[{'op':'lookup','text':'{"item":"item:oil"}','owners':['merchant']},{'op':'lookup','text':'{"item":"item:oil"}','owners':['player']}]})
    add('transfer','inventory',{'snapshot':snap,'calls':[transfer,rows]})
    add('replay','inventory',{'snapshot':snap,'calls':[transfer,transfer,rows]})
    add('tag_filter','inventory',{'snapshot':snap,'calls':[{'op':'rows','owners':['merchant','player'],'tags':['repair']},{'op':'rows','owners':[],'tags':[]}]})
    proposal={'op':'propose','owner':'merchant','text':encode({'revision':0,'item':'item:oil','to':'player','quantity':2}),'allow':[move]}
    add('propose','inventory',{'snapshot':snap,'calls':[proposal,rows]})
    for name,changes in [('denied',{'allow':[]}),('insufficient',{'moves':[{**move,'quantity':5}],'allow':[{**move,'quantity':5}]}),('atomic_later_denied',{'moves':[move,{**move,'quantity':1}]}),('zero',{'moves':[{**move,'quantity':0}]}),('fraction',{'moves':[{**move,'quantity':1.5}]}),('same_owner',{'moves':[{**move,'to':'merchant'}]})]:
        r=copy.deepcopy(transfer);r.update(changes);add(name,'inventory',{'snapshot':snap,'calls':[r,rows]})
    r=copy.deepcopy(proposal);r['text']='{"revision":0,"item":"item:oil","to":"player","quantity":2,"from":"keeper"}';add('bound_owner','inventory',{'snapshot':snap,'calls':[r,rows]})
    s=copy.deepcopy(snap);s['holdings'].append({'owner':'player','item':'item:oil','quantity':2**32-1});add('overflow','inventory',{'snapshot':s,'calls':[transfer,rows]})
    for name,change in [('duplicate_item',lambda s:s['items'].append(s['items'][0])),('duplicate_holding',lambda s:s['holdings'].append(s['holdings'][0])),('unknown_item',lambda s:s['holdings'][0].update(item='missing')),('zero_holding',lambda s:s['holdings'][0].update(quantity=0))]:
        s=copy.deepcopy(snap);change(s);add(name,'inventory',{'snapshot':s,'calls':[]},True)
    world=json.loads((ROOT/'examples/dungeon/world-structured.json').read_text());sel={'actor':'npc:merchant','grants':[],'query':'tonic purpose','subjects':['item:tonic'],'required':['tonic_use'],'limit':4,'max_bytes':2048}
    add('lore_values','lore',{'world':world,'select':sel})
    for name,value in [('scalar',12),('null',None),('array',[False,'oil',{'id':'item:oil'}]),('unicode',{'owner':'旅人','count':2})]:
        w=copy.deepcopy(world);w['entries'][2]['value']=value;add('lore_'+name,'lore',{'world':w,'select':sel})
    w=copy.deepcopy(world);w['entries'][0]['text']='Unexpected.';add('lore_mixed','lore',{'world':w,'select':sel},True)
    w=copy.deepcopy(world);w['entries'][0]['value']='x'*4095;add('lore_oversize','lore',{'world':w,'select':sel},True)
    a={'world':world,'select':{**sel,'max_bytes':2}};add('lore_required_fit','lore',a,True)
    return out

def process(v):return {'info':info.process,'inventory':inventory.process,'lore':lambda a:lore.process({'kind':'lore','args':a})}[v['kind']](v['args'])
def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
    total=0
    for name,v,error in cases():
        try:expected=process(v);assert not error,name
        except (ValueError,TypeError,KeyError):assert error,name;expected=None
        r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=encode(v),capture_output=True,text=True)
        assert bool(r.returncode)==error,(name,r.stderr)
        if not error:assert r.stdout.strip()==encode(expected),(name,r.stdout,encode(expected))
        if a.write_conformance:
            d=ROOT/'conformance/info-inventory'/name;d.mkdir(parents=True,exist_ok=True)
            (d/'case.json').write_text(encode({'op':'local',**({'error':True} if error else {})})+'\n');(d/'input.json').write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
            if not error:(d/'expected.json').write_text(encode(expected)+'\n')
        total+=1
    print(encode({'workflows':total,'api_spend_usd':0}))
if __name__=='__main__':main()
