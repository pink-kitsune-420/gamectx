#!/usr/bin/env python3
"""Offline protocol/scoring checks; fake loopback server, no installed model required."""
import json
import pathlib
import sys
import threading
import unittest
from http.server import BaseHTTPRequestHandler,HTTPServer
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import ollama_eval as e
import local_cases as cases
import local_reference as ref
class Handler(BaseHTTPRequestHandler):
    def log_message(self,*a):pass
    def do_POST(self):
        self.rfile.read(int(self.headers['Content-Length']))
        self.send_response(200);self.send_header('Content-Type','application/x-ndjson');self.end_headers()
        rows=[{'message':{'content':'{"action":"wait",'}},{'message':{'content':'"target":null}'},'done':True,'prompt_eval_count':21,'eval_count':8,'eval_duration':1000000,'done_reason':'stop'}]
        if self.server.broken:rows.pop()
        for row in rows:self.wfile.write((json.dumps(row)+'\n').encode());self.wfile.flush()
class Tests(unittest.TestCase):
    def test_stream_metrics_and_incomplete_reply(self):
        server=HTTPServer(('127.0.0.1',0),Handler);server.broken=False
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        try:
            client=e.Server('http://127.0.0.1:'+str(server.server_port),2)
            response=client.chat({'messages':[]})
            self.assertEqual(e.strict(response['text']),{'action':'wait','target':None})
            self.assertIsNotNone(response['ttft_micros'])
            self.assertIsNone(e.attempt(response,'decision',True)['cached_tokens'])
            server.broken=True
            with self.assertRaises(ValueError):client.chat({'messages':[]})
        finally:server.shutdown();server.server_close();thread.join()
    def test_remote_redirect_and_duplicate_keys_rejected(self):
        for url in ['https://127.0.0.1','http://example.test','http://a:b@localhost','http://localhost/path']:
            with self.assertRaises(ValueError):e.origin(url)
        with self.assertRaises(ValueError):e.strict('{"a":1,"a":2}')
    def test_oracles_and_scopes(self):
        suite=e.scenarios(30,17001)
        self.assertEqual({c['family'] for c in suite},set(range(6)))
        for c in suite:
            ref.choices(c['choices']);self.assertIn(c['expected'],c['choices']['choices'])
            if c['family']==4:
                target=c['scene']['required_facts']['best_affordable'];self.assertEqual(c['expected']['target'],target)
                eligible=[x for x in c['offers'] if x['price']<=c['scene']['required_facts']['coins']]
                self.assertEqual(target,max(eligible,key=lambda x:x['units'])['id'] if eligible else None)
        self.assertNotEqual(e.digest(suite),e.digest(e.scenarios(30,17002)))
    def test_runner_ownership_and_cloud_models(self):
        processes={1:{'exe':'ollama','ppid':0,'runner_arg':False,'rss':10},2:{'exe':'llama-server','ppid':1,'runner_arg':False,'rss':20},3:{'exe':'llama-server','ppid':0,'runner_arg':False,'rss':30}}
        self.assertTrue(e.runner_owned(2,processes));self.assertFalse(e.runner_owned(3,processes));self.assertFalse(e.runner_owned(1,processes))
        e.require_local_weights({'details':{'format':'gguf'},'size':200},{})
        for model in [{'details':{'format':'gguf'},'size':200,'remote_host':'cloud'}, {'details':{'format':'unknown'},'size':0}]:
            with self.assertRaises(ValueError):e.require_local_weights(model,{})
    def test_telemetry_does_not_invent_usage_or_count_preflight_as_retry(self):
        t=cases.trial();a=dict(t['attempts'][0]);a['phase']='preflight';a['truncated']=True;t['attempts'].insert(0,a)
        t['attempts'][1]['input_tokens']=None;t['memory']['runner_rss_bytes']=None
        r=ref.report({'version':1,'trials':[t]})['conditions'][0]
        self.assertEqual(r['retries'],0);self.assertEqual(r['truncated'],0)
        self.assertIsNone(r['input_tokens']);self.assertIsNone(r['memory']['runner_rss_bytes'])
if __name__=='__main__':unittest.main()
