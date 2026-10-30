import { test } from 'node:test';
import assert from 'node:assert/strict';
import { RunService,acceptRun } from '../src/runs.ts';
import { DockerRuntime } from '../src/runtime.ts';
import { validateRun } from '../src/run-store.ts';
import { renderRun } from '../src/public/runs.ts';
import { handleRequest } from '../src/http.ts';
import { Window } from 'happy-dom';
import { definition } from './fixtures.mjs';
function captured(id='selected') { const d = definition(); d.blocks.forEach(b => b.configurationId='c'); return acceptRun(d,{},[{ version:1,id:'c',name:'C',model:'m',reasoning:'low' }],id); }
const deferred = () => { let resolve; const promise = new Promise(r => { resolve=r; }); return { promise,resolve }; };

test('queued cancellation removes readiness, persists cancelled state and leaves other runs unchanged', async () => {
 const writes=[]; let stops=0; const service = new RunService({ async stop() { stops++; } },{ async save(r) { writes.push(r); } });
 const selected=captured(), other=captured('other'); service.records.set(selected.id,selected); service.records.set(other.id,other); service.ready.add(selected.id);
 await service.cancel(selected.id); assert.equal(selected.status,'cancelled'); assert.equal(service.ready.has(selected.id),false); assert.equal(other.status,'queued'); assert.equal(stops,0); assert.equal(writes.at(-1).status,'cancelled');
 assert.doesNotThrow(() => validateRun(writes.at(-1)));
});
test('active cancellation aborts even pending preparation, targets only its run and waits for owned completion', async () => {
 const stopped=[]; const service=new RunService({ async stop(id) { stopped.push(id); } }); const selected=captured(),other=captured('other'); selected.status='running'; selected.blocks.a={ status:'succeeded',prompt:'A',output:'Keep',logs:'' };
 service.records.set(selected.id,selected); service.records.set(other.id,other); service.slots.add(selected.id); const controller=new AbortController(); service.controllers.set(selected.id,controller); const completion=deferred(); service.tasks.set(selected.id,completion.promise);
 let settled=false; void service.cancel(selected.id).then(() => { settled=true; }); await new Promise(r => setImmediate(r));
 assert.equal(controller.signal.aborted,true); assert.deepEqual(stopped,[selected.id]); assert.equal(settled,false); assert.equal(selected.blocks.a.output,'Keep'); assert.equal(other.cancelRequested,undefined); assert.equal(service.slots.has(selected.id),true);
});
test('cancel flag prevents a successor transition before persistence or runtime work', async () => {
 const service=new RunService({ async execute() { assert.fail('Must not execute'); } }); const selected=captured(); selected.status='running'; selected.cancelRequested=true; selected.blocks.a={status:'succeeded',prompt:'A',output:'Keep',logs:''};
 assert.equal(await service.advance(selected,{}),false); assert.equal(selected.blocks.b,undefined); assert.equal(selected.blocks.a.output,'Keep');
});
test('cancellation/completion settlement releases an occupied slot only once', async () => {
 const service=new RunService({},undefined,1); const selected=captured(); selected.status='cancelled'; service.records.set(selected.id,selected); service.slots.add(selected.id);
 await Promise.all([service.settleExecution(selected,Promise.resolve()),service.settleExecution(selected,Promise.resolve())]);
 assert.equal(service.slots.size,0); assert.equal(service.releaseSlot(selected.id),false); assert.equal(selected.status,'cancelled');
});
test('cancelled setup command is allowed to finish before reporting abort, with no signal sent to a side-effecting Docker command', async () => {
 const command=deferred(); const calls=[]; const runtime=new DockerRuntime('/fake',async (...args) => { calls.push(args); return command.promise; }); const controller=new AbortController();
 const pending=runtime.checked(['run','-d','--name','owned'],'Failed',controller.signal); controller.abort(); let settled=false; void pending.catch(() => { settled=true; }); await Promise.resolve(); assert.equal(settled,false);
 command.resolve({code:0,stdout:'created',stderr:''}); await assert.rejects(pending,{name:'AbortError'}); assert.equal(calls[0].length,1);
});
test('cancelled agent setup does not issue a Docker creation command', async () => {
 let calls=0; const runtime=new DockerRuntime('/fake',async () => { calls++; }); const controller=new AbortController(); controller.abort();
 await assert.rejects(runtime.startAgent({agent:'owned'},controller.signal),{name:'AbortError'}); assert.equal(calls,0);
});
test('runtime stop removes only registered owned agent names and leaves network/credentials cleanup to settled execution', async () => {
 const calls=[]; const runtime=new DockerRuntime('/fake',async args => { calls.push(args); return {code:0,stdout:'',stderr:''}; }); runtime.resources.set('selected',{ agents:['wf-selected-block-a','wf-selected-block-b'] }); runtime.resources.set('other',{agents:['wf-other-agent']});
 await runtime.stop('selected'); assert.deepEqual(calls,[['rm','-f','wf-selected-block-a'],['rm','-f','wf-selected-block-b']]); assert.equal(runtime.resources.has('other'),true);
});
test('cancel control invokes only selected run and is absent for terminal history', async () => {
 const window=new Window(); const root=window.document.createElement('div'); const selected=captured(); const calls=[];
 renderRun(root,selected,{async cancel(id) {calls.push(id);}}); const button=root.querySelector('[data-cancel-run]'); await button.onclick(new window.Event('click')); assert.deepEqual(calls,[selected.id]);
 selected.status='cancelled'; renderRun(root,selected,{async cancel() {assert.fail();}}); assert.equal(root.querySelector('[data-cancel-run]'),null);
});
test('same-origin cancellation API invokes the actual selected-run boundary once', async () => {
 const calls=[]; const authority={host:'127.0.0.1:3000',token:'test'}; const request=new Request('http://127.0.0.1:3000/api/runs/selected/cancel',{method:'POST',headers:{host:authority.host,origin:'http://'+authority.host,'x-workflow-token':'test','content-type':'application/json'}});
 const response=await handleRequest(request,authority,{}, { runs:{async cancel(id) {calls.push(id);return {id,status:'cancelled'};}} });
 assert.equal(response.status,200); assert.deepEqual(calls,['selected']);
});
