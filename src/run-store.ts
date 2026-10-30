import { validId, validateDefinition } from './definition.ts';
import { validateConfiguration } from './configuration.ts';
import type { Run } from './runs.ts';
export interface RunRepository { list(): Promise<Run[]>; save(run: Run): Promise<Run>; }
const fields = ['version','id','name','createdAt','definition','inputs','prompts','configurations','status','blocks','error','cleanupPending','cancelRequested','persistencePending'];
function object(value: unknown): value is Record<string,unknown> { return !!value && typeof value === 'object' && !Array.isArray(value); }
function text(value: unknown, limit = 4_194_304) { if (typeof value !== 'string' || value.length > limit) throw new Error('Invalid retained text'); }
function onlyFields(value: object, allowed: string[]) { if (Object.keys(value).some(key => !allowed.includes(key))) throw new Error('Unsupported captured definition field'); }
export function validateRun(value: unknown): Run {
 const r = value as Run;
 if (!object(r) || Object.keys(r).some(key => !fields.includes(key))) throw new Error('Unsupported run field');
 if (r.version !== 1 || !['queued','running','succeeded','failed','cancelled'].includes(r.status)) throw new Error('Invalid run');
 validId(r.id); text(r.name,120); text(r.createdAt,80); if (!Number.isFinite(Date.parse(r.createdAt))) throw new Error('Invalid run date');
 const definition = validateDefinition(r.definition);
 onlyFields(definition,['version','id','name','blocks','connections','inputs','isolation']);
 for (const block of definition.blocks) { onlyFields(block,['id','type','name','prompt','position','configurationId']); onlyFields(block.position,['x','y']); }
 for (const edge of definition.connections) onlyFields(edge,['from','to']);
 for (const input of definition.inputs ?? []) onlyFields(input,['name','type','required','default']);
 for (const map of [r.inputs,r.prompts]) { if (!object(map)) throw new Error('Invalid retained values'); for (const [key,value] of Object.entries(map)) { validId(key); text(value); } }
 if (!object(r.configurations) || !object(r.blocks)) throw new Error('Invalid captured settings/results');
 for (const block of definition.blocks) if (!Object.hasOwn(r.configurations,block.id) || !Object.hasOwn(r.prompts,block.id)) throw new Error('Missing captured block settings');
 for (const [id,config] of Object.entries(r.configurations)) { if (!definition.blocks.some(b => b.id === id)) throw new Error('Unknown captured block'); validateConfiguration(config); }
 for (const [id,result] of Object.entries(r.blocks)) {
  if (!definition.blocks.some(b => b.id === id) || !object(result) || Object.keys(result).some(key => !['status','prompt','output','logs','error'].includes(key)) || !['running','succeeded','failed','cancelled'].includes(result.status)) throw new Error('Invalid retained result');
  text(result.prompt); text(result.logs); if (result.output !== undefined) text(result.output); if (result.error !== undefined) text(result.error,2000);
 }
 if (r.error !== undefined) text(r.error,2000);
 if (r.cleanupPending !== undefined && typeof r.cleanupPending !== 'boolean') throw new Error('Invalid cleanup state');
 if (r.cancelRequested !== undefined && typeof r.cancelRequested !== 'boolean') throw new Error('Invalid cancellation state');
 if (r.persistencePending !== undefined && typeof r.persistencePending !== 'boolean') throw new Error('Invalid persistence state');
 return structuredClone(r);
}
export function interruptedRun(value: Run): Run {
 const run = validateRun(value);
 if (run.status === 'queued' || run.status === 'running') {
  run.status = 'failed'; run.error = 'Interrupted by a previous app exit. This run will not resume automatically. Inspect owned Docker resources after an abnormal exit.';
  for (const result of Object.values(run.blocks)) if (result.status === 'running') { result.status = 'failed'; result.error = run.error; }
 }
 return run;
}
