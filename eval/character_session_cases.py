"""Persistent actor isolation/perception through Python CLI/native, with shared language fixtures."""
import argparse,copy,importlib.util,json,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from character_cases import view
from character_reference import schema
from dungeon_reference import encode
from location_reference import Locations
from world_reference import World
from info_reference import Notes
ROOT=pathlib.Path(__file__).resolve().parents[1]
def cases():
 out=[];v=view();scope='shop:player:a';notes=Notes(scope);nobody=[]
 def add(op,a,wanted=None,error=False):
  ident='character_'+str(len(out));out.append({'request':{'version':1,'id':ident,'op':'character_'+op,'args':a},'expected':'ERROR' if error else encode({'version':1,'id':ident,'ok':True,'result':wanted})})
 def call(name,text,wanted=None,error=False,id='one',revision=7,cap=2048):add('call',{'id':id,'name':name,'text':text,'revision':revision,'max_bytes':cap,'allowed':[]},wanted,error)
 for id in ['one','two']:add('create',{'id':id,'scope':scope if id=='one' else 'shop:player:b','actor':'npc:merchant','participants':['npc:merchant','player:a']},{'created':id})
 add('create',{'id':'one','scope':scope,'actor':'npc:merchant','participants':['npc:merchant','player:a']},error=True)
 add('bind',{'id':'one','view':v},{'bound':'one'})
 call('look_around','{"detail":"brief"}',error=True)
 add('enable',{'id':'one','tools':['look_around','world_status','info_lookup']},[schema(k) for k in ['look_around','world_status','info_lookup']])
 for detail in ['brief','normal','full']:
  text=encode({'detail':detail});call('look_around',text,Locations(v['locations']).call(text,v['current'],7,2048,[v['current']]))
 for text in ['{}','{"detail":"full","current":"hidden"}','{"detail":"full","detail":"brief"}']:call('look_around',text,error=True)
 call('look_around','{"detail":"brief"}',revision=8,error=True)
 call('look_around','{"detail":"full"}',cap=64,error=True)
 call('look_around','{"detail":"brief"}',id='two',error=True)
 for keys in [['time'],['weather','hidden'],['time','weather']]:
  text=encode({'keys':keys});call('world_status',text,World(v['world']).call(text,7,2048,v['world_keys']))
 lookup='{"subject":"player:a","attribute":"name"}'
 call('info_lookup',lookup,notes.call('info_lookup',lookup,v['sources']))
 # Bind/enable transitions revoke old tools, and fresh sources never imply remembered claims.
 add('bind',{'id':'one','view':v},{'bound':'one'})
 call('info_lookup',lookup,error=True)
 add('enable',{'id':'one','tools':['info_lookup']},[schema('info_lookup')])
 call('info_lookup',lookup,notes.call('info_lookup',lookup,v['sources']))
 add('enable',{'id':'one','tools':['look_around','unknown_tool']},error=True)
 call('info_lookup',lookup,notes.call('info_lookup',lookup,v['sources']))
 add('clear',{'id':'one'},{'cleared':'one'})
 call('info_lookup',lookup,error=True)
 add('drop',{'id':'one'},{'dropped':'one'})
 add('clear',{'id':'one'},error=True)
 add('drop',{'id':'two'},{'dropped':'two'})
 return out

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--fixtures',type=pathlib.Path,required=True);a=p.parse_args();assert a.cap==0
 spec=importlib.util.spec_from_file_location('character_bridge',ROOT/'bindings/python/gamectx_client.py');bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge)
 lib=ROOT/'bindings/native/target/debug/libgamectx_native.so';fixtures=cases();checks=0
 for c in [bridge.Client(None,ROOT/'target/debug/gamectx'),bridge.NativeClient(None,lib)]:
  with c:
   for case in fixtures:
    req=case['request']
    try:
     got=c.request(req['op'],req['args']);assert case['expected']!='ERROR';assert encode(got)==encode(json.loads(case['expected'])['result']),req
    except ValueError:assert case['expected']=='ERROR',req
    checks+=1
   character=bridge.Character(c,'helper','scope:helper','npc:merchant',['npc:merchant','player:a']);character.bind(view());character.enable(['look_around']);assert character.call('look_around',{'detail':'brief'},7)['location']['id']=='room:shop';character.close()
 # Execute the documented Python integration literally, against the shipped view.
 sys.modules['gamectx_client']=bridge
 doc=(ROOT/'docs/CHARACTERS.md').read_text();snippet=doc.split('```python\n',1)[1].split('```',1)[0];namespace={};exec(compile(snippet,'docs/CHARACTERS.md','exec'),namespace)
 assert namespace['observed']['location']['id']=='room:workshop'
 a.fixtures.mkdir(parents=True,exist_ok=True)
 (a.fixtures/'cases.json').write_text(encode(fixtures)+'\n');(a.fixtures/'requests.txt').write_text('\n'.join(encode(c['request']) for c in fixtures)+'\n');(a.fixtures/'expected.txt').write_text('\n'.join(c['expected'] for c in fixtures)+'\n')
 print(encode({'persistent_character_python_checks':checks,'fixtures':len(fixtures),'helper_transports':2,'api_spend_usd':0}))
if __name__=='__main__':main()
