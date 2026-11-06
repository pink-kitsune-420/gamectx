// Node.js built-ins only. Requests are serial; reuse one bridge process per project.
import { spawn } from 'node:child_process';
import { Buffer } from 'node:buffer';

export class GamectxClient {
  constructor(project = null, binary = 'gamectx') {
    this.child = spawn(binary, project === null ? ['characters'] : ['serve', project], { stdio: ['pipe', 'pipe', 'inherit'] });
    this.next = 0;
    this.pending = null;
    this.buffer = Buffer.alloc(0);
    this.failure = null;
    this.queue = Promise.resolve();
    this.child.on('error', error => this.fail(error));
    this.child.stdin.on('error', error => this.fail(error));
    this.child.on('exit', () => this.fail(new Error('gamectx bridge ended')));
    this.child.stdout.on('data', chunk => {
      this.buffer = Buffer.concat([this.buffer, chunk]);
      if (this.buffer.length > 16 * 1024 * 1024) {
        this.fail(new Error('gamectx reply exceeds limit'));
        this.child.kill();
        return;
      }
      let end;
      while ((end = this.buffer.indexOf(10)) >= 0) {
        const line = this.buffer.subarray(0, end).toString('utf8');
        this.buffer = this.buffer.subarray(end + 1);
        const active = this.pending;
        this.pending = null;
        if (!active) {
          this.fail(new Error('unexpected gamectx reply'));
          return;
        }
        try {
          const reply = JSON.parse(line);
          if (reply.version !== 1 || reply.id !== active.id) throw new Error('invalid gamectx protocol reply');
          if (!reply.ok) throw new Error(JSON.stringify(reply.error));
          active.resolve(reply.result);
        } catch (error) { active.reject(error); }
      }
    });
  }
  fail(error) {
    this.failure ??= error;
    if (this.pending) this.pending.reject(this.failure);
    this.pending = null;
  }
  request(op, args) {
    const call = this.queue.then(() => {
      if (this.failure) throw this.failure;
      const id = String(++this.next);
      const body = JSON.stringify({ version: 1, id, op, args }, (key, value) => {
        if (typeof value === 'number' && !Number.isFinite(value)) throw new Error('nonfinite JSON number');
        return value;
      });
      if (Buffer.byteLength(body) > 1572864) throw new Error('gamectx request exceeds limit');
      return new Promise((resolve, reject) => {
        this.pending = { id, resolve, reject };
        this.child.stdin.write(body + '\n');
      });
    });
    this.queue = call.catch(() => {});
    return call;
  }
  close() {
    this.fail(new Error('gamectx client closed'));
    this.child.stdin.end();
    this.child.kill();
  }
}

// Host-owned actor/audience binding; only call() arguments originate in the model.
export class Character {
  constructor(client, id) { this.client = client; this.id = id; }
  static async create(client, id, scope, actor, participants) {
    const character = new Character(client, id);
    await character.request('create', { scope, actor, participants });
    return character;
  }
  request(operation, args = {}) {
    return this.client.request('character_' + operation, { ...args, id: this.id });
  }
  bind(view) { return this.request('bind', { view }); }
  enable(tools) { return this.request('enable', { tools }); }
  call(name, args, revision, maxBytes = 2048, allowed = []) {
    const text = JSON.stringify(args, (key, value) => {
      if (typeof value === 'number' && !Number.isFinite(value)) throw new Error('nonfinite JSON number');
      return value;
    });
    return this.request('call', { name, text, revision, max_bytes: maxBytes, allowed });
  }
  prepare(planId, identity, budget, turn) {
    return this.request('prepare', { plan_id: planId, identity, budget, turn });
  }
  observe(observation) { return this.request('observe', { observation }); }
  event(event) { return this.request('event', { event }); }
  progress(progress) { return this.request('progress', { progress }); }
  references(context) { return this.request('references', { context }); }
  offers(offers) { return this.request('offers', { offers }); }
  respond(recipient, revision, confirm, terms, allowed = []) {
    return this.request('respond', { recipient, revision, confirm, terms, allowed });
  }
  clear() { return this.request('clear'); }
  close() { return this.request('drop'); }
}
