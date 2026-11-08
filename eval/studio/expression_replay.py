"""Replay frozen raw campaign replies through old chunk and new novelty contracts."""
import argparse,json,pathlib,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 if a.output:assert not a.output.exists()
 def local(kind,args):
  r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=json.dumps(dict(kind=kind,args=args)),text=True,capture_output=True)
  return None if r.returncode else json.loads(r.stdout)
 rows=[]
 for record in json.loads((ROOT/'eval/studio/roleplayer-campaign-live.json').read_text())['model_records']:
  rules=record['request']['messages'][0]['content'];mode='fiction'if rules.startswith('Tell a tiny')else'personal'if rules.startswith('Answer the current')else'social';raw=json.loads(record['raw_response']['message']['content'])['speech'];lines=raw if isinstance(raw,list)else[raw]
  old=local('studio-message-chunks',dict(op='reply',args=dict(speech=lines,max_chunks=2)))
  valid=all(local('studio-expression',dict(op='validate',args=dict(mode=mode,speech=s)))for s in lines)
  result=local('studio-expression',dict(op='distinct',args=dict(speech=lines)))if valid else None
  rows.append(dict(mode=mode,raw_speech=lines,old_chunk_accepted=old is not None,new_all_sentences_valid=bool(valid),new_pruning=result))
 result=dict(source='eval/studio/roleplayer-campaign-live.json',rows=rows,real_model_calls=0,api_spend_usd=0,limits='Replays frozen outputs, not fresh model behavior. Lexical restatement pruning does not assess literary quality or truth; original raw replies remain visible.')
 if a.output:a.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
 print(json.dumps(result))
if __name__=='__main__':main()
