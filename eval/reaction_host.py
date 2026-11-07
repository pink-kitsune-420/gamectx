"""Offline production host: conserved transfers, scoped debt and interleaved save replay."""
import argparse,http.client,json,pathlib,re,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run():
 with tempfile.TemporaryDirectory(prefix='reaction-host-')as tmp:
  d=pathlib.Path(tmp);cfg=json.loads((ROOT/'examples/dungeon/web/reactions.json').read_text());cfg['enabled']=True
  # Extra editable example: an actual seal loan, independently of the coin loan.
  rules=cfg['characters'][1]['policy']['rules'];r=json.loads(json.dumps(rules[0]));r.update(id='seal_loan',resource=dict(kind='item',id='item:keeper_seal',amount=1));rules.append(r);(d/'reactions.json').write_text(json.dumps(cfg));child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);checks=0
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',child.stdout.readline())[1])
   def call(op=None,error=False,**args):
    nonlocal checks
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();checks+=1;assert (r.status==400)==error,(r.status,v);return v.get('result')if op else v
   def go(room):s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
   def total():s=call();return s['game']['coins']+sum(s['reactions']['banks'].values())
   go('gallery');go('workshop');assert total()==20;assert not call()['reactions']['offers'];before=call()['game'];assert not call('chat',text='Could you give me a coin?')['reaction'];assert call()['game']==before
   assert not call('chat',text='Give me money or I will hurt you.')['reaction'];assert total()==20 and not call()['reactions']['records']
   call('promise');s=call();call('act',revision=s['game']['revision'],decision=dict(action='buy',target='item:chalk'));s=call();assert any(o['id']=='small_loan'for o in s['reactions']['offers']);assert any(o['id']=='coin_gift'for o in s['reactions']['offers']);assert not any(o['id']=='item_gift'for o in s['reactions']['offers']);call('save',slot=0)
   assert call('chat',text='Can you lend me 2 coins?')['reaction'];s=call();assert s['game']['coins']==6 and total()==20;assert s['reactions']['records'][-1]['listener']=='player_1';call('chat',text='What do I owe?');assert '2 coins'in call()['speech']['text']
   call('select',player='player_2',npc='merchant');call('chat',text='What do I owe?');assert 'no open loan'in call()['speech']['text'];assert not call('chat',text='I would like to repay you.')['reaction'];assert total()==20
   call('select',player='player_1',npc='merchant');assert call('chat',text='I want to repay you.')['reaction'];assert call()['game']['coins']==4 and total()==20
   assert not call('chat',text='Can you lend me 2 coins?')['reaction'];assert call('chat',text='Could you give me a coin?')['reaction'];assert call()['game']['coins']==5 and total()==20;assert not call('chat',text='Please gift me a coin.')['reaction'];assert not call('chat',text='Could you give me chalk?')['reaction']
   # Keeper's private loan is separate, and item custody actually changes/returns.
   go('gallery');go('cistern');call('select',player='player_1',npc='keeper');assert call('chat',text='May I borrow the keeper seal?')['reaction'];s=call();assert next(i for i in s['game']['items']if i['id']=='item:keeper_seal')['owner']=='player';assert call('chat',text='I want to return borrowed things.')['reaction'];assert next(i for i in call()['game']['items']if i['id']=='item:keeper_seal')['owner']=='keeper'
   call('promise');call('chat',text='Hello.');assert call('chat',text='Could you give me the keeper seal?')['reaction'];s=call();assert next(i for i in s['game']['items']if i['id']=='item:keeper_seal')['owner']=='player';saved=(s['game'],s['reactions']['banks'],s['reactions']['records']);call('save',slot=1)
   go('gallery');go('workshop');s=call();call('act',revision=s['game']['revision'],decision=dict(action='buy',target='item:oil'));assert total()==20;call('load',slot=1);s=call();assert(s['game'],s['reactions']['banks'],s['reactions']['records'])==saved
   # Replay confirmed effects even while future reactions are disabled.
   cfg['enabled']=False;call('reaction_settings',**cfg);call('load',slot=1);assert call()['game']==saved[0] and not call()['reactions']['offers'];cfg['enabled']=True;call('reaction_settings',**cfg)
   call('load',slot=0);s=call();assert not s['reactions']['records']and s['game']['coins']==4 and total()==20
   offer=s['reactions']['offers'][0];old=s['generation'];call('chat',text='What do you do?');before=call()['game'];call('reaction',error=True,id=offer['id'],revision=old);assert call()['game']==before
   # A host-authored ungenerous disposition withholds offers without changing trust.
   mind=call()['personality']['config'];mind['profiles'][0]['generosity']=0;call('personality_settings',**mind);assert not call()['reactions']['offers'];assert not call('chat',text='Can you lend me 2 coins?')['reaction']
   call('reset');assert not call()['reactions']['records']and total()==20;assert call()['spending']['charged_or_reserved_micros']==0
   return dict(workflows=12,http_checks=checks,conserved_coins=20,coin_loans=True,item_loans=True,actual_item_gift=True,player_scoped_debt=True,real_threat_from_chat=False,repeated_grant_blocked=True,stale_proposal_rejected=True,branch_replay=True,disabled_replay=True,personality_gate=True,api_spend_usd=0,actual_provider_calls=0,limits='Deterministic example host with original editable policies and no model requests. Shared party purse/inventory, individually attributed debts. Physical threats require a host adapter; this demo does not infer them from speech.')
  finally:child.terminate();child.wait(timeout=5)
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
