#!/usr/bin/env python3
"""Offline integrity checks and Markdown report for the frozen alpha.3 model test."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import provenance
import argparse,collections,hashlib,json,math,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import release_model_eval as r

def metrics(rows):
    calls=[c for row in rows for c in row['calls']]
    if not rows or not all(c['usage_known'] for c in calls):raise ValueError('missing comparable token usage')
    return dict(n=len(rows),correct=sum(row['correct'] for row in rows),input=sum(c['usage']['prompt_tokens'] for c in calls)/len(rows),output=sum(c['usage']['completion_tokens'] for c in calls)/len(rows),spend=sum(c['cost_usd'] for c in calls))

def validate(data,tokens):
    if not data['completed'] or not tokens['completed'] or data['stopped'] or tokens['stopped']:raise ValueError('only completed runs may be posted')
    assert not data['tokens_only'] and tokens['tokens_only']
    assert tuple(data['models'])==r.MODELS and tuple(data['modes'])==r.MODES
    assert provenance.matches({'eval/release_model_eval.py':data['eval_sha256']})
    assert tokens['eval_sha256']==data.get('initial_eval_sha256',data['eval_sha256'])
    assert data['states']==tokens['states']
    assert data['source_commit']==tokens['source_commit']
    for run,expected in [(data,288),(tokens,36)]:
        assert len(run['rows'])==expected
        assert run['spend_usd']<=run['cap_usd']
        assert abs(run['spend_usd']-sum(c['cost_usd'] for c in run['call_ledger']))<1e-10
        assert len(run['call_ledger'])>=sum(len(row['calls']) for row in run['rows'])
        identities=[(row['model'],row['seed'],row['mode'],row['question']) for row in run['rows']]
        assert len(identities)==len(set(identities))
        for saved,(family,seed,size) in zip(run['states'],r.STATES):
            original=r.state(family,seed,size)
            assert all(saved[k]==v for k,v in original.items())
        for row in run['rows']:
            assert row['model'] in r.MODELS and row['mode'] in r.MODES
            oracle=next(q for d in run['states'] if d['seed']==row['seed'] for q in d['questions'] if q['question']==row['question'])
            assert row['expected']==oracle['expected']
            if not run['tokens_only']:assert row['correct']==r.correct(row['expected'],row['answer'])
            assert all(0<=c['cost_usd']<=c['reserved_usd'] for c in row['calls'])
    assert data['summary']==r.summary(data) and tokens['summary']==r.summary(tokens)

def report(data,tokens):
    validate(data,tokens)
    rows=data['rows'];by_mode={mode:metrics([row for row in rows if row['mode']==mode]) for mode in r.MODES}
    total=data['spend_usd']+tokens['spend_usd'];query=[row for row in rows if row['mode']=='query']
    lines=['# Alpha.3 model test — 2026-11-06','',
           'Fresh synthetic cargo, team and route states, two small hosted models, and three context/tool formats. This tests packing and querying; local brains, memory reasoning, scheduling and live gameplay remain separate evaluations.','',
           '## Completed exchanges','',
           'Table metrics include every query attempt, error correction and final-answer call within each completed exchange. The single interrupted runner call is separately charged in total spending below. Direct JSON and packed tables use one call. Query mode executes the released Rust tool, permits one syntax/validation correction, and asks the model for a final answer.','',
           '| Format | Correct / 96 | Average total input tokens | Average total output tokens | Usage-priced cost |',
           '|---|---:|---:|---:|---:|']
    names={'json':'Full JSON','packed':'Packed tables','query':'Schema + query'}
    for mode,v in by_mode.items():lines.append(f'| {names[mode]} | {v["correct"]}/96 | {v["input"]:.3f} | {v["output"]:.3f} | ${v["spend"]:.8f} |')
    lines+=['','| Model | JSON / 48 | Packed / 48 | Query / 48 |','|---|---:|---:|---:|']
    for model in r.MODELS:
        scores=[sum(row['correct'] for row in rows if row['model']==model and row['mode']==mode) for mode in r.MODES]
        lines.append(f'| {model} | '+ ' | '.join(f'{score}/48' for score in scores)+' |')
    lines+=['','| Schema family | JSON / 32 | Packed / 32 | Query / 32 |','|---|---:|---:|---:|']
    for family in ('cargo','teams','paths'):
        scores=[sum(row['correct'] for row in rows if row['family']==family and row['mode']==mode) for mode in r.MODES]
        lines.append(f'| {family} | '+' | '.join(f'{score}/32' for score in scores)+' |')
    lines+=['',f'Query correctness: {sum(row["first_correct"] for row in query)}/96 on the first attempt, {sum(row["executed_correct"] for row in query)}/96 after at most one correction, and {sum(row["correct"] for row in query)}/96 final answers. Retries: {sum(max(0,len(row["attempts"])-1) for row in query)}. Correct execution followed by a wrong final answer: {sum(row["executed_correct"] and not row["correct"] for row in query)}. Valid but semantically wrong SQL does not trigger correction.','',
            '## Token check and spending','',f'Incomplete-exchange spend included in the accuracy total: ${data["spend_usd"]-sum(c["cost_usd"] for row in data["rows"] for c in row["calls"]):.12f}. The resumed run retains its full original call ledger; its $0.12 cap is cumulative, not an additional allowance.','',
            'One-token replies were measured first on one fixed question per state/format/model (36 calls). These measure initial prompt size, not completed-exchange cost or accuracy.','',
            '| Model | JSON input | Packed input | Query input |','|---|---:|---:|---:|']
    for model in r.MODELS:
        values=[metrics([row for row in tokens['rows'] if row['model']==model and row['mode']==mode])['input'] for mode in r.MODES]
        lines.append(f'| {model} | '+' | '.join(f'{v:.3f}' for v in values)+' |')
    lines+=['',f'Token check: ${tokens["spend_usd"]:.12f} (cap $0.01). Accuracy run: ${data["spend_usd"]:.12f} (cap $0.12). Total: **${total:.12f}**. Cost uses reported prompt/completion usage and the Venice price snapshot saved with each run; provider cache discounts are not assumed. One network attempt per call, no automatic transport retries. Unknown/failed calls retain their worst-case reservation and stop the run.','',
            '## Remaining errors','',
            'Five completed query exchanges were wrong: Llama added an unrequested `LIMIT 1` to two cargo list queries; Mistral repeated unsupported subqueries for one cargo top-three total, and added an unrequested `access = \'open\'` filter to two route top-three totals. These are recorded failures, not corrected scores. The native engine cannot infer that valid SQL has the wrong intent.','',
            'Packed and full JSON have the same aggregate score, but not identical per-case outcomes: five cases were correct only with JSON and five only with packed tables.','',
            '## Frozen method and scope','',
            '- Cargo seeds 11101/11102: 24/48 rows; teams 11201/11202: 16/32 rows; routes 11301/11302: 20/40 rows. Eight questions per state, 48 per model/format, 288 scored exchanges total. All states and prompts were frozen before the token check. An adapter bug around a retriable session EOF interrupted the run after 15 completed cases; those answers were reused unchanged. One incomplete exchange was restarted, with its cost retained. No tuning or repeats selected by score.','- JSON and packed formats receive identical field meanings and all rows. Query mode receives native schema/meanings with no numeric rows, executes Rust queries locally, and includes all tool exchanges in its usage totals. Table packing is lossless; format order rotates by question number.','- Questions cover categorical/numeric filters, counts, totals, averages, extrema and top-three sums. Independent Python comprehensions provide expected answers; all 48 oracle SQL results are checked against both Rust and Python before any paid request.','- Strict JSON replies, optionally enclosed in a code fence. Duplicate keys, nonfinite values and trailing prose are rejected. Numeric tolerance is relative 1e-6; ID lists are unordered but duplicate-sensitive. Boolean values are not numeric answers.','- Temperature zero, maximum 320 completion tokens per accuracy call, only Llama 3.2 3B and Mistral Small 3.2 24B. Hosted inference can still vary; there is one completed exchange per case, no significance or broad generalization claim.','- Three schema families broaden the earlier item-only semantic test, but all are small synthetic tabular tasks with related operations. They do not measure live gameplay, long sessions, model-authored brain quality, cited-hypothesis truth or causal effects.','',
            f'Released code commit: `{data["source_commit"]}`. Evaluation script SHA-256: `{data["eval_sha256"]}`.','',
            '## Artifacts and reproduction','',
            '[Accuracy answers, queries, frozen inputs and usage](../../eval/query/release-model-results.json) · [Initial-token run](../../eval/query/release-model-tokens.json) · [Interrupted run retained for audit](../../eval/query/release-model-interrupted.json) · [Runner](../../eval/query/release_model_eval.py) · [Offline report generator](../../eval/query/summarize_release_model.py).','',
            'Offline checks (no key or API calls):','',
            '```bash','python3 -I eval/query/test_release_model.py','python3 -I eval/query/release_model_eval.py --check --cap 0 --output eval/query/release-model-tokens.json','python3 -I eval/query/release_model_eval.py --check --cap 0 --output eval/query/release-model-results.json','python3 -I eval/query/summarize_release_model.py --check','```','',
            'Paid reruns require new output paths; do not overwrite the frozen artifacts or run them in CI:','',
            '```bash','mkdir -p eval/results','python3 -I eval/query/release_model_eval.py --tokens-only --cap 0.01 --output eval/results/release-tokens-new.json','python3 -I eval/query/release_model_eval.py --cap 0.12 --output eval/results/release-accuracy-new.json','```','']
    return '\n'.join(lines),by_mode

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--check',action='store_true');args=ap.parse_args()
    data=json.loads((r.ROOT/'eval/query/release-model-results.json').read_text());tokens=json.loads((r.ROOT/'eval/query/release-model-tokens.json').read_text());text,by_mode=report(data,tokens);path=r.ROOT/'docs/evidence/MODEL_TEST_2026_11_06.md'
    if args.check:assert path.read_text()==text
    else:path.write_text(text)
    print(json.dumps(by_mode,indent=2));print('Total spend',format(data['spend_usd']+tokens['spend_usd'],'.10f'))
if __name__=='__main__':main()
