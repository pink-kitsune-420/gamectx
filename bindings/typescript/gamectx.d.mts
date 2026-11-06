export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export type Operation = 'build' | 'query' | 'validate' | 'doctor' | 'tools' | 'features' | 'evaluate' | 'local';
export declare class GamectxClient {
  constructor(project: string, binary?: string);
  request(op: Operation, args: Json): Promise<Json>;
  close(): void;
}
