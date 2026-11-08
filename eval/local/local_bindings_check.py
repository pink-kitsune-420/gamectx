#!/usr/bin/env python3
"""Offline local-workflow parity through the shipped Python/native/C# transports."""
import importlib.util
import pathlib
import sys
ROOT=pathlib.Path(__file__).resolve().parents[2]
sys.path[1:1]=[str(d) for d in sorted((ROOT/'eval').iterdir()) if d.is_dir()]
import local_cases
import local_reference as ref
spec=importlib.util.spec_from_file_location('gamectx_client',ROOT/'bindings/python/gamectx_client.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
library=ROOT/('bindings/native/target/debug/gamectx_native.dll' if sys.platform=='win32' else 'bindings/native/target/debug/libgamectx_native.dylib' if sys.platform=='darwin' else 'bindings/native/target/debug/libgamectx_native.so')
project=ROOT/'examples/kit/project.json';checks=0
for client in [c.Client(project,ROOT/'target/debug/gamectx'),c.NativeClient(project.read_text(),library)]:
    with client:
        for name,v,error in local_cases.cases():
            try:
                result=client.request('local',v)
                assert not error,name
                assert ref.encode(result)==ref.encode(ref.process(v)),name
            except ValueError:assert error,name
            checks+=1
out=ROOT/'target/local-csharp';out.mkdir(parents=True,exist_ok=True)
requests=[];expected=[]
for name,v,error in local_cases.cases():
    if name.startswith('generated_'):continue
    ident='local_'+str(len(requests))
    requests.append(ref.encode({'version':1,'id':ident,'op':'local','args':v}))
    expected.append('ERROR' if error else ref.encode({'version':1,'id':ident,'ok':True,'result':ref.process(v)}))
(out/'requests.txt').write_text('\n'.join(requests)+'\n');(out/'expected.txt').write_text('\n'.join(expected)+'\n')
print(ref.encode({'local_python_checks':checks,'local_csharp_fixtures':len(requests),'api_spend_usd':0}))
