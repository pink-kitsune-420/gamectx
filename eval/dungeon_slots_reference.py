"""Example-only exact entity/verb guards, not a universal language interpreter."""
import re

def tokens(s):return re.findall(r'[^\W_]+',s.lower(),re.UNICODE)
def contains(hay,needle):return bool(needle) and any(hay[i:i+len(needle)]==needle for i in range(len(hay)-len(needle)+1))
def filter_pairs(pairs,items,input):
    words=tokens(input);targets=[]
    for p in pairs:
        t=p['target']
        if t is not None and t not in targets:targets.append(t)
    for item in items:
        if item['id'] not in targets:targets.append(item['id'])
    matched=[]
    for target in targets:
        aliases=[target.split(':')[-1]]+[item['label'] for item in items if item['id']==target]
        if any(contains(words,tokens(a)) for a in aliases):matched.append(target)
    if len(matched)!=1:return pairs
    actions=[a for a in ['go','look','inspect','wait','take','buy','unlock','talk','attack','heal'] if a in words]
    negative=any(x in words for x in ['not','don','never','without'])
    return [p for p in pairs if p['target']==matched[0] and (negative or len(actions)!=1 or p['action']==actions[0] or actions[0]=='look' and p['action']=='inspect')]

# New explicit resolver; v1 above remains as the frozen conservative-filter pilot.
def resolve(pairs,items,input,focus=None,hint=None):
    words=tokens(input);targets=[];spans=[]
    for p in pairs:
        if p['target'] is not None and p['target'] not in targets:targets.append(p['target'])
    for item in items:
        if item['id'] not in targets:targets.append(item['id'])
    for target in targets:
        aliases=[target.split(':')[-1]]+[i['label'] for i in items if i['id']==target]+[i['kind'] for i in items if i['id']==target and 'kind' in i]+(['door','gate'] if target.startswith('door:') else [])
        for alias in aliases:
            needle=tokens(alias)
            for start in range(len(words)-len(needle)+1):
                if words[start:start+len(needle)]==needle:spans.append((target,start,start+len(needle)))
    matched=[]
    for target,start,end in spans:
        shadowed=any(a<=start and b>=end and b-a>end-start for _,a,b in spans)
        if not shadowed and target not in matched:matched.append(target)
    verbs=[a for a in ['go','inspect','take','buy','unlock','talk','attack','heal'] if a in words]
    prefix={'go':'room:','take':'item:','buy':'item:','heal':'item:','unlock':'door:','talk':'npc:','attack':'npc:'}.get(verbs[0]) if len(verbs)==1 else None
    raw_matched=matched.copy()
    if prefix:matched=[m for m in matched if m.startswith(prefix)]
    pronoun=any(w in words for w in ['it','this','that','there']) or contains(words,['that','one']) or contains(words,['this','one'])
    if hint is not None:
        if hint not in targets:raise ValueError('unknown target hint')
        matched=[hint];reason='explicit_target'
    elif not matched and raw_matched:reason='type_mismatch'
    elif not matched and pronoun:
        if focus in targets:matched=[focus];reason='recent_action'
        else:reason='unresolved_reference'
    else:reason='phrase' if len(matched)==1 else 'ambiguous' if len(matched)>1 else 'unmatched'
    selected=matched if matched else [t for t in targets if prefix is None or t.startswith(prefix)] if reason=='unresolved_reference' else []
    candidates=[{'id':id,'label':next((i['label'] for i in items if i['id']==id),id.split(':')[-1].replace('_',' '))} for id in selected]
    if reason in ['ambiguous','unresolved_reference','type_mismatch']:out=[]
    elif not matched:out=pairs
    else:
        actions=[a for a in ['go','look','inspect','wait','take','buy','unlock','talk','attack','heal'] if a in words]
        negative=any(x in words for x in ['not','don','never','without'])
        out=[p for p in pairs if p['target']==matched[0] and (negative or len(actions)!=1 or p['action']==actions[0] or actions[0]=='look' and p['action']=='inspect')]
    return {'pairs':out,'reason':reason,'candidates':candidates}
