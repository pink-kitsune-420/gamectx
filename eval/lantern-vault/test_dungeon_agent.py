"""Failure-path checks for the local agent; HTTP is replaced, no generation occurs."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import json
import pathlib
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import dungeon_agent as agent

class FailedServer:
    sends=0
    def __init__(self,*args):pass
    def data(self,path,payload=None):
        if path=='/api/tags':return {'models':[{'name':'fixture-local','digest':'0'*64}]}
        if path=='/api/show':return {'template':'fixture'}
        return {'version':'fixture'}
    def chat(self,payload):
        type(self).sends+=1
        raise OSError('simulated connection failure')

class OverflowServer(FailedServer):
    def chat(self,payload):
        type(self).sends+=1
        return {'final':{'prompt_eval_count':5000}}

class AgentChecks(unittest.TestCase):
    def invoke(self,server,seeds):
        server.sends=0
        with tempfile.TemporaryDirectory() as d:
            path=pathlib.Path(d)/'result.json'
            argv=['dungeon_agent.py','--cap','0','--tokens-only','--model','fixture-local','--seeds',seeds,'--max-calls','1','--output',str(path)]
            with patch.object(sys,'argv',argv),patch.object(agent.oe,'Server',server),patch.object(agent.oe,'require_local_weights',lambda *_:None):
                agent.main()
            return json.loads(path.read_text())
    def test_failed_dispatch_consumes_call_reservation(self):
        result=self.invoke(FailedServer,'0,7')
        self.assertEqual(FailedServer.sends,1)
        self.assertEqual(len(result['calls']),1)
        self.assertEqual(result['calls'][0]['status'],'failed')
        self.assertTrue(all(e['stop']=='call_cap' for e in result['episodes']))
    def test_native_count_overflow_never_dispatches_decision(self):
        result=self.invoke(OverflowServer,'7')
        self.assertEqual(OverflowServer.sends,1)
        self.assertEqual([c['phase'] for c in result['calls']],['preflight'])
        self.assertNotEqual(result['episodes'][0]['stop'],'token_probe')
        self.assertFalse(result['episodes'][0]['won'])
if __name__=='__main__':unittest.main()
