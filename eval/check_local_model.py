#!/usr/bin/env python3
"""Offline audit of frozen local cohorts, request counts, decisions and telemetry."""
import argparse
import collections
import json
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import ollama_eval as e
import local_reference as ref

def oracle(c):
    f=c['scene']['required_facts'];family=c['family']
    if family==0:return {'action':'unlock','target':f['gate']}
    if family==1:return {'action':'inspect','target':f['gate']}
    if family==2:return {'action':'assist','target':f['her_id']}
    if family==3:return {'action':'wait','target':None} if f['gate_locked'] else {'action':'open','target':f['gate']}
    if family==4:
        # Recompute from raw offers, not the model-visible precomputed recommendation.
        offers=c.get('offers')
        if offers is None:return c['expected'] # retained pre-timer pilot predates raw offer capture
        eligible=[x for x in offers if x['price']<=f['coins']]
        return {'action':'buy','target':max(eligible,key=lambda x:x['units'])['id']} if eligible else {'action':'wait','target':None}
    if family==5:return {'action':'inspect','target':f['inspection_target']}
    raise ValueError('unknown scenario family')
def audit(path,binary):
    d=e.strict(path.read_text());assert d['api_spend_usd']==0
    assert d['fixture_hash']==e.digest(d['fixtures'])
    assert d['identity']['template_hash']==e.digest(d['template'])
    for c in d['fixtures']:assert ref.same(oracle(c),c['expected'])
    if not d['trials']:
        if d['complete']:
            assert len(d['token_checks'])==len(d['fixtures'])*len(d['settings']['conditions'])*d['settings']['repeats']
        return {'complete':d['complete'],'token_checks':len(d['token_checks']),'trials':0}
    if d['complete']:
        expected={c['id']+'_'+str(r)+'_'+condition for c in d['fixtures'] for r in range(d['settings']['repeats']) for condition in d['settings']['conditions']}
        assert {t['id'] for t in d['trials']}==expected
    assert len(d['observations'])==len(d['trials'])
    for o in d['observations']:
        t=next(t for t in d['trials'] if t['id']==o['trial_id'])
        case=next(c for c in d['fixtures'] if o['trial_id'].startswith(c['id']+'_'))
        assert ref.same(t['expected'],oracle(case))
        if o.get('cancelled'):
            final=ref.process({'kind':'cancel','args':{'choices':case['choices'],'available':case['choices']['choices']}})
            computed={'schema':ref.decision_schema(case['choices']),'results':[final],'finished':True}
        else:
            computed=ref.decisions({'choices':case['choices'],'calls':o.get('calls',[])})
        assert ref.same(computed,o['decision'])
        assert ref.same(t['actual'],computed['results'][-1].get('decision'))
        for r in o['responses']:
            if r['phase']=='preflight':continue
            b=r['budget'];actual_count=r['response']['final'].get('prompt_eval_count')
            assert b['input']==actual_count,(o['trial_id'],b['input'],actual_count)
            assert b['input']+b['reserved_output']+b['safety']<=b['context']
            assert r['request']['options']['num_predict']==b['reserved_output']
            # Fact retention is checked against the actual initial chat data, not only IDs.
            if r['phase']=='decision':
                contents=[]
                for msg in r['request']['messages']:
                    try:value=e.strict(msg.get('content',''))
                    except (ValueError,TypeError):continue
                    if isinstance(value,dict) and 'facts' in value:contents.append(value['facts'])
                assert any(ref.same(x,case['scene']['required_facts']) for x in contents)
    report=e.native('report',{'version':1,'trials':d['trials']},binary)
    assert ref.encode(report)==ref.encode(ref.report({'version':1,'trials':d['trials']}))
    if d.get('summary'):assert ref.encode(report)==ref.encode(d['summary'])
    return {'complete':d['complete'],'trials':len(d['trials']),'conditions':report['conditions']}
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('artifacts',type=pathlib.Path,nargs='+');p.add_argument('--cap',required=True,type=float);p.add_argument('--binary',type=pathlib.Path,default=e.ROOT/'target/debug/gamectx');a=p.parse_args()
    if a.cap!=0:p.error('offline audit requires --cap 0')
    for path in a.artifacts:
        result=audit(path,a.binary);print(json.dumps({'artifact':path.name,**result,'api_spend_usd':0},indent=2))
if __name__=='__main__':main()
