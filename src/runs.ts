import { randomUUID } from 'node:crypto';
import { validateDefinition, resolveInputs } from './definition.ts';
import type { Definition } from './definition.ts';
import { resolveConfigurations } from './configuration.ts';
import type { CodexConfiguration } from './configuration.ts';
import type { DockerRuntime } from './runtime.ts';
export type RunStatus = 'queued' | 'running' | 'succeeded' | 'failed';
export interface BlockResult { status: 'running' | 'succeeded' | 'failed'; prompt: string; output?: string; logs: string; error?: string; }
export interface Run { version: 1; id: string; name: string; createdAt: string; definition: Definition; inputs: Record<string,string>; prompts: Record<string,string>; configurations: Record<string,CodexConfiguration>; status: RunStatus; blocks: Record<string,BlockResult>; error?: string; }
export function acceptRun(value: Definition, values: Record<string,string>, configs: CodexConfiguration[], id = randomUUID(), createdAt = new Date().toISOString()): Run {
 const definition = validateDefinition(value);
 if (definition.blocks.length !== 1) throw new Error('This increment runs exactly one AI block. Multi-block ordering is not available yet.');
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
  if ([...this.records.values()].some(r => ['queued','running'].includes(r.status))) throw new Error('A run is already active. Wait for it to finish.');
  const run = acceptRun(definition, values, configs); this.records.set(run.id, run);
  void this.execute(run); return structuredClone(run);
 }
 private async execute(run: Run) {
  run.status = 'running';
  const block = run.definition.blocks[0]!;
  const result: BlockResult = { status: 'running', prompt: run.prompts[block.id]!, logs: '' }; run.blocks[block.id] = result;
  try {
   const resource = await this.runtime.prepare(run.id);
   const completed = await this.runtime.execute(resource, result.prompt, run.configurations[block.id]!);
   Object.assign(result, completed, { status: 'succeeded' }); run.status = 'succeeded';
  } catch (error) { result.status = 'failed'; result.error = error instanceof Error ? error.message : 'Execution failed'; run.error = result.error; run.status = 'failed'; }
  finally {
   try { await this.runtime.cleanup(run.id); }
   catch { run.status = 'failed'; run.error = 'Sandbox cleanup failed. Check Docker and stop the owned resources before retrying.'; }
  }
 }
}
