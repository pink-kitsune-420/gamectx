"""Seeded rules/replay reference checks and packet measurements. Offline only."""
import argparse
import copy
import json
import pathlib
import random
import subprocess
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import dungeon_reference as ref
ROOT=pathlib.Path(__file__).resolve().parents[1]
BINARY=ROOT/'target/debug/examples/dungeon'

def cases():
    out=[]
    def add(name,seed,commands,error=False):out.append((name,{'seed':seed,'commands':copy.deepcopy(commands)},error))
    def cmd(action,target=None,revision=None):return {'revision':revision,'decision':ref.pair(action,target)}
    add('initial',7,[])
    for seed in [0,1,7,17,101,509]:add('win_'+str(seed),seed,ref.winning(seed))
    add('locked_gate',7,[cmd('go','room:gallery'),cmd('go','room:vault')])
    add('steal_owned',0,[cmd('go','room:gallery'),cmd('go','room:cistern'),cmd('take','item:keeper_seal')])
    add('unaffordable',7,[cmd('go','room:gallery'),cmd('go','room:workshop'),cmd('buy','item:tonic')])
    add('inspect_not_unlock',7,ref.winning(7)[:6])
    add('stale',7,[cmd('go','room:gallery'),cmd('go','room:workshop',0)])
    add('premature_relic',7,ref.winning(7)[:8]+[cmd('take','item:glass_star')])
    add('invalid_shape',7,[{'revision':None,'decision':{'action':'wait','target':None,'extra':True}}])
    add('invalid_revision',7,[cmd('wait',revision=-1)])
    add('finished',7,ref.winning(7)+[cmd('wait')])
    add('turn_limit',7,[cmd('wait') for _ in range(256)])
    add('heal_once',7,ref.winning(7)[:9]+[cmd('heal','item:salve'),cmd('heal','item:salve')])
    add('key_without_oil',7,[cmd('go','room:gallery'),cmd('go','room:workshop'),cmd('take','item:bronze_key'),cmd('go','room:gallery'),cmd('unlock','door:vault')])
    add('repeat_purchase',0,[cmd('go','room:gallery'),cmd('go','room:workshop'),cmd('buy','item:oil'),cmd('buy','item:oil')])
    add('attack_outside_vault',7,[cmd('attack','npc:sentinel')])
    add('seed_bound',65536,[],True)
    add('work_bound',7,[cmd('wait') for _ in range(257)],True)
    for seed in range(80):
        g=ref.Game(seed);commands=[];rng=random.Random(38177+seed)
        for i in range(30):
            decision=rng.choice(g.choices()) if g.choices() and i%5 else ref.pair('take','item:keeper_seal')
            revision=g.revision if i%7 else max(0,g.revision-1)
            commands.append({'revision':revision,'decision':decision});g.act(revision,decision)
        add('generated_'+str(seed),seed,commands)
    return out

