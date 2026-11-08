"""Language-configured, bounded lexical reference prototype; IDs are opaque."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from conversation_reference import scope,fields,need,uint
from dungeon_slots_reference import tokens,contains

def resolve(a):
    fields(a,['entities','input','kind','scope','revision','max_age','focus','selection','pronouns'])
    scope(a['scope']);uint(a['revision'],2**32-1);uint(a['max_age'],1024)
    need(isinstance(a['input'],str) and 0<len(a['input'].encode())<=2048);words=tokens(a['input']);need(len(words)<=256)
    need(isinstance(a['entities'],list) and len(a['entities'])<=64);entities=a['entities'];ids=[];spans=[]
    if a['kind'] is not None:scope(a['kind'])
    need(isinstance(a['pronouns'],list) and len(a['pronouns'])<=16)
    for p in a['pronouns']:scope(p)
    for i,e in enumerate(entities):
        fields(e,['id','label','kind','aliases']);scope(e['id']);scope(e['label']);scope(e['kind']);need(e['id'] not in ids);ids.append(e['id'])
        need(isinstance(e['aliases'],list) and 1<=len(e['aliases'])<=8)
        for alias in e['aliases']:
            scope(alias);needle=tokens(alias);need(0<len(needle)<=32)
            for start in range(len(words)-len(needle)+1):
                if words[start:start+len(needle)]==needle:spans.append((i,start,start+len(needle)))
    for key in ['focus','selection']:
        if a[key] is not None:
            fields(a[key],['id','scope','revision']);scope(a[key]['id']);scope(a[key]['scope']);uint(a[key]['revision'],2**32-1)
    matched=[]
    for i,start,end in spans:
        if not any(x<=start and y>=end and y-x>end-start for _,x,y in spans) and i not in matched:matched.append(i)
    eligible=[i for i,e in enumerate(entities) if a['kind'] is None or e['kind']==a['kind']]
    raw=matched;matched=[i for i in matched if i in eligible];f=a['focus'];sel=a['selection']
    if sel is not None:
        need(sel['scope']==a['scope'] and sel['revision']==a['revision'] and sel['id'] in ids and ids.index(sel['id']) in eligible)
        matched=[ids.index(sel['id'])];reason='explicit_target'
    elif not matched and raw:reason='type_mismatch'
    elif not matched and any(contains(words,tokens(p)) for p in a['pronouns']):
        if f is not None and f['scope']==a['scope'] and f['revision']<=a['revision'] and a['revision']-f['revision']<=a['max_age'] and f['id'] in ids and ids.index(f['id']) in eligible:matched=[ids.index(f['id'])];reason='recent_action'
        else:reason='unresolved_reference'
    else:reason='phrase' if len(matched)==1 else 'ambiguous' if matched else 'unmatched'
    picked=eligible if reason=='unresolved_reference' else matched
    return {'reason':reason,'candidates':[{'id':entities[i]['id'],'label':entities[i]['label']} for i in picked]}
