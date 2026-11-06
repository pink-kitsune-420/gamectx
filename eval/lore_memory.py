"""A separate warm post-run RSS sample, never a turn/peak-memory measurement."""
import argparse,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_knowledge_cases import host
from lore_native_eval import EXTRA
from ollama_eval import memory
import play_ai as ui

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--results',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args()
    if a.cap!=0 or a.output.exists():p.error('require --cap 0 and fresh output')
    prior=ui.strict(a.results.read_text());m=ui.LocalModel(prior['identity']['model']);assert m.identity==prior['identity']
    h=host(EXTRA[0])
    try:
        k=h.call('knowledge_prepare',{'speaker':'merchant','input':'Describe my childhood.','context':2048,'size':'3'})
        h.call('knowledge_finish',{'ticket':k['ticket'],'text':'{"answer":"unknown"}'})
        sample=memory();rss=None
        try:rss=int(next(s for s in pathlib.Path('/proc',str(h.process.pid),'status').read_text().splitlines() if s.startswith('VmRSS:')).split()[1])*1024
        except (OSError,ValueError,StopIteration):pass
        resident=[{k:v.get(k) for k in ['name','digest','size','size_vram','context_length']} for v in m.data('/api/ps')['models']]
        result={'version':1,'kind':'post_run_sample','identity':m.identity,'resident_models':resident,'runner_rss_bytes':sample['runner_rss_bytes'],
            'rust_host_rss_bytes':rss,'evaluation_process_rss_bytes':sample['game_rss_bytes'],'system_available_bytes':sample['available_bytes'],
            'system_swap_in_bytes_cumulative':sample['swap_in_bytes'],'system_swap_out_bytes_cumulative':sample['swap_out_bytes'],
            'model_calls':0,'api_spend_usd':0}
        ui.checkpoint(a.output,result);print(ui.encode({k:result[k] for k in ['kind','runner_rss_bytes','rust_host_rss_bytes','evaluation_process_rss_bytes','model_calls','api_spend_usd']}))
    finally:h.close()
if __name__=='__main__':main()
