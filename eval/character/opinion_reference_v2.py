"""Small subjective-speech request; no inventory/action/quest context is injected."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
from lore_reference import Lore
from dungeon_reference import encode
SUMMARIES={'entry':'The entrance stair above the lantern gallery.','gallery':'An underground gallery beside a bronze vault gate.','workshop':'An underground workshop where a merchant trades supplies.','cistern':'An underground cistern tended by its keeper.','vault':'The old vault chamber.','exit':'The daylight stair beyond the vault.'}
RULES="Reply to the current question with one short subjective reaction. You may imagine or express a preference. Use supplied facts only; invent no weather observations, personal history or actions. Outdoor weather is unknown. If speaking about the sky, explicitly say you can only imagine it from here. Keep under 160 characters. Return JSON with speech only."
def facts(g,world,speaker,input):
 actor='npc:'+speaker if speaker!='narrator' else speaker
 lore=Lore(world).select({'actor':actor,'grants':['vault_open'] if g.gate_open else [],'query':input+' background','subjects':[actor,'room:'+g.room],'required':[],'limit':2,'max_bytes':1024})['packet']
 return {'character':actor,'place':SUMMARIES[g.room],'lore':lore,'outdoor_weather':None,'outdoor_weather_observed':False}
def request(g,world,speaker,input,memories=(),history=()):
 data={'facts':facts(g,world,speaker,input),'player_quotes':list(memories),'recent_dialogue':list(history)}
 persona='You are the workshop merchant.' if speaker=='merchant' else 'You are the cistern keeper.'
 return {'messages':[{'role':'system','content':persona+' '+RULES},{'role':'user','content':'Context data:\n'+encode(data)+'\n\nCurrent request:\n'+input}], 'format':{'type':'object','properties':{'speech':{'type':'string','maxLength':160}},'required':['speech'],'additionalProperties':False}}
