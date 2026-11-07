import { test } from 'node:test';
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
const definition = { schemaVersion: 1, id: 'ecbca0a8-d457-4c42-89d4-6727261747bc', name: 'Research' };

test('save action invokes the registered command with the expected revision', async () => {
  let invocation: unknown;
  const commands = createCommands(async <T>(command: string, args?: Record<string, unknown>) => {
    invocation = { command, args };
    return { definition, revision: 4 } as T;
  });
  const result = await saveEditor({ working: definition, revision: 3, dirty: true, error: '' }, commands);
  assert.deepEqual(invocation, { command: 'save_definition', args: { request: { definition, expectedRevision: 3 } } });
  assert.equal(result.revision, 4);
  assert.equal(result.dirty, false);
});
