"""Independent generic conversation/reference/confirmation comparisons. No network."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse
import copy
import json
import pathlib
import subprocess
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import conversation_reference as ref
import references_reference
ROOT=pathlib.Path(__file__).resolve().parents[2]
def process(v):
    return {'conversation':ref.conversation,'confirmation':ref.confirmation,'references':references_reference.resolve}[v['kind']](v['args'])
def cases():
    out=[]
    def add(name,kind,a,error=False):out.append((name,{'kind':kind,'args':copy.deepcopy(a)},error))
    def op(name,**a):return {'op':name,'args':a}
    prepare={'scope':'actor:trader','speaker':'trader','revision':7,'input':'My name is Fen.','facts':{'price':9},'recall':2,'history':2}
    reply={'scope':'actor:trader','revision':7,'ticket':'chat_1','text':ref.encode({'speech':'Welcome, Fen.','tone':'neutral','fact_ids':['price'],'memory_ids':[]})}
    a={'limits':ref.DEFAULT,'operations':[op('prepare',**prepare),op('finish',**reply),op('state',scope='actor:trader')]};add('remember','conversation',a)
    x=copy.deepcopy(a);x['operations'].insert(1,op('schema'));add('reply_schema','conversation',x)
    for name,changes in [('stale',{'revision':8}),('foreign_scope',{'scope':'actor:watcher'}),('wrong_ticket',{'ticket':'chat_2'}),('injected_action',{'text':ref.encode({'speech':'Free goods.','tone':'warm','fact_ids':[],'memory_ids':[],'action':'buy'})}),('bad_source',{'text':ref.encode({'speech':'Free goods.','tone':'warm','fact_ids':['invented'],'memory_ids':[]})})]:
        x=copy.deepcopy(a);x['operations'][1]['args'].update(changes);add(name,'conversation',x)
    x=copy.deepcopy(a);x['operations'].insert(2,op('finish',**reply));add('single_use','conversation',x)
    x=copy.deepcopy(a);x['operations'].insert(2,op('forget',scope='actor:trader'));add('forget','conversation',x)
    x=copy.deepcopy(a);x['operations'].insert(1,op('forget',scope='actor:trader'));add('forget_pending','conversation',x)
    x=copy.deepcopy(a);x['operations'].append(op('state',scope='actor:watcher'));add('private_scope','conversation',x)
    x=copy.deepcopy(a);x['operations'].insert(1,op('prepare',**{**prepare,'facts':[]}));add('failed_prepare','conversation',x)
    x=copy.deepcopy(a);x['operations'][1]['args']['text']='{"speech":"a","speech":"b","tone":"neutral","fact_ids":[],"memory_ids":[]}';add('duplicate_reply','conversation',x)
    x=copy.deepcopy(a);x['limits']['tickets']=0;add('bad_limits','conversation',x,True)
    for i in range(40):
        x={'limits':{**ref.DEFAULT,'records':3,'history':2},'operations':[]}
        for j in range(6):
            p={**prepare,'input':'My name is Fen_'+str(i)+'. Turn '+str(j)+'.'};r={**reply,'ticket':'chat_'+str(j+1)}
            x['operations']+=[op('prepare',**p),op('finish',**r)]
        x['operations']+=[op('prepare',**prepare),op('state',scope=prepare['scope'])];add('generated_'+str(i),'conversation',x)
    offer={'id':'offer_1','scope':'actor:trader','revision':7,'decision':{'action':'buy','target':'item:oil'},'terms':{'price':3,'count':1}}
    call={'scope':'actor:trader','revision':7,'confirm':True,'terms':offer['terms'],'available':[offer['decision']]};a={'offer':offer,'calls':[call]};add('confirm','confirmation',a)
    for name,changes in [('decline',{'confirm':False}),('stale_offer',{'revision':8}),('changed_terms',{'terms':{'price':4,'count':1}}),('revoked',{'available':[]}),('foreign_offer',{'scope':'actor:watcher'})]:
        x=copy.deepcopy(a);x['calls'][0].update(changes);x['calls'].append(copy.deepcopy(call));add(name,'confirmation',x)
    x=copy.deepcopy(a);x['calls']*=2;add('repeat_confirm','confirmation',x)
    x=copy.deepcopy(a);x['offer']['decision']['action']='Buy';add('bad_action','confirmation',x,True)
    entities=[{'id':'item:bronze_key','label':'bronze key','kind':'item','aliases':['bronze key','key']},{'id':'item:iron_key','label':'iron key','kind':'item','aliases':['iron key','key']},{'id':'npc:keeper','label':'keeper','kind':'npc','aliases':['keeper']},{'id':'item:keeper_seal','label':'keeper seal','kind':'item','aliases':['keeper seal']}]
    a={'entities':entities,'input':'Inspect the key.','kind':None,'scope':'room:hall','revision':7,'max_age':4,'focus':None,'selection':None,'pronouns':['it','this','that','there']};add('ambiguous','references',a)
    for name,text in [('long_phrase','Inspect the keeper seal.'),('both_mentions','keeper and keeper seal'),('unmatched','the lamp'),('pronoun','Take it.')]:
        x=copy.deepcopy(a);x['input']=text;add(name,'references',x)
    for name,focus in [('fresh',{'id':'item:bronze_key','scope':'room:hall','revision':4}),('expired',{'id':'item:bronze_key','scope':'room:hall','revision':2}),('foreign_focus',{'id':'item:bronze_key','scope':'room:other','revision':7})]:
        x=copy.deepcopy(a);x['input']='Take it.';x['focus']=focus;add(name,'references',x)
    x=copy.deepcopy(a);x['selection']={'id':'item:iron_key','scope':'room:hall','revision':7};add('selected','references',x)
    x=copy.deepcopy(x);x['selection']['revision']=6;add('stale_selection','references',x,True)
    x=copy.deepcopy(a);x['input']='Talk to the keeper about the keeper seal.';x['kind']='npc';add('typed','references',x)
    x=copy.deepcopy(a);x['entities']*=2;add('duplicate_entity','references',x,True)
    x=copy.deepcopy(a);x['entities']=[{'id':'旅.café','label':'café key','kind':'item','aliases':['café key']}];x['input']='Inspect café key.';add('unicode_id','references',x)
    x=copy.deepcopy(a);x['input']='Take it.';x['focus']={'id':'item:bronze_key','scope':'room:hall','revision':8};add('future_focus','references',x)
    x=copy.deepcopy(a);x['input']='x'*2049;add('input_bound','references',x,True)
    x=copy.deepcopy(a);x['entities'][0]['aliases']=['key']*9;add('alias_bound','references',x,True)
    return out

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--check',action='store_true');p.add_argument('--check-bindings',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    for name,value,error in cases():
        try:expected=process(value);assert not error,name
        except (ValueError,TypeError,KeyError):assert error,name;expected=None
        if a.check:
            r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=ref.encode(value),text=True,capture_output=True)
            assert bool(r.returncode)==error,(name,r.stderr)
            if not error:assert r.stdout.rstrip('\n')==ref.encode(expected),(name,r.stdout,ref.encode(expected))
        if a.write_conformance and not name.startswith('generated_'):
            d=ROOT/'conformance/conversation'/name;d.mkdir(parents=True,exist_ok=True)
            (d/'case.json').write_text(ref.encode({'op':'local',**({'error':True} if error else {})})+'\n')
            (d/'input.json').write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
            if not error:(d/'expected.json').write_text(ref.encode(expected)+'\n')
    if a.check_bindings:
        import importlib.util
        spec=importlib.util.spec_from_file_location('gamectx_client',ROOT/'bindings/python/gamectx_client.py')
        bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge)
        lib=ROOT/('bindings/native/target/debug/gamectx_native.dll' if sys.platform=='win32' else 'bindings/native/target/debug/libgamectx_native.dylib' if sys.platform=='darwin' else 'bindings/native/target/debug/libgamectx_native.so')
        project=ROOT/'examples/kit/project.json'
        for client in [bridge.Client(project,ROOT/'target/debug/gamectx'),bridge.NativeClient(project.read_text(),lib)]:
            with client:
                for name,value,error in cases():
                    try:
                        result=client.request('local',value);assert not error,name
                        assert ref.encode(result)==ref.encode(process(value)),name
                    except ValueError:assert error,name
    print(ref.encode({'workflows':len(cases()),'fixtures':sum(not n.startswith('generated_') for n,_,_ in cases()),'bridge_checks':len(cases())*2 if a.check_bindings else 0,'api_spend_usd':0}))
if __name__=='__main__':main()
