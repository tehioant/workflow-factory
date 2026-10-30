import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validateDefinition } from '../src/definition.ts';
import { definition } from './fixtures.mjs';
test('definition accepts typed AI properties, positions and ordered connections', () => {
  assert.deepEqual(validateDefinition(definition()), definition());
});
test('definition rejects unsupported block types and unsafe identifiers', () => {
  const invalid = definition(); invalid.blocks[0].type = 'shell';
  assert.throws(() => validateDefinition(invalid), /AI block/);
  assert.throws(() => validateDefinition({ ...definition(), id: '../escape' }), /identifier/);
});
test('definition refuses dangling, duplicate and cyclic execution order', () => {
  const d = definition(); d.connections.push({ from: 'b', to: 'a' });
  assert.throws(() => validateDefinition(d), /cycle/);
  assert.throws(() => validateDefinition({ ...definition(), connections: [{ from: 'a', to: 'missing' }] }), /connection/);
  assert.throws(() => validateDefinition({ ...definition(), blocks: [definition().blocks[0], definition().blocks[0]] }), /Duplicate/);
});
