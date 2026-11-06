"""Offline audit of saved dungeon agent traces against independent rules and native host."""
import argparse
import json
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import dungeon_reference as ref
import dungeon_cases as cases

def check(path):
    r=json.loads(path.read_text());assert r['complete'] and r['api_spend_usd']==0
    assert len(r['calls'])<=256 and len(r['episodes'])==len(r['seeds'])
    previous_probe=None
    for c in r['calls']:
        assert c['phase'] in ['preflight','decision']
        if c['phase']=='decision':
            assert previous_probe is not None
            expected=dict(previous_probe['payload']);expected['options']={**expected['options'],'num_predict':96}
            assert c['payload']==expected
            previous_probe=None
        else:previous_probe=None
        if 'response' not in c:
            assert c.get('status') in ['failed','reserved'];continue
        response=c['response'];assert response['chunks'][-1]==response['final']
        assert response['text']==''.join(x.get('message',{}).get('content','') for x in response['chunks'])
        assert c['payload']['model']==r['identity']['model']
        assert c['payload']['options']['num_predict']==(1 if c['phase']=='preflight' else 96)
        if c['phase']=='preflight':
            count=response['final'].get('prompt_eval_count')
            if type(count) is int and 0<=count and count+352<=2048:previous_probe=c
    turns=accepted=progress=0;rooms=set()
    for episode,seed in zip(r['episodes'],r['seeds']):
        assert episode['seed']==seed;g=ref.Game(seed);client=cases.Client(seed)
        try:
            for turn in episode['turns']:
                turns+=1;assert turn['before']==g.public();rooms.add(g.room)
                observation=client.call('observe',{'input':'Recover the glass star and escape.','mode':r['mode'],'context':2048})
                assert observation==turn['observation']
                assert 'host-only-diagnostic' not in ref.encode(observation['request'])
                for attempt in turn['attempts']:
                    native=client.request('decide',{'revision':g.revision,'text':attempt['text'],'format':'named' if r['mode']=='named' else 'pair'})
                    expected=attempt['reply'];assert isinstance(expected['id'],str) and expected['id'].isdigit()
                    assert {k:v for k,v in native.items() if k!='id'}=={k:v for k,v in expected.items() if k!='id'}
                    is_accepted=bool(native.get('ok') and native['result'].get('status')=='accepted')
                    assert is_accepted==attempt['accepted']
                    if is_accepted:
                        decision=json.loads(attempt['text'])
                        if r['mode']=='named':
                            prompt=json.loads(observation['request']['messages'][-1]['content'])
                            decision=next(n['pair'] for n in prompt['choices'] if n['name']==decision['choice'])
                        assert g.act(g.revision,decision)==native['result'];accepted+=1
                after=g.public()
                if 'after' in turn:
                    assert after==turn['after'] and client.call('state',{})==after
                    before=dict(turn['before']);after=dict(after);before.pop('revision');after.pop('revision');progress+=before!=after
            assert g.won==episode['won']
        finally:client.close()
    return {'episodes':len(r['episodes']),'turns':turns,'accepted':accepted,'state_changing_turns':progress,'distinct_rooms':len(rooms),'wins':sum(e['won'] for e in r['episodes']),'api_spend_usd':0}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cap',type=float,required=True);p.add_argument('paths',nargs='+',type=pathlib.Path);a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    for path in a.paths:print(json.dumps(check(path)))
if __name__=='__main__':main()
