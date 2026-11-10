import { test } from 'node:test';
import assert from 'node:assert/strict';
import { nodeCases, rustCases } from '../scripts/test-inventory.mjs';

test('inventory counts direct named and aliased Node test registrations', () => {
  const source = "import check from 'node:test'; import { test as verify } from 'node:test'; check('save', () => {}); verify('load', async () => {});";
  assert.deepEqual(nodeCases(source, 'sample.test.ts'), ['sample.test.ts :: save', 'sample.test.ts :: load']);
});

test('inventory rejects nested test registrations instead of hiding their cases', () => {
  assert.throws(() => nodeCases("import { test } from 'node:test'; test('outer', () => { test('inner', () => {}); });", 'nested.test.ts'), /top-level/);
});

test('inventory rejects dynamic names instead of guessing the case count', () => {
  assert.throws(() => nodeCases("import { test } from 'node:test'; test(label, () => {});", 'dynamic.test.ts'), /literal/);
});

test('inventory keeps Rust cases distinct by compiled test target', () => {
  assert.deepEqual(rustCases('save: test\nload: test\n\n2 tests, 0 benchmarks\n', 'engine/store'), ['engine/store :: save', 'engine/store :: load']);
});
