export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export type Operation = 'build' | 'query' | 'validate' | 'doctor' | 'tools' | 'features' | 'evaluate' | 'local' | 'character_create' | 'character_bind' | 'character_enable' | 'character_call' | 'character_prepare' | 'character_observe' | 'character_event' | 'character_progress' | 'character_references' | 'character_offers' | 'character_respond' | 'character_clear' | 'character_drop' | 'character_routine_configure' | 'character_routine_update' | 'character_routine_observation' | 'character_routine_complete' | 'character_routine_reserve' | 'character_routine_save' | 'character_routine_restore' | 'character_routine_context' | 'character_routine_wake' | 'character_routine_affected';
export declare class GamectxClient {
  constructor(project?: string | null, binary?: string);
  request(op: Operation, args: Json): Promise<Json>;
  close(): void;
}
export declare class Character {
  constructor(client: GamectxClient, id: string);
  static create(client: GamectxClient, id: string, scope: string, actor: string, participants: string[]): Promise<Character>;
  bind(view: Json): Promise<Json>;
  enable(tools: string[]): Promise<Json>;
  call(name: string, args: Json, revision: number, maxBytes?: number, allowed?: Json[]): Promise<Json>;
  prepare(planId: string, identity: Json, budget: Json, turn: Json): Promise<Json>;
  observe(observation: Json): Promise<Json>;
  routineConfigure(calendar: string, definition?: Json, defaults?: Json, overrides?: Json): Promise<Json>;
  routineUpdate(frame: Json): Promise<Json>;
  routineObservation(observation: Json): Promise<Json>;
  routineComplete(activity: string, start: number, revision: number, token: number): Promise<Json>;
  routineReserve(revision: number): Promise<Json>;
  routineContext(selection: Json, revision: number, maxBytes?: number): Promise<Json>;
  routineWake(): Promise<Json>;
  routineAffected(conditions?: string[], activities?: string[]): Promise<Json>;
  routineSave(): Promise<Json>;
  routineRestore(state: Json, minRevision: number, minObservation: number, minCompletion: number, minTick: number): Promise<Json>;
  event(event: Json): Promise<Json>;
  progress(progress: Json): Promise<Json>;
  references(context: Json): Promise<Json>;
  offers(offers: Json): Promise<Json>;
  respond(recipient: string, revision: number, confirm: boolean, terms: Json, allowed?: Json[]): Promise<Json>;
  clear(): Promise<Json>;
  close(): Promise<Json>;
}
