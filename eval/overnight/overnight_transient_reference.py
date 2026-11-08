"""Independent transient-record retention policy, declared before Rust implementation."""
def trim(events,keep,protected=()):
 assert type(keep)is int and 0<=keep<=128
 tokens={e['token']:e for e in events}
 chosen={e['token'] for e in events if e['retain']}|set(protected)
 def closure(xs):
  xs=set(xs)
  while True:
   before=set(xs)
   for e in events:
    if e['token'] in xs or xs&set(e['causes']):xs|={e['token']}|set(e['causes'])&tokens.keys()
   if before==xs:return xs
 chosen=closure(chosen)&tokens.keys();assert len(chosen)<=keep
 for e in reversed(events):
  group=closure([e['token']])
  if len(chosen|group)<=keep:chosen|=group
 return [e.copy() for e in events if e['token'] in chosen]
def trace(turns=1000):
 active=[];roots=0;peak=0
 for n in range(1,turns+1):
  if len(active)>=128:active=trim(active,64)
  active.append(dict(token=n,retain=False,causes=[]));peak=max(peak,len(active))
 assert peak==128 and len(active)==104 and roots==0
 return dict(turns=turns,active_events=len(active),peak_events=peak,rumor_roots=roots)
if __name__=='__main__':print(trace())
