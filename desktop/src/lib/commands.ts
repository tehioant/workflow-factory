import type { SaveDefinition, SavedDefinition } from './transport.ts';
export type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
export function createCommands(invoke: Invoke) {
  return {
    list: () => invoke<SavedDefinition[]>('list_definitions'),
    load: (id: string) => invoke<SavedDefinition>('load_definition', { id }),
    save: (request: SaveDefinition) => invoke<SavedDefinition>('save_definition', { request }),
  };
}
export type Commands = ReturnType<typeof createCommands>;
