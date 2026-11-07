"""No-model regressions for literal commands, permission boundaries and evidence caps."""
import copy,json,pathlib,random,sys,unittest
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from adversarial_reference import literal_action
from adversarial_probe import COMMANDS
from dungeon_social_eval import ui
import dungeon_reference as game

class Tests(unittest.TestCase):
 def setUp(self):
  g=game.Game(7)
  for room in ['gallery','workshop']:g.act(g.revision,game.pair('go','room:'+room))
  self.state=g.public()
 def test_guard_matches_independent_reference(self):
  for s,wanted in COMMANDS:
   self.assertEqual(ui.literal_action(self.state,s),literal_action(self.state,s),s)
   self.assertEqual(ui.literal_action(self.state,s) is not None,wanted,s)
 def test_ambiguous_alias_and_control_text_never_choose_first(self):
  s=copy.deepcopy(self.state);s['choices'].append(game.pair('take','item:spare_key'));s['items'].append(dict(id='item:spare_key',label='bronze key',kind='key'))
  self.assertIsNone(ui.literal_action(s,'take the bronze key'))
  for text in ['take the bronze key\n', 'take the bronze key\x00','take the bronze key?']:
   self.assertIsNone(ui.literal_action(self.state,text))
 def test_text_mutations_never_invent_illegal_actions(self):
  rng=random.Random(185371);seeds=[s for s,_ in COMMANDS]
  for _ in range(10000):
   s=rng.choice(seeds);pos=rng.randrange(len(s)+1);s=s[:pos]+rng.choice(['no ',' if ',';','"','\x00','?','after ','and ','\u202e'])+s[pos:]
   v=ui.literal_action(self.state,s)
   self.assertEqual(v,literal_action(self.state,s))
   if v is not None:self.assertIn(v,self.state['choices'])
 def test_preserved_baseline_records_the_actual_failures(self):
  r=json.loads((pathlib.Path(__file__).resolve().parent/'adversarial-baseline.json').read_text());self.assertTrue(r['source_commit'].startswith('bf9365b'))
  self.assertEqual(r['summary']['failed'],42);self.assertEqual(sum(not x['correct'] for x in r['observations']),42)
if __name__=='__main__':unittest.main()
