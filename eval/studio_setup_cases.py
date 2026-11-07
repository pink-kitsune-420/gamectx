"""Predeclared setup/recovery expectations; no services, models or paid calls."""
import argparse,json

def readiness(selected,installed,reachable,model,paused):
 if selected=='0b_fallback':return 'offline'
 if paused:return 'paused'
 if not reachable:return 'unavailable'
 if not model:return 'unconfigured'
 if model not in installed:return 'missing_model'
 return 'ready'

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);a=p.parse_args();assert a.cap==0
 cases=[(('0b_fallback',[],False,'',False),'offline'),(('local_ollama',['local:3b'],True,'local:3b',False),'ready'),(('local_ollama',['local:3b'],True,'absent:3b',False),'missing_model'),(('local_ollama',[],False,'local:3b',False),'unavailable'),(('local_ollama',['local:3b'],True,'',False),'unconfigured'),(('local_ollama',['local:3b'],True,'local:3b',True),'paused')]
 for args,wanted in cases:assert readiness(*args)==wanted
 before=dict(coins=2,room='workshop',model='local:3b',spend=5,assets=['approved'],saves=['A'],epoch=12,seq=90)
 reset=before|dict(coins=5,room='entry',epoch=13)
 assert all(reset[k]==before[k] for k in ['model','spend','assets','saves','seq'])
 assert reset['epoch']!=before['epoch']
 print(json.dumps(dict(expectations=len(cases)+2,actual_provider_calls=0,api_spend_usd=0,limits='Independent policy prototype. Runtime tests must prove stale-worker rejection, scoped model configuration, connection checks and actual reset behavior.')))
if __name__=='__main__':main()
