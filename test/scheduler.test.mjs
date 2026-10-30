import { test } from 'node:test';
import assert from 'node:assert/strict';
import { concurrencyLimit } from '../src/scheduler.ts';
import { RunService, acceptRun } from '../src/runs.ts';
import { definition } from './fixtures.mjs';
const queued = id => ({ id, status: 'queued', blocks: {} });
function scheduler(limit) {
 const service = new RunService({}, undefined, limit); const starts = [];
 service.execute = async run => { starts.push(run.id); await new Promise(() => {}); };
 return { service, starts };
}
test('global concurrency limit rejects invalid and unbounded values', () => {
 assert.equal(concurrencyLimit('2'),2); assert.equal(concurrencyLimit(undefined),2);
 assert.throws(() => concurrencyLimit('0'), /1.*8/); assert.throws(() => concurrencyLimit('9'), /1.*8/); assert.throws(() => concurrencyLimit('1.5'), /1.*8/);
});
test('one prepared queue admission reserves its slot before dispatch and respects the global limit', () => {
 const { service, starts } = scheduler(1); const a = queued('a'), b = queued('b');
 service.records.set('a',a); service.records.set('b',b); service.ready.add('a'); service.ready.add('b');
 assert.equal(service.dispatchOne(),true); assert.equal(service.dispatchOne(),false);
 assert.deepEqual(starts,['a']); assert.equal(a.status,'running'); assert.equal(b.status,'queued'); assert.equal(service.slots.size,1);
});
test('one slot release automatically dispatches a prepared waiting run only once', () => {
 const { service, starts } = scheduler(1); service.records.set('done',{ id:'done',status:'succeeded' }); service.slots.add('done');
 service.records.set('waiting',queued('waiting')); service.ready.add('waiting');
 assert.equal(service.releaseSlot('done'),true); assert.equal(service.releaseSlot('done'),false);
 assert.deepEqual(starts,['waiting']); assert.deepEqual([...service.slots],['waiting']);
});
test('cleanup holds and not-yet-durable admissions cannot consume another execution slot', () => {
 const { service, starts } = scheduler(1); service.records.set('held',{ id:'held',status:'failed',cleanupPending:true }); service.slots.add('held');
 service.records.set('waiting',queued('waiting')); service.ready.add('waiting');
 assert.equal(service.releaseSlot('held'),false); assert.equal(service.dispatchOne(),false); assert.deepEqual(starts,[]);
 const { service: unready } = scheduler(1); unready.records.set('pending',queued('pending')); assert.equal(unready.dispatchOne(),false);
});
test('acceptance captures separate queued values/configuration/definitions for same and different workflows', () => {
 const d = definition(); d.inputs = [{ name:'topic',type:'text',required:true }]; d.blocks.forEach(b => b.configurationId = 'c');
 const config = { version:1,id:'c',name:'C',model:'initial',reasoning:'low' };
 const a = acceptRun({ ...d,isolation:'workflow' },{topic:'A'},[config],'a');
 const b = acceptRun({ ...d,isolation:'block' },{topic:'B'},[config],'b');
 const c = acceptRun({ ...d,id:'other' },{topic:'C'},[config],'c');
 config.model = 'later'; d.blocks[0].prompt = 'later';
 assert.equal(a.status,'queued'); assert.equal(a.inputs.topic,'A'); assert.equal(b.inputs.topic,'B'); assert.equal(c.definition.id,'other');
 assert.equal(b.definition.isolation,'block'); assert.equal(a.configurations.a.model,'initial'); assert.equal(b.definition.blocks[0].prompt,'Summarize this');
});
