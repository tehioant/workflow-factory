import { test } from 'node:test';
// Test-only pinned Svelte seam; production uses no Svelte internals.
import { proxy as reactive } from 'svelte/internal/client';
import assert from 'node:assert/strict';
import { saveEditor, openEditor } from '../src/lib/editor.ts';
import { createCommands } from '../src/lib/commands.ts';
test('failed save retains the dirty working copy and revision', async () => {
  const state = { working: definition, revision: 3, dirty: true, error: '' };
  const commands = createCommands(async () => { throw { code: 'conflict', message: 'Revision changed' }; });
  const result = await saveEditor(state, commands);
  assert.deepEqual(result, { ...state, error: 'Revision changed' });
  assert.equal(state.working.name, 'Research');
});
test('open refuses dirty navigation without invoking load', async () => {
  const state = { working: definition, revision: 3, dirty: true, error: '' };
  const commands = createCommands(async () => { assert.fail('must not load'); });
  const result = await openEditor(state, 'another', commands);
  assert.equal(result.working, definition);
  assert.equal(result.dirty, true);
  assert.match(result.error, /Save or discard/);
});
test('save accepts nested reactive proxies without aliasing the invoked JSON domain', async () => {
  const source = { ...definition, isolation: 'perBlock' as const, blocks: [{ id: 'a', name: 'A', prompt: '', position: { x: 1, y: 2 }, configurationId: null }], connections: [], inputs: [] };
  let request: any;
  let invocation: unknown;
  const commands = createCommands(async <T>(command: string, args?: Record<string, unknown>) => {
    invocation = { command, args };
    request = args?.request;
    structuredClone(request);
    return { definition: request.definition, revision: 4 } as T;
  });
  const result = await saveEditor(reactive({ working: source, revision: 3, dirty: true, error: '' }), commands);
  assert.equal(result.error, '');
  assert.equal(result.dirty, false);
  assert.equal(result.revision, 4);
  assert.deepEqual(invocation, { command: 'save_definition', args: { request: { definition: source, expectedRevision: 3 } } });
  assert.deepEqual(request.definition, source);
  request.definition.blocks[0].position.x = 99;
  assert.equal(source.blocks[0].position.x, 1);
});
const definition = { schemaVersion: 1, isolation: 'perRun' as const, id: 'ecbca0a8-d457-4c42-89d4-6727261747bc', name: 'Research', blocks: [], connections: [], inputs: [] };
