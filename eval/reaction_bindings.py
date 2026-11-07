"""Shared reactions fixture parity through Python CLI/native, Node and C#."""
import argparse,importlib.util,json,pathlib
ROOT=pathlib.Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);a=p.parse_args();assert a.cap==0
 spec=importlib.util.spec_from_file_location('bridge',ROOT/'bindings/python/gamectx_client.py');b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
 fixtures=[]
 for file in sorted((ROOT/'conformance/reactions').glob('*/input.json')):
  error=json.loads(file.with_name('case.json').read_text()).get('error',False);i=str(len(fixtures));request=dict(version=1,id=i,op='local',args=json.loads(file.read_text()));expected=None if error else dict(version=1,id=i,ok=True,result=json.loads(file.with_name('expected.json').read_text()));fixtures.append(dict(request=request,expected=expected,error=error))
 for client in [b.Client(None,ROOT/'target/debug/gamectx'),b.NativeClient(None,ROOT/'bindings/native/target/debug/libgamectx_native.so')]:
  with client:
   for f in fixtures:
    try:got=client.request('local',f['request']['args']);assert not f['error'];assert got==f['expected']['result']
    except ValueError:assert f['error']
 out=ROOT/'target/reactions-bindings';out.mkdir(exist_ok=True)
 enc=lambda v:json.dumps(v,ensure_ascii=False,separators=(',',':'))
 (out/'cases.json').write_text(enc(fixtures)+'\n');(out/'requests.txt').write_text(''.join(enc(f['request'])+'\n'for f in fixtures));(out/'expected.txt').write_text(''.join(('ERROR'if f['error']else enc(f['expected']))+'\n'for f in fixtures))
 print(enc(dict(python_cli_native_checks=2*len(fixtures),shared_fixtures=len(fixtures),actual_provider_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
