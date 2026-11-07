import type { Definition } from './transport.ts';
import type { Commands } from './commands.ts';
export type Editor = { working: Definition | null; revision: number | null; dirty: boolean; error: string };
export function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error && typeof error.message === 'string') return error.message;
  return 'Desktop command failed. Your edits are retained; retry.';
}
export type GraphAction =
  | { type: 'connect'; source: string; target: string }
  | { type: 'remove'; id: string }
  | { type: 'add'; id: string }
  | { type: 'update'; id: string; patch: Partial<Pick<Definition['blocks'][number], 'name' | 'prompt' | 'position' | 'configurationId'>> };

export function editGraph(state: Editor, action: GraphAction): Editor {
  if (!state.working) return state;
  let working = state.working;
  switch (action.type) {
    case 'connect':
      working = { ...working, connections: [...working.connections, { source: action.source, target: action.target }] };
      break;
    case 'remove':
      working = {
        ...working,
        blocks: working.blocks.filter(block => block.id !== action.id),
        connections: working.connections.filter(connection => connection.source !== action.id && connection.target !== action.id),
      };
      break;
    case 'add':
      working = {
        ...working,
        blocks: [...working.blocks, {
          id: action.id, name: 'AI block', prompt: '', configurationId: null,
          position: { x: 80 + working.blocks.length * 220, y: 100 },
        }],
      };
      break;
    case 'update':
      working = { ...working, blocks: working.blocks.map(block => block.id === action.id ? { ...block, ...action.patch } : block) };
      break;
  }
  return { ...state, working, dirty: true, error: '' };
}

export const emptyEditor = (): Editor => ({ working: null, revision: null, dirty: false, error: '' });
export async function openEditor(state: Editor, id: string, commands: Commands): Promise<Editor> {
  if (state.dirty) return { ...state, error: 'Save or discard your edits before opening another definition.' };
  try {
    const saved = await commands.load(id);
    return { working: saved.definition, revision: saved.revision, dirty: false, error: '' };
  } catch (error) { return { ...state, error: errorMessage(error) }; }
}
export async function saveEditor(state: Editor, commands: Commands): Promise<Editor> {
  if (!state.working) return state;
  try {
    const saved = await commands.save({ definition: structuredClone(state.working), expectedRevision: state.revision });
    return { working: saved.definition, revision: saved.revision, dirty: false, error: '' };
  } catch (error) {
    return { ...state, error: errorMessage(error) };
  }
}
