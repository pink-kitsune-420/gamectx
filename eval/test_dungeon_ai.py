"""Player-facing AI controls, using real game rules and fake generation. No HTTP."""
import pathlib
import sys
import unittest
from unittest import mock
import contextlib
import io
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[1]/'examples/dungeon'))
import play_ai as ui
ROOT=pathlib.Path(__file__).resolve().parents[1]
class Replies:
    context=2048
    def __init__(self,*items):self.items=list(items)
    def generate(self,request,output):return self.items.pop(0)

def move(h,room):
    s=h.call('state',{});h.call('act',{'revision':s['revision'],'decision':{'action':'go','target':'room:'+room}})

class HumanAIChecks(unittest.TestCase):
    def setUp(self):self.host=ui.Host(ROOT/'target/debug/examples/dungeon',7)
    def tearDown(self):self.host.close()
    def test_natural_world_change_requires_human_confirmation(self):
        before=self.host.call('state',{})
        r=ui.interpret(self.host,Replies('{"choice":"go_gallery"}'),'Walk to the gallery.',lambda _:False)
        self.assertEqual(r['status'],'cancelled');self.assertEqual(before,self.host.call('state',{}))
        r=ui.interpret(self.host,Replies('{"choice":"go_gallery"}'),'Walk to the gallery.',lambda _:True)
        self.assertEqual(r['status'],'accepted');self.assertEqual(self.host.call('state',{})['room'],'room:gallery')
    def test_clarification_and_bad_schema_never_execute(self):
        before=self.host.call('state',{})
        r=ui.interpret(self.host,Replies('{"choice":"clarify"}'),'Make a bridge.',lambda _:self.fail('should not confirm'))
        self.assertEqual(r['status'],'clarify');self.assertEqual(before,self.host.call('state',{}))
        with self.assertRaises(ui.HostError):ui.interpret(self.host,Replies('{"choice":"go_gallery","action":"go"}','{"choice":"unknown"}'),'Something.',lambda _:True)
        self.assertEqual(before,self.host.call('state',{}))
    def test_character_reply_has_no_action_channel(self):
        move(self.host,'gallery');move(self.host,'workshop');before=self.host.call('state',{})
        bad='{"speech":"Free oil!","tone":"warm","fact_ids":[],"memory_ids":[],"action":"buy"}'
        good='{"speech":"Welcome, Fen.","tone":"warm","fact_ids":[],"memory_ids":[]}'
        r=ui.speak(self.host,Replies(bad,good),'merchant','My name is Fen.')
        self.assertEqual(r['speech'],'Welcome, Fen.');self.assertEqual(before,self.host.call('state',{}))
        memory=self.host.call('speech_state',{'speaker':'merchant'})['memories'];self.assertEqual([m['text'] for m in memory],['My name is Fen.'])
    def test_ambiguity_asks_human_and_never_picks_first(self):
        move(self.host,'gallery');move(self.host,'workshop')
        model=Replies('{"choice":"inspect_chalk"}');model.guidance='focused'
        before=self.host.call('state',{})
        question=ui.interpret(self.host,model,'Inspect oil or chalk.',lambda _:False)
        self.assertEqual(question['status'],'clarify');self.assertEqual(len(question['candidates']),2);self.assertEqual(before,self.host.call('state',{}))
        answer=ui.interpret(self.host,model,'Inspect oil or chalk.',lambda _:False,lambda candidates:next(c['id'] for c in candidates if c['id']=='item:chalk'))
        self.assertEqual(answer['status'],'accepted');self.assertIn('chalk',answer['message'])
    def test_unavailable_owned_item_needs_no_generation(self):
        move(self.host,'gallery');move(self.host,'cistern');model=Replies();model.guidance='focused'
        before=self.host.call('state',{})
        result=ui.interpret(self.host,model,'Take the keeper seal.',lambda _:self.fail('should not confirm'))
        self.assertEqual(result['status'],'clarify');self.assertEqual(before,self.host.call('state',{}))
    def test_unaffordable_conversation_purchase_reports_shortfall_without_model(self):
        move(self.host,'gallery');move(self.host,'workshop');model=Replies();before=self.host.call('state',{})
        answer=ui.converse(self.host,model,'merchant',"I'd like to buy the tonic.")
        self.assertIn('9 coins',answer['speech']);self.assertIn('You have 5',answer['speech']);self.assertIn('4 more',answer['speech'])
        again=ui.converse(self.host,model,'merchant','yes');self.assertIn('No purchase is waiting',again['speech'])
        self.assertEqual(before,self.host.call('state',{}))
    def test_affordable_conversation_purchase_is_confirmed_only_once(self):
        move(self.host,'gallery');move(self.host,'workshop');model=Replies();before=self.host.call('state',{})
        offer=ui.converse(self.host,model,'merchant',"I'd like to buy hinge oil.")
        self.assertIn('Say yes or no',offer['speech']);self.assertEqual(before,self.host.call('state',{}))
        answer=ui.converse(self.host,model,'merchant','yes');self.assertIn('You bought hinge oil',answer['speech'])
        after=self.host.call('state',{});self.assertEqual(after['coins'],2)
        self.assertEqual(next(i for i in after['items'] if i['id']=='item:oil')['owner'],'player')
        ui.converse(self.host,model,'merchant','yes');self.assertEqual(after,self.host.call('state',{}))
    def test_decline_and_stale_confirmation_preserve_inventory(self):
        move(self.host,'gallery');move(self.host,'workshop');model=Replies()
        before=self.host.call('state',{});ui.converse(self.host,model,'merchant','buy chalk')
        ui.converse(self.host,model,'merchant','no');self.assertEqual(before,self.host.call('state',{}))
        ui.converse(self.host,model,'merchant','buy chalk')
        self.host.call('act',{'revision':before['revision'],'decision':{'action':'wait','target':None}})
        changed=self.host.call('state',{});answer=ui.converse(self.host,model,'merchant','yes')
        self.assertIn('no longer current',answer['speech']);self.assertEqual(changed,self.host.call('state',{}))
    def test_real_conversation_loop_routes_purchase_and_yes(self):
        move(self.host,'gallery');move(self.host,'workshop');before=self.host.call('state',{})
        output=io.StringIO()
        with mock.patch('builtins.input',side_effect=["I'd like to buy the tonic.",'yes','/back']),contextlib.redirect_stdout(output):
            ui.conversation(self.host,Replies(),'merchant')
        self.assertIn('4 more',output.getvalue());self.assertIn('No purchase is waiting',output.getvalue());self.assertEqual(before,self.host.call('state',{}))
    def test_generation_failure_still_consumes_call_limit(self):
        m=object.__new__(ui.LocalModel);m.model='fixture';m.context=2048;m.maximum=1;m.calls=[];m.log=None;m.records={'calls':m.calls}
        def fail(*_):raise OSError('simulated failure')
        m.data=fail
        with self.assertRaises(OSError):m.chat({'messages':[]},1,'preflight')
        self.assertEqual(m.calls[0]['status'],'failed')
        with self.assertRaises(RuntimeError):m.chat({'messages':[]},1,'preflight')
        self.assertEqual(len(m.calls),1)
    def test_overflow_preflight_never_generates(self):
        m=object.__new__(ui.LocalModel);m.identity={};m.context=2048;m.identify=lambda:{};calls=[]
        def probe(request,output,phase):calls.append(phase);return {'prompt_eval_count':2000}
        m.chat=probe
        with self.assertRaises(ValueError):m.generate({'messages':[]},128)
        self.assertEqual(calls,['preflight'])
if __name__=='__main__':unittest.main()
