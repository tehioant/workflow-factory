import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validateRun, interruptedRun } from '../src/run-store.ts';
import { JsonStore } from '../src/store.ts';
import { RunService } from '../src/runs.ts';
import { renderRun } from '../src/public/runs.ts';
import { Window } from 'happy-dom';
import { definition } from './fixtures.mjs';
import { DockerRuntime } from '../src/runtime.ts';
const captured = () => ({ version: 1, id: 'run1', name: 'Historical', createdAt: '2026-10-30T00:00:00Z', definition: definition(), inputs: {}, prompts: { a: 'Captured A', b: 'Captured B' }, configurations: Object.fromEntries(['a','b'].map(id => [id,{version:1,id:'cfg',name:'Old config',model:'old-model',reasoning:'low'}])), status: 'succeeded', blocks: { a: { status: 'succeeded', prompt: 'Historical prompt', output: 'Retained answer', logs: 'turn.completed' } } });
test('durable read model retains captured prompts/settings/results without current definition lookups', async () => {
 const record = captured(); const disk = { async readFile() { return JSON.stringify(record); } };
 const loaded = await new JsonStore('/fake/history', validateRun, disk).get('run1');
 assert.deepEqual(loaded, record); record.definition.name = 'Changed now'; assert.equal(loaded.definition.name, 'Research');
});
test('run validation refuses credential-bearing extra fields and invalid captured settings', () => {
 assert.throws(() => validateRun({ ...captured(), credentials: { access_token: 'fake' } }), /Unsupported/);
 const record = captured(); record.configurations.a.apiKey = 'fake'; assert.throws(() => validateRun(record), /Unsupported/);
 const nested = captured(); nested.definition.credentials = 'fake'; assert.throws(() => validateRun(nested), /Unsupported/);
 const block = captured(); block.definition.blocks[0].runtime = { secret: 'fake' }; assert.throws(() => validateRun(block), /Unsupported/);
});
test('one restart projection marks interrupted work failed without resuming or losing completed outputs', () => {
 const record = captured(); record.status = 'running'; record.blocks.b = { status: 'running', prompt: 'Started B', logs: '' };
 const recovered = interruptedRun(record); assert.equal(recovered.status, 'failed'); assert.equal(recovered.blocks.b.status, 'failed');
 assert.equal(recovered.blocks.a.output, 'Retained answer'); assert.equal(record.status, 'running');
});
test('selected historical view shows captured inputs, definition, prompts and settings as text', () => {
 const window = new Window(); const root = window.document.createElement('div'); const record = captured(); record.inputs = { topic: 'Old input' };
 renderRun(root, record); assert.match(root.textContent, /Old input/); assert.match(root.textContent, /old-model/); assert.match(root.textContent, /Historical prompt/); assert.match(root.textContent, /Research/); assert.match(root.textContent, /Retained answer/);
});
test('serialized persistence captures each supplied state before later mutation', async () => {
 const writes = []; const service = new RunService({}, { async save(record) { writes.push(structuredClone(record)); } });
 const record = captured(); const first = service.persist(record); record.error = 'Later error'; await service.persist(record); await first;
 assert.equal(writes[0].error, undefined); assert.equal(writes[1].error, 'Later error'); assert.notEqual(writes[0], record);
});

test('terminal cleanup reconciliation clears the admission hold only after scoped runtime confirmation and persistence', async () => {
 const calls = []; const service = new RunService({ async reconcileCleanup(id, blocks) { calls.push([id,blocks]); } }, { async save(record) { calls.push(record.cleanupPending); } });
 const record = captured(); record.status = 'failed'; record.cleanupPending = true; service.records.set(record.id,record);
 await service.reconcileCleanup(record.id); assert.deepEqual(calls, [['run1',['a','b']],false]); assert.equal(record.cleanupPending,false);
 const refused = new RunService({ async reconcileCleanup() { throw new Error('Resources remain'); } }); const held = captured(); held.status = 'failed'; held.cleanupPending = true; refused.records.set(held.id,held);
 await assert.rejects(refused.reconcileCleanup(held.id), /Resources remain/); assert.equal(held.cleanupPending,true);
});

test('post-restart cleanup verification refuses present containers without deleting or stopping anything', async () => {
 const calls = []; const runtime = new DockerRuntime('/fake/history', async args => { calls.push(args); return args[0] === 'info' ? { code:0,stdout:'{"OSType":"linux"}' } : { code:0,stdout:'[]' }; });
 await assert.rejects(runtime.reconcileCleanup('run1',['a']), /present or cannot be verified/);
 assert.deepEqual(calls, [['info','--format','{{json .}}'],['container','inspect','wf-run1-agent']]);
});

test('historical cleanup control invokes only the selected run reconciliation boundary', async () => {
 const window = new Window(); const root = window.document.createElement('div'); const record = captured(); record.cleanupPending = true; record.status = 'failed'; const calls = [];
 renderRun(root,record,{ async recheckCleanup(id) { calls.push(id); } });
 const button = root.querySelector('button'); await button.onclick(new window.Event('click'));
 assert.deepEqual(calls,['run1']); assert.equal(button.textContent,'Cleanup verified');
});
