"""Independent lore/answer conformance, byte selection and native/bridge comparisons."""
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import lore_reference as ref
ROOT=pathlib.Path(__file__).resolve().parents[1]
def cases():
    out=[]
    def add(n,k,a,error=False):out.append((n,{'kind':k,'args':copy.deepcopy(a)},error))
    world=json.loads((ROOT/'examples/dungeon/world.json').read_text())
    select={'actor':'npc:merchant','grants':[],'query':'tonic purpose','subjects':['item:tonic'],'required':[],'limit':4,'max_bytes':1024}
    a={'world':world,'select':select};add('select','lore',a)
    for name,changes in [('private',{'subjects':['npc:keeper'],'query':'keeper background'}),('gated',{'actor':'npc:keeper','subjects':['room:vault'],'query':'inscription'}),('granted',{'actor':'npc:keeper','subjects':['room:vault'],'query':'inscription','grants':['vault_open']}),('no_match',{'query':'xyzzymissingterm','subjects':[]}),('required',{'required':['tonic_use'],'query':''})]:
        x=copy.deepcopy(a);x['select'].update(changes);add(name,'lore',x)
    for name,changes in [('required_private',{'required':['keeper_background']}),('required_overflow',{'required':['tonic_use'],'max_bytes':2}),('required_count',{'required':['tonic_use'],'limit':0}),('future_grant_typo',{'grants':[3]})]:
        x=copy.deepcopy(a);x['select'].update(changes);add(name,'lore',x,True)
    x=copy.deepcopy(a);x['world']['entries']*=2;add('duplicate','lore',x,True)
    x=copy.deepcopy(a);x['world']['version']=2;add('version','lore',x,True)
    x=copy.deepcopy(a);x['world']['entries'][0]['known_by']=[];add('empty_audience','lore',x,True)
    x=copy.deepcopy(a);x['world']['entries'][0]['text']='x'*1025;add('long_text','lore',x,True)
    for i in range(48):
        x=copy.deepcopy(a);x['select']['subjects']=[];x['select']['query']='background tonic seal weather';x['select']['max_bytes']=2+i*30;add('generated_budget_'+str(i),'lore',x)
    cards=[{'id':'price','subject':'item:oil','topic':'price','kind':'state','source':'item:oil','text':'The oil costs 3 coins.'},
        {'id':'quote_1','subject':'player','topic':'home','kind':'quote','source':'1','text':'My home is Reed Hollow.'}]
    definition={'version':1,'scope':'player:trader','revision':7,'unknown':"You haven't supplied that information.",'cards':cards}
    call={'text':'{"answer":"price"}','context':{'scope':'player:trader','revision':7},'available':[{'kind':'state','id':'item:oil'},{'kind':'quote','id':'1'}]};a={'definition':definition,'calls':[call]};add('answer','answers',a)
    for name,changes in [('quote',{'text':'{"answer":"quote_1"}'}),('unknown',{'text':'{"answer":"unknown"}'}),('injection',{'text':'{"answer":"price","speech":"Free stock!"}'}),('duplicate_json',{'text':'{"answer":"price","answer":"quote_1"}'}),('foreign_id',{'text':'{"answer":"invented"}'}),('stale',{'context':{'scope':'player:trader','revision':8}}),('foreign_scope',{'context':{'scope':'other','revision':7}}),('revoked',{'available':[]})]:
        x=copy.deepcopy(a);x['calls'][0].update(changes);add(name,'answers',x)
    x=copy.deepcopy(a);x['definition']['cards']*=2;add('duplicate_card','answers',x,True)
    x=copy.deepcopy(a);x['definition']['cards'][0]['kind']='assistant';add('assistant_source','answers',x,True)
    x=copy.deepcopy(a);x['definition']['cards'][0]['id']='unknown';add('reserved_id','answers',x,True)
    x=copy.deepcopy(a);x['definition']['cards'].append({'id':'old_price','subject':'item:oil','topic':'price','kind':'lore','source':'old_price','text':'The oil costs 1 coin.'});add('live_over_lore','answers',x)
    x=copy.deepcopy(x);x['calls'][0]['text']='{"answer":"old_price"}';add('shadowed_source','answers',x)
    for i in range(32):
        x=copy.deepcopy(a);x['definition']['revision']=i;x['calls'][0]['context']['revision']=i;add('generated_revision_'+str(i),'answers',x)
    return out

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--check',action='store_true');p.add_argument('--check-bindings',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    for name,value,error in cases():
        try:expected=ref.process(value);assert not error,name
        except (ValueError,TypeError,KeyError):assert error,name;expected=None
        if a.check:
            r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=ref.encode(value),text=True,capture_output=True)
            assert bool(r.returncode)==error,(name,r.stderr)
            if not error:assert r.stdout.rstrip('\n')==ref.encode(expected),(name,r.stdout,ref.encode(expected))
        if a.write_conformance and not name.startswith('generated_'):
            d=ROOT/'conformance/lore'/name;d.mkdir(parents=True,exist_ok=True)
            (d/'case.json').write_text(ref.encode({'op':'local',**({'error':True} if error else {})})+'\n');(d/'input.json').write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
            if not error:(d/'expected.json').write_text(ref.encode(expected)+'\n')
    if a.check_bindings:
        import importlib.util
        spec=importlib.util.spec_from_file_location('gamectx_client',ROOT/'bindings/python/gamectx_client.py');bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge)
        lib=ROOT/('bindings/native/target/debug/gamectx_native.dll' if sys.platform=='win32' else 'bindings/native/target/debug/libgamectx_native.dylib' if sys.platform=='darwin' else 'bindings/native/target/debug/libgamectx_native.so');project=ROOT/'examples/kit/project.json'
        for client in [bridge.Client(project,ROOT/'target/debug/gamectx'),bridge.NativeClient(project.read_text(),lib)]:
            with client:
                for name,value,error in cases():
                    try:result=client.request('local',value);assert not error,name;assert ref.encode(result)==ref.encode(ref.process(value)),name
                    except ValueError:assert error,name
    print(ref.encode({'workflows':len(cases()),'fixtures':sum(not n.startswith('generated_') for n,_,_ in cases()),'bridge_checks':2*len(cases()) if a.check_bindings else 0,'api_spend_usd':0}))
if __name__=='__main__':main()
