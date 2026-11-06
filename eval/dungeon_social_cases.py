"""Independent conversation regression fixtures and Rust parity; no network."""
import argparse
import copy
import json
import pathlib
import subprocess
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import dungeon_social_reference as ref
ROOT=pathlib.Path(__file__).resolve().parents[1]

def cases():
    def op(name,**args):return {'op':name,'args':args}
    start=[op('act',decision={'action':'go','target':'room:gallery'}),op('act',decision={'action':'go','target':'room:workshop'})]
    def prepare(s='merchant',input='My name is Fen.'):return op('prepare',speaker=s,input=input)
    def reply(id='chat_1',speech='Welcome, Fen.',**extras):return op('finish',ticket=id,text=json.dumps({'speech':speech,'tone':'warm','fact_ids':[],'memory_ids':[],**extras},ensure_ascii=False))
    out=[]
    def add(n,ops):out.append((n,{'seed':7,'operations':copy.deepcopy(ops)}))
    add('remember',start+[prepare(),reply(),prepare(input='What name did I tell you?'),op('state',speaker='merchant')])
    add('foreign_speaker',start+[prepare('keeper'),op('state',speaker='keeper')])
    add('bad_fact',start+[prepare(),reply(fact_ids=['invented']),op('state',speaker='merchant')])
    add('bad_memory',start+[prepare(),reply(memory_ids=[99]),op('state',speaker='merchant')])
    add('no_actions',start+[prepare(),reply(action='buy'),op('state',speaker='merchant')])
    add('stale',start+[prepare(),op('act',decision={'action':'look','target':'room:workshop'}),reply()])
    add('single_use',start+[prepare(),reply(),reply(),op('state',speaker='merchant')])
    add('forget',start+[prepare(),reply(),prepare(input='What is my name?'),op('forget',speaker='merchant'),reply('chat_2'),op('state',speaker='merchant')])
    add('replaced_ticket',start+[prepare(),prepare(input='Do you sell oil?'),reply(),reply('chat_2')])
    add('invalid_text',start+[prepare(input='x'*241),op('state',speaker='merchant')])
    bad=op('finish',ticket='chat_1',text='{"speech":"a","speech":"b","tone":"warm","fact_ids":[],"memory_ids":[]}')
    add('duplicate_json',start+[prepare(),bad,op('state',speaker='merchant')])
    add('private_scopes',start+[prepare(),reply(),op('act',decision={'action':'go','target':'room:gallery'}),op('act',decision={'action':'go','target':'room:cistern'}),prepare('keeper','What is my name?'),op('state',speaker='keeper')])
    add('narrator_no_key_hint',[prepare('narrator','Describe this room.')])
    for i in range(30):
        ops=copy.deepcopy(start)+[prepare(input='My name is Fen_'+str(i)+'.'),reply(speech='Good to meet you.')]
        for j in range(4):ops.extend([prepare(input='Tell me about supplies '+str(j)+'.'),reply('chat_'+str(j+2),speech='My stock is on the bench.',fact_ids=['scene'])])
        ops+=[prepare(input='What name did I tell you?'),reply('chat_6',speech='You said your name earlier.',memory_ids=[1]),op('state',speaker='merchant')]
        add('generated_'+str(i),ops)
    return out

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cap',type=float,required=True);p.add_argument('--check',action='store_true');p.add_argument('--write-fixtures',action='store_true');a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    accepted=rejected=0
    for name,value in cases():
        expected=ref.replay(value);accepted+=sum(r['ok'] for r in expected['results']);rejected+=sum(not r['ok'] for r in expected['results'])
        if a.check:
            r=subprocess.run([str(ROOT/'target/debug/examples/dungeon'),'--social-replay','-'],input=ref.game.encode(value),text=True,capture_output=True)
            assert r.returncode==0,(name,r.stderr)
            assert r.stdout.rstrip('\n')==ref.game.encode(expected),(name,r.stdout[:200],ref.game.encode(expected)[:200])
        if a.write_fixtures and not name.startswith('generated_'):
            d=ROOT/'examples/dungeon/social-cases'/name;d.mkdir(parents=True,exist_ok=True)
            (d/'input.json').write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
            (d/'expected.json').write_text(ref.game.encode(expected)+'\n')
    print(json.dumps({'workflows':len(cases()),'accepted_operations':accepted,'rejected_operations':rejected,'api_spend_usd':0}))
if __name__=='__main__':main()
