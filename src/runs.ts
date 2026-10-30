import { randomUUID } from 'node:crypto';
import { validateDefinition, resolveInputs } from './definition.ts';
import type { Definition } from './definition.ts';
import { resolveConfigurations } from './configuration.ts';
import type { CodexConfiguration } from './configuration.ts';
import type { DockerRuntime, Resources } from './runtime.ts';
import { executionOrder, nextBlock, resolveBlockPrompt } from './flow.ts';
import { validateRun, interruptedRun } from './run-store.ts';
import type { RunRepository } from './run-store.ts';
import { concurrencyLimit } from './scheduler.ts';
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
 private store?: RunRepository;
 private writes = new Map<string,Promise<unknown>>();
 private slots = new Set<string>();
 private ready = new Set<string>();
 readonly limit: number;
 constructor(runtime: DockerRuntime, store?: RunRepository, limit: number = 2) { this.runtime = runtime; this.store = store; this.limit = concurrencyLimit(limit); }
 async restore() {
  if (!this.store) return;
  for (const saved of await this.store.list()) {
   const run = interruptedRun(saved); this.records.set(run.id,run);
   if (run.cleanupPending) this.slots.add(run.id);
   if (run.status !== saved.status) await this.persist(run);
  }
 }
 async persist(run: Run) {
  if (!this.store) return;
  const snapshot = validateRun(run);
  const previous = this.writes.get(run.id) ?? Promise.resolve();
  const write = previous.catch(() => {}).then(() => this.store!.save(snapshot)).catch(() => { throw new Error('Run history could not be saved. Check the private data folder and free disk space.'); });
  this.writes.set(run.id,write); await write;
 }
 async flush() { await Promise.all([...this.writes.values()]); }
 async reconcileCleanup(id: string) {
  const run = this.records.get(id);
  if (!run || !run.cleanupPending || ['queued','running'].includes(run.status)) throw new Error('Only a terminal cleanup hold may be reconciled');
  await this.runtime.reconcileCleanup(id,run.definition.blocks.map(block => block.id));
  const resolved = { ...run, cleanupPending: false };
  await this.persist(resolved); Object.assign(run,resolved); this.releaseSlot(id);
 }
 list() { return [...this.records.values()].map(run => structuredClone(run)); }
 get(id: string) { const run = this.records.get(id); if (!run) throw new Error('Run not found'); return structuredClone(run); }
 async launch(definition: Definition, values: Record<string,string>, configs: CodexConfiguration[]) {
  const run = acceptRun(definition, values, configs); this.records.set(run.id, run);
  try { await this.persist(run); }
  catch (error) { run.status = 'failed'; run.error = 'Run was not started because history could not be saved.'; throw error; }
  this.ready.add(run.id); this.pump(); return structuredClone(run);
 }
 dispatchOne() {
  if (this.slots.size >= this.limit) return false;
  const run = [...this.records.values()].find(run => run.status === 'queued' && this.ready.has(run.id));
  if (!run) return false;
  this.ready.delete(run.id); this.slots.add(run.id); run.status = 'running';
  void this.settleExecution(run,this.execute(run));
  return true;
 }
 settleExecution(run: Run, execution: Promise<void>) {
  return execution.catch(() => { run.status = 'failed'; run.cleanupPending = true; run.error = 'Terminal run history could not be saved. Cleanup was attempted; fix storage and recheck the retained cleanup/persistence hold.'; }).finally(() => { this.releaseSlot(run.id); });
 }
 private pump() { while (this.dispatchOne()) { /* slots are reserved synchronously before async execution */ } }
 releaseSlot(id: string) {
  if (this.records.get(id)?.cleanupPending || !this.slots.delete(id)) return false;
  this.pump(); return true;
 }
 async advance(run: Run, resource: Resources) {
  let result: BlockResult | undefined;
  try {
   const block = nextBlock(run); if (!block) return false;
   result = beginBlock(run, block.id, resolveBlockPrompt(run, block));
   await this.persist(run);
   const completed = await this.runtime.execute(resource, result.prompt, run.configurations[block.id]!, block.id);
   Object.assign(result, completed, { status: completed.error ? 'failed' : 'succeeded' });
   await this.persist(run);
   if (completed.error) throw new Error(completed.error);
   return true;
  } catch (error) {
   run.error = error instanceof Error ? error.message : 'Execution failed';
   if (result) { result.status = 'failed'; result.error = run.error; }
   throw error;
  }
 }
 private async execute(run: Run) {
  run.status = 'running';
  try {
   await this.persist(run);
   const resource = await this.runtime.prepare(run.id, run.definition.isolation ?? 'workflow');
   while (await this.advance(run, resource)) { /* each successful transition admits only its successor */ }
  } catch (error) { run.error = error instanceof Error ? error.message : 'Execution failed'; }
  finally {
   run.cleanupPending = true;
   try { await this.runtime.cleanup(run.id); run.cleanupPending = false; }
   catch { run.error = 'Sandbox cleanup failed. Admission is blocked; check Docker and stop the owned resources before restarting.'; }
   run.status = run.error ? 'failed' : 'succeeded';
   await this.persist(run);
  }
 }
}
