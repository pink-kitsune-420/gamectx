#!/usr/bin/env python3
"""Offline streaming aggregate prototype, execution parity and plan parity checks."""
import json, math, random, subprocess, sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import query_upgrade_eval as u
ROOT=u.r.ROOT

def streaming_aggregate(name,values):
    total=0.;count=0;best=None
    for value in values:
        if value is None:continue
        count+=1;total+=value
        if best is None or (value<best if name=='min' else value>best):best=value
    result=total if name=='sum' else None if count==0 else total/count if name=='avg' else best
    return result if result is None or math.isfinite(result) else None

def main():
    for profile,want in [('standard',''),('explicit-selection',u.GUIDANCE)]:
        reply=subprocess.check_output([str(ROOT/'target/release/gamectx'),'query','--guidance',profile],text=True)
        assert json.loads(reply)==want
    rng=random.Random(781255)
    for _ in range(1000):
        values=[rng.choice([None,-0.,0.,rng.uniform(-100,100)]) for _ in range(rng.randrange(32))]
        for name in ('sum','avg','min','max'):
            want=u.q.evaluate(('agg',name,('col','n')),None,[dict(n=v) for v in values]);got=streaming_aggregate(name,iter(values))
            assert got==want and (got!=0 or math.copysign(1,got)==math.copysign(1,want))
    inputs=[]
    selects=['id','n','COUNT(*)','SUM(n)','AVG(n)','MIN(n), MAX(n)','COUNT(n), COUNT(n*2)','SUM(n*2)+AVG(n)','n*2','id,n']
    cells=[None,False,True,'x','z',-0.,0,1,2,-3,{'a':[1]},[2,3]]
    for _ in range(2000):
        rows=[dict(id=f'zone:café.{i}',n=rng.choice(cells)) for i in range(rng.randrange(20))]
        if rng.randrange(10)==0:rows.append({} if rng.randrange(2) else 4)
        sql='SELECT '+rng.choice(selects)+' FROM cargo'+rng.choice(['',' WHERE n > 0',' WHERE n != 0',' WHERE n IN (1,2)'])+rng.choice(['',' ORDER BY n',' ORDER BY n DESC',' ORDER BY id DESC'])+rng.choice(['',' LIMIT 0',' LIMIT 1',' LIMIT 3'])
        inputs.append(dict(sql=sql,rows=rows))
    # Spellings, nonfinite arithmetic, signed zero and cancellation order.
    for values in ['[-0.0,0.0,null]','[1e308,1e308,-1e308]','[1e999,-1e999,1]','[1e16,1,-1e16]']:
        rows=[dict(n=n) for n in json.loads(values)]
        # Retain exponent spellings instead of serializing Python infinity.
        text='['+','.join('{"n":'+n+'}' for n in values[1:-1].split(','))+']'
        for select in selects[2:8]:
            inputs.append('{"sql":'+json.dumps('SELECT '+select+' FROM cargo')+',"rows":'+text+'}')
    payload='\n'.join(x if isinstance(x,str) else json.dumps(x,ensure_ascii=False) for x in inputs)+'\n'
    subprocess.run([str(ROOT/'target/query-compare')],input=payload,text=True,check=True)
    cases=[v for _,v,error in u.fixtures()]
    for _ in range(500):
        sql='SELECT '+rng.choice(selects)+' FROM cargo'+rng.choice(['',' WHERE n > -1.234567890123',' WHERE n IN (one,two)'])+rng.choice(['',' ORDER BY n DESC LIMIT 3'])
        cases.append(dict(sql=sql))
    proc=subprocess.run([str(ROOT/'target/query-plan-check')],input='\n'.join(u.dump(v) for v in cases)+'\n',text=True,capture_output=True,check=True)
    for v,actual in zip(cases,proc.stdout.splitlines(),strict=True):
        try:want=u.dump(u.process(v))
        except (ValueError,u.q.QueryError):want='ERROR'
        assert actual==want,(v,actual,want)
    print('4000 streaming aggregate checks;',len(cases),'Rust/Python plans/bounds/shape checks match')
if __name__=='__main__':main()
