import type { Definition } from './transport.ts';
import type { Commands } from './commands.ts';
export type Editor = { working: Definition | null; revision: number | null; dirty: boolean; error: string };
export function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error && typeof error.message === 'string') return error.message;
  return 'Desktop command failed. Your edits are retained; retry.';
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
