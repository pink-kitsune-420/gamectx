// Per-browser playback preference. It never acknowledges text-only replies or replays history.
export class AutoSpeech {
 constructor(){this.seen=new Set();this.generation=null;}
 take(s,{enabled,host=true,busy=false}={}){
  const speech=s.speech;
  if(!speech)return false;
  const key=`${s.generation}:${speech.id}`;
  if(this.seen.has(key)||!enabled||busy||!host||speech.pending||speech.playback_blocked||speech.delivery.stage!=='validated'||speech.delivery.done||s.effective_backends.tts==='off')return false;
  this.seen.add(key);if(this.seen.size>256)this.seen.delete(this.seen.values().next().value);
  return true;
 }
}
