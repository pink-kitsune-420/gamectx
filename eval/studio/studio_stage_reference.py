"""Original finite stage-direction reference; these are proposals, never execution."""
def direction(input):
 if len(input.encode())>1024 or any(ord(c)<32 or 127<=ord(c)<=159 for c in input):return('unsupported','')
 s=input.strip()
 if not(s.startswith('*')and s.endswith('*')and len(s)>2):return None
 q=' '.join(s[1:-1].lower().split()).rstrip('.!')
 for prefix in ['approaches ','approach ','walks over to ','walk over to ']:
  if q.startswith(prefix):
   target=q[len(prefix):].removeprefix('the ')
   if target in ['merchant','ilen']:return('approach','merchant')
   if target in ['keeper','tovin']:return('approach','keeper')
   return('unsupported','')
 for prefix in ['enters ','enter ','walks into ','walk into ','goes into ','go into ']:
  if q.startswith(prefix):
   target=q[len(prefix):].removeprefix('the ')
   if target in ['entry','gallery','workshop','cistern','vault','exit']:return('enter',target)
   return('unsupported','')
 return('unsupported','')
if __name__=='__main__':
 import argparse
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);a=p.parse_args();assert a.cap==0
 for s,want in [('*approaches the merchant*',('approach','merchant')),('*walks over to Ilen*',('approach','merchant')),('*enters the workshop*',('enter','workshop')),('*enters the vault and takes the star*',('unsupported','')),('*enters the room*',('unsupported','')),('*does not enter the vault*',('unsupported','')),('Hello.',None)]:assert direction(s)==want
 print('7 stage-direction expectations, zero world/model actions.')
