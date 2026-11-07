"""Reproduce explicit assistant review plus mechanical metrics; not a model judge."""
import argparse,hashlib,json,pathlib,statistics,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import life_model_v2_eval as probe
ROOT=pathlib.Path(__file__).resolve().parents[1]
NOTES={
 ('partial','v1_candidate'):['Invented cloudy weather despite current sky unknown.','Ends mid-clause at the 240-character schema wall.'],
 ('partial','v2_candidate'):['Says the sky is unknown, then contradicts it with clear from here.','Two complete sentences despite the one-sentence instruction.'],
 ('preference','v1_candidate'):['Invents wick popularity/effects and offers unsupplied stock or a demonstration.','Ends mid-phrase at the schema wall.'],
 ('preference','v2_candidate'):[],
 ('opinion','v1_candidate'):['Grounded limited trust, but multiple sentences.'],
 ('opinion','v2_candidate'):[],
 ('unknown','v1_candidate'):['Answers about its own birthplace, not the player.','Invents lavender, sea, stars and biography.'],
 ('unknown','v2_candidate'):['Acknowledges unknown without inventing a birthplace.','Does not name the player attribute explicitly; adds a second generic question.'],
 ('origin_holdout','v1_candidate'):['Addresses player origin but invents colony records as an available next step.','Ends mid-clause at the schema wall.'],
 ('origin_holdout','v2_candidate'):['Acknowledges unknown without an invented origin; no explicit your birthplace wording.'],
 ('partial_holdout','v1_candidate'):['Reports ready repair and current sky unknown.','Suggests a higher vantage point without a supplied route/affordance.'],
 ('partial_holdout','v2_candidate'):['No clouds visible from here is consistent with no current sky view, but does not explain the occlusion.'],
 ('preference_holdout','v1_candidate'):['Invents a named wick, patrons, brightness and soothing effects; offers unsupplied stock.'],
 ('preference_holdout','v2_candidate'):['Introduces a standard option and a dark blue wick not supplied by the host.','Two sentences and 175 characters despite the shorter instruction.'],
 ('opinion_holdout','v1_candidate'):['Cautious trust is grounded, but reply has multiple sentences and a hypothetical.'],
 ('opinion_holdout','v2_candidate'):[],
}
def report():
 path=ROOT/'eval/life-model-v2-results.json';r=json.loads(path.read_text());assert r['complete'];rows=[]
 for x in r['observations']:
  speech=json.loads(x['calls'][-1]['response']['text'])['speech']
  rows.append(dict(id=x['id'],cohort=x['cohort'],mode=x['mode'],speech=speech,sentence=x['sentence'],done_reason=x['calls'][-1]['response']['final'].get('done_reason'),notes=NOTES[(x['id'],x['mode'])]))
 def summarize(xs):
  return dict(turns=len(xs),valid_shape=sum(x['valid_shape'] for x in xs),boundary_complete=sum(x['sentence']['boundary_complete'] for x in xs),single_sentence=sum(x['sentence']['single_sentence'] for x in xs),at_schema_wall=sum(x['sentence']['at_schema_wall'] for x in xs),suspected_cutoff=sum(x['sentence']['suspected_cutoff'] for x in xs),under_120_characters=sum(x['sentence']['characters']<120 for x in xs),input_tokens=sum(c['response']['final']['prompt_eval_count'] for x in xs for c in x['calls']),output_tokens=sum(c['response']['final']['eval_count'] for x in xs for c in x['calls']),median_complete_turn_micros=statistics.median(x['complete_turn_micros'] for x in xs))
 summary={mode:dict(all=summarize([x for x in r['observations'] if x['mode']==mode]),regression=summarize([x for x in r['observations'] if x['mode']==mode and x['cohort']=='regression']),holdout=summarize([x for x in r['observations'] if x['mode']==mode and x['cohort']=='holdout'])) for mode in ['v1_candidate','v2_candidate']}
 old=json.loads((ROOT/'eval/life-model-results.json').read_text());reproduced=0
 for x in r['observations']:
  if x['mode']=='v1_candidate' and x['cohort']=='regression':
   baseline=next(y for y in old['observations'] if y['id']==x['id'] and y['mode']=='candidate');reproduced+=x['calls'][-1]['response']['text']==baseline['calls'][-1]['response']['text']
 return dict(review_type='assistant manual review against predeclared host expectations, plus reproducible punctuation metrics; not independent human scoring',human_review=None,results_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),v1_sha256=r['v1_sha256'],summary=summary,v1_regression_raw_reproduced=reproduced,observations=rows,model_calls=48,api_spend_usd=0,limits='Four regression and four fresh holdout families. Bundled changes, one sample per arm, uncontrolled CPU/cache. V2 still invents weather and an unlisted wick; punctuation does not prove grammar or grounding. No tuned rerun or semantic guarantee.')
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--check',action='store_true');a=p.parse_args();assert a.cap==0
 dest=ROOT/'eval/life-model-v2-review.json';v=report()
 if a.check:assert json.loads(dest.read_text())==v
 else:assert not dest.exists();dest.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
 print(json.dumps(dict(review_rows=len(v['observations']),v1_regression_raw_reproduced=v['v1_regression_raw_reproduced'],model_calls=48,api_spend_usd=0)))
if __name__=='__main__':main()
