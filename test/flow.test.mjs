import { test } from 'node:test';
import assert from 'node:assert/strict';
import { executionOrder, nextBlock, resolveBlockPrompt } from '../src/flow.ts';
import { definition } from './fixtures.mjs';
import { Window } from 'happy-dom';
import { mountEditor } from '../src/public/editor.ts';
test('one successful-predecessor transition selects B while failure or running work prevents it', () => {
 const d = definition(); const run = { status: 'running', definition: d, blocks: { a: { status: 'succeeded', output: 'Answer' } } };
 assert.equal(nextBlock(run).id, 'b');
 run.blocks.a.status = 'running'; assert.equal(nextBlock(run), undefined);
 run.blocks.a.status = 'failed'; assert.throws(() => nextBlock(run), /failed/);
});
test('B consumes only selected final response and its own prompt; inserted values stay literal', () => {
 const d = { ...definition(), inputs: [{ name: 'topic', type: 'text', required: true }] }; d.blocks[1].prompt = 'Review {{blocks.a.final}} for {{inputs.topic}}';
 const run = { definition: d, inputs: { topic: '{{blocks.a.final}}' }, blocks: { a: { status: 'succeeded', output: 'A response', logs: 'private conversation' } } };
 assert.equal(resolveBlockPrompt(run, d.blocks[1]), 'Review A response for {{blocks.a.final}}');
 run.blocks.a.status = 'failed'; assert.throws(() => resolveBlockPrompt(run, d.blocks[1]), /unavailable/);
});
test('execution rejects disconnected/branching graphs and future, missing or unsupported output references', () => {
 const d = definition(); assert.deepEqual(executionOrder(d).map(b => b.id), ['a','b']);
 assert.throws(() => executionOrder({ ...d, connections: [] }), /single connected chain/);
 d.blocks[0].prompt = '{{blocks.b.final}}'; assert.throws(() => executionOrder(d), /earlier/);
 d.blocks[0].prompt = '{{blocks.missing.final}}'; assert.throws(() => executionOrder(d), /earlier/);
 d.blocks[0].prompt = '{{blocks.a.history}}'; assert.throws(() => executionOrder(d), /reference/);
});
test('B picker inserts the ancestor output token without copying its result', () => {
 const window = new Window(); const root = window.document.createElement('main'); const editor = mountEditor(root, definition(), {});
 const picker = root.querySelectorAll('[data-reference]')[1]; picker.value = 'blocks.a.final'; picker.dispatchEvent(new window.Event('change'));
 assert.match(editor.snapshot().blocks[1].prompt, /\{\{blocks\.a\.final\}\}/);
 assert.equal(root.querySelector('[data-reference] option[value="blocks.b.final"]'), null);
});
