#!/usr/bin/env python3
"""Offline identifier-boundary regression and Rust/Python parity check; no API calls."""
import json, re, subprocess, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def words(text):
    out=[]
    for word in re.split(r'[^\w]+',text.lower()):
        if not word or all('0'<=c<='9' for c in word):continue
        for w in [word]+([word[:-1]] if len(word)>3 and word.endswith('s') else []):
            if w not in out:out.append(w)
    return out
def parts(text):
    text=text.lower();out=[text]
    for segment in re.split(r'[:./-]',text):out.extend([segment]+segment.split('_'))
    return out
def score(row,text,legacy=False):
    tokens=[]
    for value in row.values():
        if isinstance(value,str):tokens.extend([value.lower()]+value.lower().split('_') if legacy else parts(value))
    return sum(w in tokens for w in words(text))
def fixtures():
    rows=[dict(id='grove:salt_scroll',title='CAFÉ',serial=9007199254740993),dict(id='quest.gate.open',title='門',serial=9007199254740995),dict(id='grove:café_map',title='East',serial=9007199254740997),dict(id='other:salt_scrollwork',title='Plain',serial=9007199254740999)]
    for name,text,ids in [('namespace','grove',['grove:salt_scroll','grove:café_map']),('component','scroll',['grove:salt_scroll']),('segment','salt_scroll',['grove:salt_scroll']),('dotted','gate',['quest.gate.open']),('full-dotted','quest.gate.open',['quest.gate.open']),('full-namespaced','grove:salt_scroll',['grove:salt_scroll']),('unicode','CAFÉ',['grove:salt_scroll','grove:café_map']),('unicode-id','café_map',['grove:café_map']),('no-substring','salt_scrollwork',['other:salt_scrollwork']),('unrelated','unseen',[])]:
        yield name,rows,text,ids

def main():
    old=new=0
    with tempfile.TemporaryDirectory() as tmp:
        for name,rows,text,ids in fixtures():
            for legacy in (True,False):
                scores=[score(row,text,legacy) for row in rows];best=max(scores);got=[row['id'] for row,s in zip(rows,scores) if best and s==best]
                if legacy:old+=got==ids
                else:new+=got==ids;assert got==ids,(name,got,ids)
            manifest=dict(sections=[dict(name='rows',pack='table',value=rows,relevant_to=text,relevant_fallback='none',fields=['id','serial'])])
            path=Path(tmp)/'manifest.json';path.write_text(json.dumps(manifest,ensure_ascii=False))
            output=subprocess.check_output([str(ROOT/'target/release/gamectx'),'pack',str(path),'--budget','4096'],text=True)
            packed=json.loads(output)['rows'];assert packed==dict(columns=['id','serial'] if ids else [],rows=[[r['id'],r['serial']] for r in rows if r['id'] in ids]),(name,packed)
    catalog_cases=0
    with tempfile.TemporaryDirectory() as tmp:
        for folder in sorted((ROOT/'conformance/catalog').glob('relevant-identifier-*')):
            case=json.loads((folder/'case.json').read_text());entries=json.loads((folder/'input.json').read_text());text=case['args']['relevant_to'];wanted=words(text)
            def ranking(e):
                tokens=parts(e['id'])
                for t in e['tags']:tokens.extend(parts(t.lstrip('#')))
                tokens+=[p[:-1] for p in list(tokens) if len(p)>3 and p.endswith('s')]
                return sum(w in tokens for w in wanted)
            expected=json.loads((folder/'expected.json').read_text())
            chosen=sorted(entries,key=lambda e:-ranking(e));chosen=[e for e in chosen if ranking(e)>0]
            assert expected['rows']==[[e['kind'],e['id'],e['tags']] for e in chosen]
            path=Path(tmp)/'manifest.json';path.write_text(json.dumps(dict(sections=[dict(name='catalog',pack='catalog',value=entries,**case['args'])])))
            result=json.loads(subprocess.check_output([str(ROOT/'target/release/gamectx'),'pack',str(path)],text=True))
            assert json.dumps(result['catalog'],ensure_ascii=False,separators=(',',':'))==(folder/'expected.json').read_text().strip()
            catalog_cases+=1
    result=dict(catalog_parity_cases=catalog_cases,api_spend_usd=0,cases=10,legacy_correct=old,boundary_correct=new,scope='Synthetic lexical regression only; no model-accuracy or semantic-relevance claim.')
    (ROOT/'eval/relevance-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
if __name__=='__main__':main()