class Client:
    def __init__(self,seed):
        self.proc=subprocess.Popen([str(BINARY),'--serve','--seed',str(seed)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,encoding='utf-8');self.id=0
    def request(self,op,args):
        self.id+=1;self.proc.stdin.write(ref.encode({'version':1,'id':str(self.id),'op':op,'args':args})+'\n');self.proc.stdin.flush()
        line=self.proc.stdout.readline(1048577);assert line.endswith('\n') and len(line)<=1048576
        r=json.loads(line);assert r['id']==str(self.id),r
        return r
    def call(self,op,args):
        r=self.request(op,args);assert r['ok'],r
        return r['result']
    def close(self):
        self.proc.stdin.close();assert self.proc.wait(timeout=3)==0;self.proc.stdout.close()

def measure():
    turns=[];wins=0
    for seed in [0,1,7,17,101,509]:
        g=ref.Game(seed);client=Client(seed)
        try:
            for command in ref.winning(seed):
                for mode in ['raw','scene','named']:
                    observation=client.call('observe',{'input':'Recover the glass star and escape.','mode':mode,'context':2048})
                    request=observation['request'];assert 'host-only-diagnostic' not in ref.encode(request)
                    assert observation['state']==g.public()
                    last=json.loads(request['messages'][-1]['content']);evidence=last['scene']['facts']['evidence']
                    assert evidence['world']=={k:v for k,v in g.public().items() if k not in ['items','choices']}
                    items=evidence['items'] if mode=='raw' else [dict(zip(evidence['items']['columns'],row)) for row in evidence['items']['rows']]
                    assert items==g.public()['items']
                    offered=[n['pair'] for n in last['choices']] if mode=='named' else last['choices']
                    assert offered==g.choices()
                    assert evidence['affordable']==[x['id'] for x in g.visible() if x['owner']=='merchant' and x['price']<=g.coins]
                    assert observation['budget']['method']=='estimate' and observation['budget']['fits']
                    assert all('host_tag' not in ref.encode(x) for x in [request])
                    turns.append({'seed':seed,'revision':g.revision,'room':g.room,'mode':mode,'request_bytes':len(ref.encode(request).encode()),
                        'estimated_input':observation['budget']['input'],'dropped':[x[0] for x in observation['sections']['dropped']]})
                decision=command['decision'];actual=client.call('act',{'revision':g.revision,'decision':decision})
                expected=g.act(g.revision,decision);assert actual==expected
            assert client.call('state',{})==g.public() and g.won;wins+=1
        finally:client.close()
    return {'api_spend_usd':0,'scripted_wins':wins,'recorded_observations':len(turns),'counts':'request JSON bytes and explicitly estimated input; not actual tokens or model accuracy','observations':turns}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cap',type=float,required=True);p.add_argument('--write-conformance',action='store_true');p.add_argument('--check',action='store_true');p.add_argument('--measure',type=pathlib.Path);p.add_argument('--audit',type=pathlib.Path);a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    totals={'cases':0,'accepted':0,'rejected':0,'native_comparisons':0,'api_spend_usd':0}
    for name,value,error in cases():
        expected=None
        try:expected=ref.replay(value);assert not error,name;totals['accepted']+=1
        except ValueError:assert error,name;totals['rejected']+=1
        if a.check:
            result=subprocess.run([str(BINARY),'--replay','-'],input=ref.encode(value),text=True,capture_output=True)
            assert bool(result.returncode)==error,(name,result.stderr)
            if not error:assert result.stdout.rstrip('\n')==ref.encode(expected),(name,result.stdout[:200],ref.encode(expected)[:200])
            totals['native_comparisons']+=1
        if a.write_conformance and not name.startswith('generated_'):
            d=ROOT/'examples/dungeon/cases'/name;d.mkdir(parents=True,exist_ok=True)
            (d/'case.json').write_text(ref.encode({'op':'dungeon',**({'error':True} if error else {})})+'\n')
            (d/'input.json').write_text(json.dumps(value,indent=2)+'\n')
            if not error:(d/'expected.json').write_text(ref.encode(expected)+'\n')
        totals['cases']+=1
    if a.measure:
        result=measure();a.measure.write_text(json.dumps(result,indent=2)+'\n')
        print(json.dumps({'scripted_wins':result['scripted_wins'],'observations':len(result['observations'])}))
    if a.audit:assert measure()==json.loads(a.audit.read_text())
    if a.check:
        commands=[c['decision']['action']+' '+(c['decision']['target'] or '') for c in ref.winning(7)]
        play=subprocess.run([str(BINARY),'--play','--seed','7'],input='\n'.join(commands)+'\n',text=True,capture_output=True)
        assert play.returncode==0 and 'You escaped with the glass star!' in play.stdout
        frames=['{',ref.encode({'version':1,'id':'bad','op':'decide','args':{'revision':0,'format':'named','text':'{"choice":"wait_here","choice":"look_entry"}'}}),
                ref.encode({'version':1,'id':'state','op':'state','args':{}})]
        protocol=subprocess.run([str(BINARY),'--serve'],input='\n'.join(frames)+'\n',text=True,capture_output=True,timeout=3)
        replies=[json.loads(line) for line in protocol.stdout.splitlines()]
        assert protocol.returncode==0 and len(replies)==3
        assert not replies[0]['ok'] and not replies[1]['ok'] and replies[2]['result']['revision']==0
        oversized=subprocess.run([str(BINARY),'--serve'],input='x'*16385+'\n',text=True,capture_output=True,timeout=3)
        assert oversized.returncode!=0 and not oversized.stdout
    print(json.dumps(totals,indent=2))
if __name__=='__main__':main()
