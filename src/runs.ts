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
export type RunStatus = 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled';
export interface BlockResult { status: 'running' | 'succeeded' | 'failed' | 'cancelled'; prompt: string; output?: string; logs: string; error?: string; }
export interface Run { version: 1; id: string; name: string; createdAt: string; definition: Definition; inputs: Record<string,string>; prompts: Record<string,string>; configurations: Record<string,CodexConfiguration>; status: RunStatus; blocks: Record<string,BlockResult>; error?: string; cleanupPending?: boolean; cancelRequested?: boolean; }
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
 private controllers = new Map<string,AbortController>();
 private tasks = new Map<string,Promise<void>>();
 private cancellations = new Map<string,Promise<Run>>();
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
  catch (error) { if (!run.cancelRequested) { run.status = 'failed'; run.error = 'Run was not started because history could not be saved.'; } throw error; }
  if (run.cancelRequested) return structuredClone(run);
  this.ready.add(run.id); this.pump(); return structuredClone(run);
 }
 cancel(id: string): Promise<Run> {
  const existing = this.cancellations.get(id); if (existing) return existing;
  const run = this.records.get(id); if (!run) return Promise.reject(new Error('Run not found'));
  if (!['queued','running'].includes(run.status)) return Promise.resolve(structuredClone(run));
  const queued = run.status === 'queued';
  run.cancelRequested = true; this.ready.delete(id); this.controllers.get(id)?.abort();
  if (queued) run.status = 'cancelled';
  const stop = queued ? Promise.resolve() : this.runtime.stop(id).catch(() => {});
  const operation = (async () => {
   let persistenceError: unknown;
   try { await this.persist(run); } catch (error) { persistenceError = error; }
   await stop;
   if (!queued) await this.tasks.get(id);
   if (run.cleanupPending) throw new Error(run.error ?? 'Cancellation cleanup or persistence is still pending');
   if (queued && persistenceError) throw persistenceError;
   return structuredClone(run);
  })().finally(() => { this.cancellations.delete(id); });
  this.cancellations.set(id,operation); return operation;
 }
 dispatchOne() {
  if (this.slots.size >= this.limit) return false;
  const run = [...this.records.values()].find(run => run.status === 'queued' && this.ready.has(run.id));
  if (!run) return false;
  this.ready.delete(run.id); this.slots.add(run.id); run.status = 'running';
  this.controllers.set(run.id,new AbortController());
  const task = this.settleExecution(run,this.execute(run)).finally(() => { this.tasks.delete(run.id); this.controllers.delete(run.id); });
  this.tasks.set(run.id,task);
  return true;
 }
 settleExecution(run: Run, execution: Promise<void>) {
  return execution.catch(() => { run.status = run.cancelRequested ? 'cancelled' : 'failed'; run.cleanupPending = true; run.error = 'Terminal run history could not be saved. Cleanup was attempted; fix storage and recheck the retained cleanup/persistence hold.'; }).finally(() => { this.releaseSlot(run.id); });
 }
 private pump() { while (this.dispatchOne()) { /* slots are reserved synchronously before async execution */ } }
 releaseSlot(id: string) {
  if (this.records.get(id)?.cleanupPending || !this.slots.delete(id)) return false;
  this.pump(); return true;
 }
 async advance(run: Run, resource: Resources) {
  if (run.cancelRequested) return false;
  const signal = this.controllers.get(run.id)?.signal;
  let result: BlockResult | undefined;
  try {
   const block = nextBlock(run); if (!block) return false;
   result = beginBlock(run, block.id, resolveBlockPrompt(run, block));
   await this.persist(run);
   signal?.throwIfAborted(); if (run.cancelRequested) throw new Error('Run cancelled');
   const completed = await this.runtime.execute(resource, result.prompt, run.configurations[block.id]!, block.id,signal);
   Object.assign(result, completed, { status: completed.error ? 'failed' : 'succeeded' });
   await this.persist(run);
   if (run.cancelRequested) return false;
   if (completed.error) throw new Error(completed.error);
   return true;
  } catch (error) {
   run.error = error instanceof Error ? error.message : 'Execution failed';
   if (result) { result.status = run.cancelRequested ? 'cancelled' : 'failed'; result.error = run.error; }
   throw error;
  }
 }
 private async execute(run: Run) {
  run.status = 'running';
  const signal = this.controllers.get(run.id)?.signal;
  try {
   await this.persist(run);
   signal?.throwIfAborted();
   const resource = await this.runtime.prepare(run.id, run.definition.isolation ?? 'workflow',signal);
   signal?.throwIfAborted();
   while (await this.advance(run, resource)) { /* each successful transition admits only its successor */ }
  } catch (error) { run.error = error instanceof Error ? error.message : 'Execution failed'; }
  finally {
   run.cleanupPending = true;
   try { await this.runtime.cleanup(run.id); run.cleanupPending = false; }
   catch { run.error = 'Sandbox cleanup failed. Admission is blocked; check Docker and stop the owned resources before restarting.'; }
   run.status = run.cancelRequested ? 'cancelled' : run.error ? 'failed' : 'succeeded';
   if (run.cancelRequested) {
    if (!run.cleanupPending) run.error = 'Run cancelled by user';
    for (const result of Object.values(run.blocks)) if (result.status === 'running') { result.status = 'cancelled'; result.error = 'Run cancelled'; }
   }
   await this.persist(run);
  }
 }
}
