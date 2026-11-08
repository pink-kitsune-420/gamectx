"""New informational/conversation workflows through CLI/native Python and C# fixtures."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,importlib.util,json,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import info_inventory_cases as information
import interaction_cases as interactions
from dungeon_reference import encode
ROOT=pathlib.Path(__file__).resolve().parents[2]
def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--fixtures',type=pathlib.Path,required=True);a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    spec=importlib.util.spec_from_file_location('gamectx_client',ROOT/'bindings/python/gamectx_client.py');bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge)
    project=ROOT/'examples/kit/project.json';lib=ROOT/('bindings/native/target/debug/gamectx_native.dll' if sys.platform=='win32' else 'bindings/native/target/debug/libgamectx_native.dylib' if sys.platform=='darwin' else 'bindings/native/target/debug/libgamectx_native.so')
    cases=information.cases()+interactions.cases();fixtures=[];checks=0
    for name,v,error in cases:
        ident='information_'+str(len(fixtures));result=None if error else (interactions.ref.process(v['args']) if v['kind']=='interactions' else information.process(v))
        fixtures.append({'name':name,'request':{'version':1,'id':ident,'op':'local','args':v},'expected':'ERROR' if error else encode({'version':1,'id':ident,'ok':True,'result':result})})
    for connection in [bridge.Client(project,ROOT/'target/debug/gamectx'),bridge.NativeClient(project.read_text(),lib)]:
        with connection:
            for case in fixtures:
                try:
                    result=connection.request('local',case['request']['args']);assert case['expected']!='ERROR',case['name']
                    assert encode(result)==encode(json.loads(case['expected'])['result']),case['name']
                except ValueError:assert case['expected']=='ERROR',case['name']
                checks+=1
    a.fixtures.mkdir(parents=True,exist_ok=True)
    (a.fixtures/'cases.json').write_text(encode(fixtures)+'\n')
    (a.fixtures/'requests.txt').write_text('\n'.join(encode(c['request']) for c in fixtures)+'\n')
    (a.fixtures/'expected.txt').write_text('\n'.join(c['expected'] for c in fixtures)+'\n')
    print(encode({'information_python_bridge_checks':checks,'language_fixtures':len(fixtures),'api_spend_usd':0}))
if __name__=='__main__':main()
