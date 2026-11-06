"""Task/context expansion matches an independent reference; fake byte counts only."""
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import character_reference as ref
from dungeon_reference import encode
ROOT=pathlib.Path(__file__).resolve().parents[1]
def view():
 return {'revision':7,'world':{'version':1,'id':'world:test','revision':7,'states':[{'key':'time','value':{'tick':7,'label':'third bell','calendar':{'watch':3}}},{'key':'weather','value':None}]},'world_keys':['time'],'locations':{'version':1,'revision':7,'places':[{'id':'room:shop','label':'shop','summary':'A workshop.','description':'A sheltered workbench.','details':{'weather':None,'indoors':True}}]},'current':'room:shop','inventory':None,'owners':[],'sources':[{'id':'line:1','speaker':'player:a','text':'My name is Fen.'}]}
def turn():
 return {'task':'observe','input':'What changed here?','facts':{'scope':'shop:player:a','must_keep':'The character is indoors.'},'tools':[{'name':'world_status','task':'observe','priority':50,'required':True},{'name':'location_lookup','task':'observe','priority':40,'required':False},{'name':'info_lookup','task':'observe','priority':10,'required':False},{'name':'look_around','task':'other_task','priority':100,'required':False}],'memories':[{'value':{'id':i,'quote':'A relevant original player statement. '*4},'priority':30-i,'required':False} for i in range(12)],'dialogue':[{'user':'A previous question '+str(i),'assistant':'An unverified previous response '+str(i)} for i in range(4)],'attention':{'tools':1,'memories':2,'exchanges':1}}
def cases():
 out=[]
 def add(name,t,budget,error=False):out.append((name,{'kind':'character-plan','args':{'scope':'shop:player:a','actor':'npc:merchant','participants':['npc:merchant','player:a'],'view':view(),'turn':t,'budget':budget}},error))
 add('focused',turn(),{'context':2048,'output':128,'safety':256})
 add('spare_space_still_focused',turn(),{'context':32768,'output':256,'safety':256})
 t=turn();t['attention']={'tools':8,'memories':32,'exchanges':8};add('expanded',t,{'context':32768,'output':256,'safety':256})
 add('pressure',t,{'context':2048,'output':128,'safety':256})
 t=turn();t['tools'][1]['required']=True;add('too_many_required_tools',t,{'context':32768,'output':256,'safety':256},True)
 t=turn();t['memories'][0]['required']=True;t['attention']['memories']=0;add('required_memory_attention',t,{'context':32768,'output':128,'safety':256},True)
 t=turn();t['tools'][-1]['required']=True;add('cross_task_required',t,{'context':32768,'output':128,'safety':256},True)
 t=turn();t['facts']['must_keep']='x'*4096;add('required_does_not_fit',t,{'context':2048,'output':128,'safety':256},True)
 add('explicit_large_context',turn(),{'context':131072,'output':4096,'safety':1024})
 for i in range(40):
  t=turn();t['attention']={'tools':1+i%3,'memories':i%13,'exchanges':i%5};add('generated_'+str(i),t,{'context':8192,'output':128,'safety':256})
 return out
def expected(v):
 a=v['args'];t=copy.deepcopy(a['turn'])
 for tool in t['tools']:tool['schema']=ref.schema(tool['name'])
 return ref.plan({**t,'budget':a['budget']})
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,v,error in cases():
  try:wanted=expected(v);assert not error,name
  except (ValueError,TypeError,KeyError):assert error,name;wanted=None
  r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=encode(v),text=True,capture_output=True);assert bool(r.returncode)==error,(name,r.stderr)
  if not error:assert r.stdout.strip()==encode(wanted),(name,r.stdout,encode(wanted))
  if a.write_conformance and not name.startswith('generated_'):
   d=ROOT/'conformance/character'/name;d.mkdir(parents=True,exist_ok=True);(d/'case.json').write_text(encode({'op':'local',**({'error':True} if error else {})})+'\n');(d/'input.json').write_text(json.dumps(v,indent=2)+'\n')
   if not error:(d/'expected.json').write_text(encode(wanted)+'\n')
 print(encode({'workflows':len(cases()),'fixtures':sum(not n.startswith('generated_') for n,_,_ in cases()),'api_spend_usd':0}))
if __name__=='__main__':main()
