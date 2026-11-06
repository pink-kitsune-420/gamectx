#!/usr/bin/env python3
"""Exercise the real persistent/native adapters against the independent reference."""
import importlib.util
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'eval'))
import kit_eval as ref
spec = importlib.util.spec_from_file_location('gamectx_client', ROOT / 'bindings/python/gamectx_client.py')
client = importlib.util.module_from_spec(spec)
spec.loader.exec_module(client)
project = ROOT / 'examples/kit/project.json'
binary = ROOT / 'target/debug/gamectx'
library = ROOT / ('bindings/native/target/debug/gamectx_native.dll' if sys.platform == 'win32' else
                  'bindings/native/target/debug/libgamectx_native.dylib' if sys.platform == 'darwin' else
                  'bindings/native/target/debug/libgamectx_native.so')
checks = 0
for connection in [client.Client(project, binary), client.NativeClient(project.read_text(), library)]:
    with connection:
        for n, v, error in ref.cases():
            if v['project'] != json.loads(project.read_text()):
                continue
            try:
                result = connection.request(v['op'], v['args'])
                assert not error, n
                assert ref.encode(result) == ref.encode(ref.process(v)), n
            except ValueError:
                assert error, n
            checks += 1
        assert connection.request('doctor', {})['warnings'] == []
        checks += 1
# Every definition, including rejected configurations, also crosses the C ABI.
for n, v, error in ref.cases():
    try:
        with client.NativeClient(ref.encode(v['project']), library) as connection:
            result = connection.request(v['op'], v['args'])
            assert not error, n
            assert ref.encode(result) == ref.encode(ref.process(v)), n
    except ValueError:
        assert error, n
    checks += 1
if '--csharp-fixtures' in sys.argv:
    destination = pathlib.Path(sys.argv[sys.argv.index('--csharp-fixtures') + 1])
    destination.mkdir(parents=True,exist_ok=True)
    requests, expected = [], []
    for n,v,error in ref.cases():
        if v['project'] != json.loads(project.read_text()): continue
        ident = 'cs_' + str(len(requests))
        requests.append(ref.encode({'version':1,'id':ident,'op':v['op'],'args':v['args']}))
        expected.append('ERROR' if error else ref.encode({'version':1,'id':ident,'ok':True,'result':ref.process(v)}))
    (destination/'requests.txt').write_text('\n'.join(requests)+'\n')
    (destination/'expected.txt').write_text('\n'.join(expected)+'\n')
# Native and line protocols preserve original large-number spelling in a direct projection.
p, s = ref.fixture()
p['recipes'][0]['datasets'][0]['view'] = {'fields': ['id','count']}
s['tables']['stock']['rows'] = [{'id': 'grove:Élan.9007199254740993', 'kind':'food', 'count':9007199254740993,'secret':'private'}]
with client.NativeClient(ref.encode(p), library) as connection:
    assert connection.request('query', {'recipe':'conversation','snapshot':s,'sql':'SELECT count FROM stock'}) == [9007199254740993]
    checks += 1
print(json.dumps({'python_bridge_checks': checks, 'api_spend_usd': 0}))
