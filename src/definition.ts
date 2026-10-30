export interface AIBlock {
  id: string; type: 'ai'; name: string; prompt: string;
  position: { x: number; y: number };
  configurationId?: string;
}
export interface Definition {
  version: 1; id: string; name: string; blocks: AIBlock[];
  connections: { from: string; to: string }[];
  inputs?: WorkflowInput[];
}
export interface WorkflowInput { name: string; type: 'text'; required: boolean; default?: string; }
export function validId(value: unknown): asserts value is string {
  if (typeof value !== 'string' || !/^[a-zA-Z0-9_-]{1,80}$/.test(value)) throw new Error('Invalid identifier');
}
function text(value: unknown, limit: number, required = false) {
  if (typeof value !== 'string' || value.length > limit || (required && !value.trim())) throw new Error('Invalid text property');
}
export function validateDefinition(value: unknown): Definition {
  const d = value as Definition;
  if (!d || d.version !== 1 || !Array.isArray(d.blocks) || !Array.isArray(d.connections) || d.blocks.length > 200 || d.connections.length > 1000) throw new Error('Invalid definition');
  validId(d.id);
  text(d.name, 120, true);
  const inputs = d.inputs ?? [];
  if (!Array.isArray(inputs) || inputs.length > 50) throw new Error('Maximum 50 text inputs');
  const names = new Set<string>();
  for (const input of inputs) {
    if (!input || typeof input.name !== 'string' || !/^[A-Za-z][A-Za-z0-9_]{0,79}$/.test(input.name) || names.has(input.name) || input.type !== 'text' || typeof input.required !== 'boolean') throw new Error('Invalid or duplicate input name (use letters, digits and underscores)');
    names.add(input.name);
    if (input.default !== undefined) text(input.default, 100_000);
  }
  for (const b of d.blocks) {
    validId(b.id);
    if (b.type !== 'ai') throw new Error('Only AI blocks are supported');
    if (b.configurationId !== undefined) validId(b.configurationId);
    text(b.name, 120, true); text(b.prompt, 100_000);
    for (const match of b.prompt.matchAll(/\{\{inputs\.([^{}]*)\}\}/g)) if (!names.has(match[1]!)) throw new Error(`Unknown workflow input: ${match[1]}`);
    if (!b.position || !Number.isFinite(b.position.x) || !Number.isFinite(b.position.y) || b.position.x < 0 || b.position.y < 0 || b.position.x > 3000 || b.position.y > 2000) throw new Error('Invalid canvas position');
  }
  const ids = new Set(d.blocks.map(b => b.id));
  if (ids.size !== d.blocks.length) throw new Error('Duplicate block identifier');
  const edges = new Set<string>();
  for (const edge of d.connections) {
    if (!edge || !ids.has(edge.from) || !ids.has(edge.to) || edges.has(`${edge.from}:${edge.to}`)) throw new Error('Invalid connection');
    edges.add(`${edge.from}:${edge.to}`);
  }
  const visiting = new Set<string>(), done = new Set<string>();
  function visit(id: string) {
    if (visiting.has(id)) throw new Error('Execution order contains a cycle');
    if (done.has(id)) return;
    visiting.add(id);
    for (const edge of d.connections) if (edge.from === id) visit(edge.to);
    visiting.delete(id); done.add(id);
  }
  for (const id of ids) visit(id);
  return structuredClone(d);
}
export function resolveInputs(value: Definition, supplied: Record<string, string>) {
  const definition = validateDefinition(value);
  if (!supplied || typeof supplied !== 'object' || Array.isArray(supplied)) throw new Error('Input values must be an object');
  const declarations = definition.inputs ?? [];
  for (const name of Object.keys(supplied)) if (!declarations.some(input => input.name === name)) throw new Error(`Unknown workflow input: ${name}`);
  const inputs = Object.fromEntries(declarations.map(input => {
    const value = Object.hasOwn(supplied, input.name) ? supplied[input.name] : (input.default ?? '');
    text(value, 100_000);
    if (input.required && !value!.trim()) throw new Error(`Required input: ${input.name}`);
    return [input.name, value!];
  }));
  const prompts = Object.fromEntries(definition.blocks.map(block => [block.id, block.prompt.replace(/\{\{inputs\.([^{}]*)\}\}/g, (_, name: string) => inputs[name]!)]));
  return { inputs, prompts };
}
