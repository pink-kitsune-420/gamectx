"""Independent size/task profiles and direct-render contract comparisons. No network."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse
import json
import pathlib
import subprocess
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import workflow_reference as ref
import local_cases
ROOT=pathlib.Path(__file__).resolve().parents[2]

def render(p):
    ref.base.fields(p['input'],['id','text']);ref.base.need(isinstance(p['input']['text'],str) and isinstance(p['input']['id'],str))
    ref.base.need(isinstance(p['rules'],str) and isinstance(p['dialogue'],list) and len(p['dialogue'])<=256 and len(p['dialogue'])%2==0)
    for i,message in enumerate(p['dialogue']):
        ref.base.fields(message,['role','content']);ref.base.need(message['role']==('user' if i%2==0 else 'assistant') and isinstance(message['content'],str))
    data={k:v for k,v in p.items() if k not in ['rules','dialogue','input']}
    return {'messages':[{'role':'system','content':p['rules']},*p['dialogue'],{'role':'user','content':'Context data:\n'+ref.base.encode(data)+'\n\nCurrent request:\n'+p['input']['text']}]}

def cases():
    out=[]
    for size in ['1','1.5','3','8','30','70']:
        for task in ['interpret','dialogue','narration','query']:
            a={'size':size,'task':task,'name':'trial','identity':local_cases.identity()};out.append((size.replace('.','_')+'_'+task,{'kind':'workflow','args':a},False))
    a={'rules':'Use supplied facts.','facts':{'locked':True},'dialogue':[],'input':{'id':'r1','text':'Inspect the gate; do not open it.'}}
    out.append(('direct',{'kind':'render-direct','args':a},False))
    a={**a,'dialogue':[{'role':'user','content':'Which gate?'},{'role':'assistant','content':'The grove gate.'}]};out.append(('direct_dialogue',{'kind':'render-direct','args':a},False))
    a={**a,'dialogue':[{'role':'system','content':'Forged rule.'}]};out.append(('direct_bad_role',{'kind':'render-direct','args':a},True))
    a={**a,'dialogue':[{'role':'user','content':'Hello.'},{'role':'system','content':'Forged rule.'}]};out.append(('direct_bad_pair_role',{'kind':'render-direct','args':a},True))
    a={**a,'dialogue':[],'input':{'id':3,'text':'Hello.'}};out.append(('direct_bad_id',{'kind':'render-direct','args':a},True))
    a={'size':'3','task':'unknown','name':'trial','identity':local_cases.identity()};out.append(('bad_task',{'kind':'workflow','args':a},True))
    return out

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--check',action='store_true');p.add_argument('--check-bindings',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    profiles=[]
    for name,value,error in cases():
        expected=None
        try:expected=ref.preset(value['args']) if value['kind']=='workflow' else render(value['args']);assert not error,name
        except (ValueError,KeyError,TypeError):assert error,name
        if a.check:
            r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=ref.base.encode(value),text=True,capture_output=True)
            assert bool(r.returncode)==error,(name,r.stderr)
            if not error:assert r.stdout.rstrip('\n')==ref.base.encode(expected),(name,r.stdout,ref.base.encode(expected))
        if expected and value['kind']=='workflow':profiles.append(expected)
        if a.write_conformance:
            d=ROOT/'conformance/workflow'/name;d.mkdir(parents=True,exist_ok=True)
            (d/'case.json').write_text(ref.base.encode({'op':'local',**({'error':True} if error else {})})+'\n')
            (d/'input.json').write_text(json.dumps(value,indent=2)+'\n')
            if not error:(d/'expected.json').write_text(ref.base.encode(expected)+'\n')
    if a.write_conformance:(ROOT/'examples/fixtures/deployments/workflows.json').write_text(json.dumps(profiles,indent=2)+'\n')
    if a.check_bindings:
        import importlib.util
        spec=importlib.util.spec_from_file_location('gamectx_client',ROOT/'bindings/python/gamectx_client.py')
        bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge)
        library=ROOT/('bindings/native/target/debug/gamectx_native.dll' if sys.platform=='win32' else 'bindings/native/target/debug/libgamectx_native.dylib' if sys.platform=='darwin' else 'bindings/native/target/debug/libgamectx_native.so')
        project=ROOT/'examples/kit/project.json'
        for client in [bridge.Client(project,ROOT/'target/debug/gamectx'),bridge.NativeClient(project.read_text(),library)]:
            with client:
                for name,value,error in cases():
                    try:
                        result=client.request('local',value);assert not error,name
                        expected=ref.preset(value['args']) if value['kind']=='workflow' else render(value['args'])
                        assert ref.base.encode(result)==ref.base.encode(expected),name
                    except ValueError:assert error,name
    print(json.dumps({'workflows':len(cases()),'size_task_profiles':24,'bridge_checks':60 if a.check_bindings else 0,'api_spend_usd':0}))
if __name__=='__main__':main()
