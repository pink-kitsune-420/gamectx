"""Player-facing factual replies and live knowledge lifecycle; fake generation, real host."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import json,pathlib,sys,tempfile,unittest
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import make_host,go,ROOT
from test_dungeon_ai import Replies
import play_ai as ui
class KnowledgeChecks(unittest.TestCase):
    def setUp(self):self.h=make_host('workshop')
    def tearDown(self):self.h.close()
    def offer(self,text,context=2048):return self.h.call('knowledge_prepare',{'speaker':'merchant','input':text,'context':context,'size':'3'})
    def finish(self,offer,id):return self.h.call('knowledge_finish',{'ticket':offer['ticket'],'text':ui.encode({'answer':id})})
    def remember(self,text):
        t=self.h.call('speech_prepare',{'speaker':'merchant','input':text,'context':2048,'guidance':'focused'})
        self.h.call('speech_finish',{'ticket':t['ticket'],'text':'{"speech":"Noted.","tone":"neutral","fact_ids":[],"memory_ids":[]}'})
    def test_missing_home_is_local_and_no_biography_is_invented(self):
        before=self.h.call('state',{});r=ui.converse(self.h,Replies(),'merchant','Where is my home?')
        self.assertEqual(r['speech'],"You haven't told me where your home is.");self.assertEqual(before,self.h.call('state',{}))
    def test_tonic_reply_is_only_host_text(self):
        r=ui.converse(self.h,Replies('{"answer":"tonic_use"}'),'merchant','What does the tonic do?')
        self.assertEqual(r['speech'],'The tonic has no implemented use. Its intended purpose has not been specified.')
        self.assertEqual(r['sources'],[{'kind':'lore','id':'tonic_use'}])
    def test_model_cannot_append_speech_or_execute(self):
        before=self.h.call('state',{});offered=self.offer('Do I own the salve?')
        with self.assertRaises(ui.HostError):self.h.call('knowledge_finish',{'ticket':offered['ticket'],'text':'{"answer":"owner_salve","speech":"Free goods!"}'})
        self.assertEqual(before,self.h.call('state',{}));self.finish(offered,'owner_salve')
        with self.assertRaises(ui.HostError):self.finish(offered,'owner_salve')
    def test_player_quote_is_exact_even_beyond_old_speech_length(self):
        original='My home is '+('x'*229);self.assertEqual(len(original),240);self.remember(original)
        r=self.finish(self.offer('Where is my home?'),'quote_1');self.assertEqual(r['speech'],'I remember you saying: "'+original+'"')
    def test_forget_revokes_prepared_quote(self):
        self.remember('My name is Fen.');p=self.offer('What is my name?')
        self.h.call('speech_forget',{'speaker':'merchant'})
        with self.assertRaises(ui.HostError):self.finish(p,'quote_1')
        self.assertEqual(ui.converse(self.h,Replies(),'merchant','What is my name?')['status'],'unknown')
    def test_world_change_revokes_prepared_price(self):
        p=self.offer('What does oil cost?');s=self.h.call('state',{})
        self.h.call('act',{'revision':s['revision'],'decision':{'action':'buy','target':'item:oil'}})
        before=self.h.call('state',{})
        with self.assertRaises(ui.HostError):self.finish(p,'price_oil')
        self.assertEqual(before,self.h.call('state',{}))
    def test_private_background_and_other_speaker_memory_are_not_exposed(self):
        self.remember('My name is Fen.');go(self.h,'gallery');go(self.h,'cistern')
        for q in ["Tell me the merchant's background.",'What is my name?']:
            r=ui.converse(self.h,Replies(),'keeper',q);self.assertEqual(r['status'],'unknown');self.assertEqual(r['sources'],[])
    def test_small_budget_does_not_replace_pending_answer(self):
        p=self.offer('What is your story?')
        with self.assertRaises(ui.HostError):self.offer('What is your story?',512)
        self.assertEqual(self.finish(p,'merchant_background')['status'],'answered')
    def test_past_disclosure_question_never_uses_npc_biography(self):
        self.remember('My name is Fen.')
        r=ui.converse(self.h,Replies(),'merchant','Did I mention sailing?')
        self.assertEqual(r['status'],'unknown');self.assertEqual(r['sources'],[])
    def test_choice_hint_can_be_overridden_without_dropping_cards(self):
        with tempfile.TemporaryDirectory() as folder:
            path=pathlib.Path(folder)/'world.json'
            entries=[{'id':'tonic_'+str(i),'subject':'item:tonic','topic':'use','text':'Tonic fact '+str(i)+'.','tags':['tonic'],'known_by':['npc:merchant'],'requires':[]} for i in range(4)]
            path.write_text(ui.encode({'version':1,'entries':entries}));h=ui.Host(ROOT/'target/debug/examples/dungeon',7,path)
            try:
                go(h,'gallery');go(h,'workshop');args={'speaker':'merchant','input':'Describe the tonic?','context':2048,'size':'1'}
                with self.assertRaises(ui.HostError):h.call('knowledge_prepare',args)
                offered=h.call('knowledge_prepare',{**args,'choices_limit':8})
                self.assertEqual(len(offered['offer']['definition']['cards']),6)
            finally:h.close()
    def test_custom_world_is_loaded_once(self):
        with tempfile.TemporaryDirectory() as folder:
            path=pathlib.Path(folder)/'world.json';world=ui.strict((ROOT/'examples/lantern-vault/world-v1.json').read_text());world['entries'][0]['text']='I repair travelers\' lamps.';path.write_text(ui.encode(world))
            h=ui.Host(ROOT/'target/debug/examples/dungeon',7,path)
            try:
                go(h,'gallery');go(h,'workshop');world['entries'][0]['text']='A changed background.';path.write_text(ui.encode(world))
                r=ui.converse(h,Replies('{"answer":"merchant_background"}'),'merchant','What is your story?')
                self.assertEqual(r['speech'],"I repair travelers' lamps.")
            finally:h.close()
if __name__=='__main__':unittest.main()
