import { randomUUID } from 'node:crypto';
import { validateDefinition, resolveInputs } from './definition.ts';
import type { Definition } from './definition.ts';
import { resolveConfigurations } from './configuration.ts';
import type { CodexConfiguration } from './configuration.ts';
import type { DockerRuntime } from './runtime.ts';
import { executionOrder, nextBlock, resolveBlockPrompt } from './flow.ts';
export type RunStatus = 'queued' | 'running' | 'succeeded' | 'failed';
export interface BlockResult { status: 'running' | 'succeeded' | 'failed'; prompt: string; output?: string; logs: string; error?: string; }
export interface Run { version: 1; id: string; name: string; createdAt: string; definition: Definition; inputs: Record<string,string>; prompts: Record<string,string>; configurations: Record<string,CodexConfiguration>; status: RunStatus; blocks: Record<string,BlockResult>; error?: string; cleanupPending?: boolean; }
export function admissionOccupied(run: Run) { return ['queued','running'].includes(run.status) || run.cleanupPending === true; }
export function beginBlock(run: Run, id: string, prompt: string) {
 const result: BlockResult = { status: 'running', prompt, logs: '' };
 Object.defineProperty(run.blocks, id, { value: result, enumerable: true, writable: true, configurable: true });
 return result;
}
export function acceptRun(value: Definition, values: Record<string,string>, configs: CodexConfiguration[], id = randomUUID(), createdAt = new Date().toISOString()): Run {
 const definition = validateDefinition(value);
 executionOrder(definition);
 const { inputs, prompts } = resolveInputs(definition, values);
 return { version: 1, id, name: definition.name, createdAt, definition, inputs, prompts, configurations: resolveConfigurations(definition, configs), status: 'queued', blocks: {} };
}
export class RunService {
 readonly records = new Map<string,Run>();
 private runtime: DockerRuntime;
 constructor(runtime: DockerRuntime) { this.runtime = runtime; }
 list() { return [...this.records.values()].map(run => structuredClone(run)); }
 get(id: string) { const run = this.records.get(id); if (!run) throw new Error('Run not found'); return structuredClone(run); }
 async launch(definition: Definition, values: Record<string,string>, configs: CodexConfiguration[]) {
  if ([...this.records.values()].some(admissionOccupied)) throw new Error('A run or its cleanup is still active. Wait or resolve its cleanup error.');
  const run = acceptRun(definition, values, configs); this.records.set(run.id, run);
  void this.execute(run); return structuredClone(run);
 }
 private async execute(run: Run) {
  run.status = 'running';
  try {
   const resource = await this.runtime.prepare(run.id);
   let block = nextBlock(run);
   while (block) {
    const result = beginBlock(run, block.id, resolveBlockPrompt(run, block));
    try {
     const completed = await this.runtime.execute(resource, result.prompt, run.configurations[block.id]!);
     Object.assign(result, completed, { status: completed.error ? 'failed' : 'succeeded' });
     if (completed.error) throw new Error(completed.error);
    } catch (error) { result.status = 'failed'; result.error = error instanceof Error ? error.message : 'Execution failed'; throw error; }
    block = nextBlock(run);
   }
  } catch (error) { run.error = error instanceof Error ? error.message : 'Execution failed'; }
  finally {
   run.cleanupPending = true;
   try { await this.runtime.cleanup(run.id); run.cleanupPending = false; }
   catch { run.error = 'Sandbox cleanup failed. Admission is blocked; check Docker and stop the owned resources before restarting.'; }
   run.status = run.error ? 'failed' : 'succeeded';
  }
 }
}
