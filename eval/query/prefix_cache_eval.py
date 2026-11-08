#!/usr/bin/env python3
"""Offline prefix accounting or opt-in llama.cpp loopback-only cache measurement.
Requires --cap 0. Never reads credentials. Reports missing token counts as null.
"""
import argparse
import json
import statistics
import time
import urllib.parse
import urllib.request


def prompts(turns):
    stable={'rules':'Choose legal actions from current observations. '*64,
            'tools':'react(target): pick an eligible reaction. '*64}
    return [json.dumps({**stable,'tick':i,'danger':(i%10)/10},ensure_ascii=False,separators=(',',':')) for i in range(turns)]


def common_prefix(a,b):
    for i,(left,right) in enumerate(zip(a,b)):
        if left!=right:return i
    return min(len(a),len(b))


def local_url(url):
    parsed=urllib.parse.urlsplit(url)
    if parsed.scheme!='http' or parsed.hostname not in ('localhost','127.0.0.1','::1') or parsed.username or parsed.password or parsed.query or parsed.fragment:
        raise ValueError('Only an unauthenticated HTTP loopback llama.cpp server is supported')
    if parsed.path not in ('','/'):
        raise ValueError('Supply the server origin without a path')
    return url.rstrip('/')+'/completion'


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,*args,**kwargs):
        raise ValueError('Redirects are disabled for this local benchmark')


def run(url,turns):
    endpoint=local_url(url)
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}),NoRedirect())
    observations=[]
    for enabled in (False,True):
        # Distinct warmup resets the explicit slot before each mode.
        sequence=['Benchmark reset: '+str(enabled)]+prompts(turns)
        for i,prompt in enumerate(sequence):
            payload={'prompt':prompt,'n_predict':1,'temperature':0,'seed':42,
                     'cache_prompt':enabled,'id_slot':0,'stream':False}
            request=urllib.request.Request(endpoint,data=json.dumps(payload).encode(),headers={'Content-Type':'application/json'})
            start=time.perf_counter()
            with opener.open(request,timeout=60) as response:
                body=json.loads(response.read(4_194_304))
            if i==0:continue
            timings=body.get('timings',{})
            observations.append(dict(cache_prompt=enabled,turn=i-1,wall_ms=(time.perf_counter()-start)*1000,
                                     prompt_n=timings.get('prompt_n'),cache_n=timings.get('cache_n'),
                                     prompt_ms=timings.get('prompt_ms'),predicted_n=timings.get('predicted_n')))
    summary=[]
    for enabled in (False,True):
        rows=[r for r in observations if r['cache_prompt']==enabled]
        summary.append(dict(cache_prompt=enabled,median_wall_ms=statistics.median(r['wall_ms'] for r in rows),
                            reported_cached_tokens=sum(r['cache_n'] for r in rows) if all(isinstance(r['cache_n'],int) for r in rows) else None))
    return dict(api_spend_usd=0,kind='local llama.cpp; no remote provider',summary=summary,observations=observations)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--url',help='optional running llama.cpp HTTP loopback origin')
    p.add_argument('--turns',type=int,default=12)
    p.add_argument('--cap',type=float,required=True)
    args=p.parse_args()
    if args.cap!=0 or not 2<=args.turns<=120:p.error('--cap must be 0 and --turns must be 2..120')
    if args.url:
        result=run(args.url,args.turns)
    else:
        values=[v.encode() for v in prompts(args.turns)]
        result=dict(api_spend_usd=0,turns=args.turns,full_bytes=sum(map(len,values)),
                    repeated_prefix_bytes=sum(common_prefix(a,b) for a,b in zip(values,values[1:])),
                    tokenizer_measured=False,kv_cache_measured=False)
    print(json.dumps(result,indent=2))


if __name__=='__main__':main()
