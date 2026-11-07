"""Independent persistent life/continuity fixture expectations across adapters."""
import argparse,copy,importlib.util,json,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from life_cases import base,event,enc
from life_reference import run
from continuity_cases import base as co_base
ROOT=pathlib.Path(__file__).resolve().parents[1]
def cases():
 b=base();b['scope']='counter';b['defaults'].update(tools=['life_recall']);state=run(b)['results'][-1]['result'];ops=[]
 caps=dict(facts=4,facets=2,attitudes=2,episodes=2,exchanges=1)
 def add(op,args,result=None,error=False):
  key='life_'+str(len(ops));ops.append(dict(request=dict(version=1,id=key,op='character_'+op,args=args),expected='ERROR' if error else enc(dict(version=1,id=key,ok=True,result=result))))
 def req(op,result=None,error=False,id='one',**args):add(op,dict(id=id,**args),result,error)
 cfg=dict(enabled=True,balanced=True,deception=False,mistaken=False,tools=['life_recall'])
 def configure(id):req('life_configure',dict(configured=id),id=id,setting=b['setting'],definition=b['definition'],defaults=b['defaults'],overrides={})
 req('create',dict(created='one'),scope='counter',actor='merchant',participants=b['participants']);req('create',dict(created='two'),id='two',scope='private',actor='merchant',participants=b['participants'])
 configure('one');configure('two');req('life_configure',error=True,setting=b['setting'],definition=b['definition'],defaults=b['defaults'],overrides={})
 req('life_experience',dict(revision=1),event=event());req('life_appraise',state['changes'][0],rule='returned_lamp',event=1,revision=1)
 req('life_appraise',error=True,rule='returned_lamp',event=1,revision=2)
 context=run(b|dict(operations=b['operations'][:2]+[dict(op='context',args=dict(tags=['trade'],limit=2))]))['results'][-1]['result']
 req('life_context',context,tags=['trade'],caps=caps);req('life_recall',[event()],input='lamp',tags=['trade'],count=2)
 req('life_inspect',dict(life=state,continuity=None,initiative=None,retention=True))
 foreign=run(b|dict(scope='private',operations=[dict(op='context',args=dict(tags=['trade'],limit=2))]))['results'][0]['result'];req('life_context',foreign,id='two',tags=['trade'],caps=caps)
 identity=dict(campaign='vault',branch='save:a',instance='merchant:1')
 saved=dict(version=2,interactions=dict(version=1,scope='counter',actor='merchant',participants=b['participants'],notes=dict(version=1,scope='counter',sequence=0,notes=[]),events=[],progress=[],used=[]),initiative=None,life=dict(version=1,identity=identity,scope='counter',actor='merchant',participants=b['participants'],definition=b['definition'],config=cfg,state=state,archive_ack=0,summaries=[]),continuity=None,retention=True)
 req('life_save',saved,identity=identity)
 req('life_retention',dict(retain=False),retain=False);req('life_experience',dict(revision=3),event=event(2,2));req('life_forget',dict(revision=4,forgotten=[2],invalidate=['summaries','recall','pending','checkpoints','archives']),tokens=[2],revision=3)
 fence=dict(min_revision=0,max_revision=100,min_token=0,max_token=100,max_tick=100)
 restore=dict(identity=identity,state=saved,fence=fence,min_ticket=0,min_proposal=0,min_receipt=0,min_note=0,max_tick=100)
 req('life_restore',error=True,**restore);req('life_restore',error=True,id='two',**restore)
 req('create',dict(created='three'),id='three',scope='counter',actor='merchant',participants=b['participants']);configure('three');req('life_restore',dict(restored='three'),id='three',**restore)
 req('continuity_configure',dict(configured='three'),id='three');scene=co_base()['operations'][0]['args'];scene['at']=2;req('continuity_scene',dict(bound=True),id='three',scene=scene);req('continuity_thread',dict(bound=True),id='three',thread=co_base()['operations'][1]['args'])
 req('continuity_generate',dict(id=1,stage='generated'),id='three',thread='trade',text='A new wick.',dependencies=['window']);req('continuity_validate',dict(id=1,stage='validated'),id='three',ticket=1)
 req('continuity_validate',error=True,id='three',ticket=1)
 for stage,n,recipient,played,heard in [('queued',0,None,0,[]),('played',5,None,5,[]),('heard',3,'player',5,[dict(recipient='player',through=3)])]:req('continuity_delivery',dict(id=1,stage='played' if stage=='heard' else stage,played=played,heard=heard),id='three',delivery=dict(id=1,event=stage,through=n,recipient=recipient))
 req('continuity_recognition',dict(status='interim',text='take the lamp',uncertainty='Name unclear.',actions=[]),id='three',recognition=dict(text='take the lamp',final=False,uncertainty='Name unclear.'))
 req('clear',dict(cleared='three'),id='three');req('continuity_generate',error=True,id='three',thread='trade',text='Replay?',dependencies=[])
 req('continuity_scene',dict(bound=True),id='three',scene=scene);req('continuity_thread',dict(bound=True),id='three',thread=co_base()['operations'][1]['args']);req('continuity_generate',dict(id=2,stage='generated'),id='three',thread='trade',text='Fresh.',dependencies=[])
 for ident in ['one','two','three']:req('drop',dict(dropped=ident),id=ident)
 return ops
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--cli-only',action='store_true');a=p.parse_args();assert a.cap==0
 spec=importlib.util.spec_from_file_location('bridge',ROOT/'bindings/python/gamectx_client.py');bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge)
 clients=[bridge.Client(None,ROOT/'target/debug/gamectx')]
 if not a.cli_only:clients.append(bridge.NativeClient(None,ROOT/'bindings/native/target/debug/libgamectx_native.so'))
 fixtures=cases();checks=0
 for client in clients:
  with client:
   for case in fixtures:
    request=case['request']
    try:
     result=client.request(request['op'],request['args']);assert case['expected']!='ERROR',request
     assert enc(result)==enc(json.loads(case['expected'])['result']),(request,result,case['expected'])
    except ValueError:assert case['expected']=='ERROR',request
    checks+=1
   b=base();helper=bridge.Character(client,'helper','counter','merchant',b['participants']);helper.life_configure(b['setting'],b['definition'],dict(enabled=True));assert helper.life_context(['trade'],dict(facts=4,facets=2,attitudes=2,episodes=2,exchanges=1))['revision']==0;helper.close()
 target=ROOT/'target/life-csharp';target.mkdir(parents=True,exist_ok=True)
 for f,lines in [('cases.json',[enc(fixtures)]),('requests.txt',[enc(c['request']) for c in fixtures]),('expected.txt',[c['expected'] for c in fixtures])]: (target/f).write_text('\n'.join(lines)+'\n')
 print(enc(dict(fixtures=len(fixtures),python_checks=checks,model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
