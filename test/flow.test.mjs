import { test } from 'node:test';
import assert from 'node:assert/strict';
import { executionOrder, nextBlock, resolveBlockPrompt } from '../src/flow.ts';
import { definition } from './fixtures.mjs';
import { Window } from 'happy-dom';
import { mountEditor } from '../src/public/editor.ts';
import { RunService } from '../src/runs.ts';
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
 const third = { ...d.blocks[1], id: 'c', name: 'C' };
 assert.throws(() => executionOrder({ ...d, blocks: [...d.blocks, third], connections: [{ from: 'a', to: 'b' }, { from: 'a', to: 'c' }] }), /single connected chain/);
 assert.throws(() => executionOrder({ ...d, blocks: [...d.blocks, third], connections: [{ from: 'a', to: 'c' }, { from: 'b', to: 'c' }] }), /single connected chain/);
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

test('actual continuation invokes only B from a seeded A-completed record using the supplied run resource', async () => {
 const d = definition(); d.blocks[1].prompt = 'B reviews {{blocks.a.final}}'; const resource = { id: 'private-run-resource' }; const calls = [];
 const service = new RunService({ async execute(...args) { calls.push(args); return { output: 'B final', logs: 'turn.completed' }; } });
 const config = { model: 'captured-model' };
 const run = { status: 'running', definition: d, inputs: {}, configurations: { b: config }, blocks: { a: { status: 'succeeded', output: 'A final' } } };
 assert.equal(await service.advance(run, resource), true);
 assert.deepEqual(calls, [[resource, 'B reviews A final', config, 'b', undefined]]); assert.equal(run.blocks.b.output, 'B final'); assert.equal(run.blocks.b.status, 'succeeded');
});

test('actual continuation records a predecessor failure and never invokes any downstream block or retry', async () => {
 let executions = 0; const service = new RunService({ async execute() { executions++; } });
 const run = { status: 'running', definition: definition(), blocks: { a: { status: 'failed', error: 'A failed' } } };
 await assert.rejects(service.advance(run, {}), /Predecessor.*failed/);
 assert.match(run.error, /failed/); assert.equal(executions, 0); assert.equal(Object.hasOwn(run.blocks, 'b'), false);
});
