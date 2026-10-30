import { validateDefinition } from './definition.ts';
import type { Definition, AIBlock } from './definition.ts';
import type { Run } from './runs.ts';
export function predecessors(definition: Definition, id: string) {
 const ids = new Set<string>(); const pending = [id];
 while (pending.length) {
  const current = pending.shift();
  for (const edge of definition.connections) if (edge.to === current && !ids.has(edge.from)) { ids.add(edge.from); pending.push(edge.from); }
 }
 return definition.blocks.filter(block => ids.has(block.id) && block.id !== id);
}
export function executionOrder(value: Definition) {
 const d = validateDefinition(value);
 if (!d.blocks.length) throw new Error('Add at least one AI block');
 if (d.connections.length !== d.blocks.length - 1 || d.blocks.some(b => d.connections.filter(e => e.to === b.id).length > 1 || d.connections.filter(e => e.from === b.id).length > 1)) throw new Error('Execution requires a single connected chain; forks and joins are not supported');
 const first = d.blocks.find(b => !d.connections.some(e => e.to === b.id));
 const ordered: AIBlock[] = []; let block = first;
 while (block) { ordered.push(block); const edge = d.connections.find(e => e.from === block!.id); block = d.blocks.find(b => b.id === edge?.to); }
 if (ordered.length !== d.blocks.length) throw new Error('Execution requires a single connected chain');
 for (const [index, block] of ordered.entries()) for (const match of block.prompt.matchAll(/\{\{blocks\.([^{}]*)\}\}/g)) {
  const reference = /^(.*)\.final$/.exec(match[1]!);
  if (!reference) throw new Error('Only final response output references are supported');
  if (!ordered.slice(0,index).some(b => b.id === reference[1])) throw new Error(`Output reference must name an earlier connected block: ${match[1]}`);
 }
 return ordered;
}
export function nextBlock(run: Run) {
 if (run.status !== 'running') return undefined;
 for (const block of executionOrder(run.definition)) {
  const result = Object.hasOwn(run.blocks, block.id) ? run.blocks[block.id] : undefined;
  if (!result) return block;
  if (result.status === 'failed') throw new Error(`Predecessor ${block.name} failed; no downstream block will start`);
  if (result.status === 'running') return undefined;
 }
 return undefined;
}
export function resolveBlockPrompt(run: Run, block: AIBlock) {
 return block.prompt.replace(/\{\{(inputs\.[^{}]*|blocks\.[^{}]*)\}\}/g, (_, reference: string) => {
  if (reference.startsWith('inputs.')) {
   const name = reference.slice(7); if (!Object.hasOwn(run.inputs, name)) throw new Error(`Input unavailable: ${name}`); return run.inputs[name]!;
  }
  const match = /^blocks\.(.*)\.final$/.exec(reference);
  if (!match || !predecessors(run.definition, block.id).some(b => b.id === match[1])) throw new Error(`Output reference unavailable: ${reference}`);
  const result = Object.hasOwn(run.blocks, match[1]!) ? run.blocks[match[1]!] : undefined;
  if (result?.status !== 'succeeded' || typeof result.output !== 'string') throw new Error(`Output reference unavailable: ${reference}`);
  return result.output;
 });
}
