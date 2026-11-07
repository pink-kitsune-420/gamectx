// Original receipt-driven player. No duration-ratio guesses and no timer that marks
// an unplayed tail heard. UTF-16 browser indices are converted to Unicode scalars.
export const scalarOffset=(text,utf16)=>{
  if(!Number.isSafeInteger(utf16)||utf16<0||utf16>text.length)return 0;
  let units=0,count=0;
  for(const c of text){
    if(units+c.length>Math.max(0,utf16))break;
    units+=c.length;
    count++;
  }
  return count;
}
;
export const prefix=(text,through)=>Array.from(text).slice(0,through).join('');
export class VoicePlayer {
  constructor({
    post,emit=()=>{
    }
    ,synth=globalThis.speechSynthesis,Utterance=globalThis.SpeechSynthesisUtterance,audioFactory=url=>new Audio(url)}
  ) {
    Object.assign(this,{
      post,emit,synth,Utterance,audioFactory}
    );
    this.serial=Promise.resolve();
    this.current=null;
    this.generation=0;
  }
  receipt(event,through=this.current?.through||0,heard=through) {
    const id=this.current?.id;
    if(!id)return Promise.resolve();
    const payload={
      id,event,through,heard}
    ;
    this.serial=this.serial.catch(()=>{
    }
    ).then(()=>this.post('delivery',payload));
    return this.serial;
  }
  update(through) {
    const c=this.current;
    if(!c||c.closed||through<c.through)return;
    c.through=through;
    this.emit({
      id:c.id,text:c.text,through,status:'Playing'}
    );
    this.receipt('played',through).catch(()=>this.interrupt());
  }
  async interrupt() {
    const c=this.current;
    this.generation++;
    this.synth?.cancel();
    if(this.audio){
      this.audio.pause();
      this.audio.removeAttribute?.('src');
      this.audio.load?.();
      this.audio=null;
    }
    if(this.audioUrl){
      URL.revokeObjectURL(this.audioUrl);
      this.audioUrl=null;
    }
    if(!c||c.closed)return;
    c.closed=true;
    this.emit({
      id:c.id,text:c.text,through:c.through,status:'Interrupted · tail unheard'}
    );
    await this.receipt('interrupted',c.through);
  }
  async start(speech,backend,voice) {
    await this.interrupt();
    const generation=++this.generation;
    this.current={
      id:speech.id,text:speech.text,through:0,closed:false,backend,voice}
    ;
    await this.receipt('queued',0);
    if(generation!==this.generation)return;
    if(backend==='off') {
      this.emit({
        id:speech.id,text:speech.text,through:0,status:'Text only · click Read to acknowledge'}
      );
      return;
    }
    if(backend==='browser_web_speech')return this.browser(generation);
    return this.nextChunk(generation);
  }
  browser(generation) {
    const c=this.current;
    if(!this.synth||!this.Utterance) {
      this.emit({
        status:'Browser speech unavailable · text remains visible'}
      );
      return;
    }
    const base=c.through,tail=Array.from(c.text).slice(base).join('');
    const utterance=new this.Utterance(tail);
    utterance.pitch=Number(c.voice.pitch);
    utterance.rate=Number(c.voice.rate);
    const voices=this.synth.getVoices();
    utterance.voice=voices.find(v=>v.voiceURI===c.voice.voice_id||v.name===c.voice.voice_id)||voices.find(v=>v.localService)||null;
    utterance.onboundary=e=>{
      if(generation===this.generation&&!c.closed)this.update(base+scalarOffset(tail,e.charIndex));
    }
    ;
    utterance.onend=()=>{
      if(generation===this.generation&&!c.closed)this.finish();
    }
    ;
    utterance.onerror=()=>{
      if(generation===this.generation&&!c.closed)this.interrupt();
    }
    ;
    this.synth.speak(utterance);
    this.emit({
      id:c.id,text:c.text,through:0,status:'Speaking · browser boundaries'}
    );
  }
  async nextChunk(generation=this.generation) {
    if(generation!==this.generation||!this.current||this.current.closed)return;
    try {
      const r=await this.post('synthesize',{
        id:this.current.id}
      );
      if(generation!==this.generation)return;
      if(!r.result.queued){
        this.emit({
          status:'HTTP voice unavailable / cap reached · using browser voice'}
        );
        this.current.backend='browser_web_speech';
        this.browser(generation);
      }
    }
    catch {
      if(generation===this.generation){
        this.current.backend='browser_web_speech';
        this.browser(generation);
      }
    }
  }
  async chunk(speech) {
    const c=this.current,a=speech.audio;
    if(!c||c.closed||c.id!==speech.id||!a||this.audio)return;
    if(a.fallback==='browser_web_speech'){
      if(c.backend!=='browser_web_speech'){
        c.backend='browser_web_speech';
        this.browser(this.generation);
      }
      return;
    }
    if(a.start!==c.through)return;
    const generation=this.generation;
    const bytes=Uint8Array.from(atob(a.audio),x=>x.charCodeAt(0));
    const url=URL.createObjectURL(new Blob([bytes],{
      type:a.mime}
    ));
    const audio=this.audioFactory(url);
    this.audio=audio;
    this.audioUrl=url;
    // HTTP TTS supplies rate. Browser playbackRate provides best-effort pitch;
    // without time-stretch DSP this also changes duration, never receipt offsets.
    audio.preservesPitch=false;
    audio.playbackRate=Number(c.voice.pitch);
    audio.onended=async()=>{
      URL.revokeObjectURL(url);
      if(generation!==this.generation||c.closed)return;
      this.audio=null;
      this.update(a.through);
      await this.serial;
      if(a.through===Array.from(c.text).length)await this.finish();
      else await this.nextChunk(generation);
    }
    ;
    audio.onerror=()=>{
      URL.revokeObjectURL(url);
      if(generation===this.generation)this.interrupt();
    }
    ;
    try {
      await audio.play();
    }
    catch {
      URL.revokeObjectURL(url);
      await this.interrupt();
    }
  }
  async finish() {
    const c=this.current;
    if(!c||c.closed)return;
    c.closed=true;
    const through=Array.from(c.text).length;
    c.through=through;
    this.emit({
      id:c.id,text:c.text,through,status:'Complete'}
    );
    await this.receipt('finished',through);
  }
  async read() {
    if(this.current&&!this.current.closed)await this.finish();
  }
}
