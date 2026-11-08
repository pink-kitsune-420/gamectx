"""Offline size AND task recipe prototypes. All size-class settings are unmeasured hints."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import local_reference as base

def preset(a):
    base.fields(a,['size','task','name','identity'])
    task=a['task'];base.need(task in ['interpret','dialogue','narration','query'])
    raw=base.preset({k:v for k,v in a.items() if k!='task'})
    label=raw['guideline']['size_class'];index=['1b','1.5b','3b','8b','30b','70b'].index(label)
    output={'interpret':[32]*6,'dialogue':[64,96,128,160,192,192],
            'narration':[64,96,128,192,256,256],'query':[64,96,128,160,192,256]}[task][index]
    raw['profile']['budget']['output']=output
    raw['workflow']={'version':1,'task':task,'size_class':label,'measured':False,'renderer':'direct',
        'response':'choice' if task=='interpret' else 'sql' if task=='query' else 'speech',
        'exchanges_hint':[1,1,1,2,3,3][index] if task=='dialogue' else 0,
        'recall_hint':[1,1,2,3,4,6][index] if task=='dialogue' else 0,
        'choices_hint':raw['guideline']['choices_hint'],'include_quest_goal':False,
        'required_policy':'preserve_or_fail','hint_policy':'editable_not_capability_limits'}
    return raw
