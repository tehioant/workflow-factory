import {test} from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {Window} from 'happy-dom';
import {RunService,acceptRun} from '../src/runs.ts';
import {mountQuit} from '../src/public/lifecycle.ts';
import {installShutdownSignals,closeAfterReply} from '../src/lifecycle.ts';
import {handleRequest} from '../src/http.ts';
import {definition} from './fixtures.mjs';
function retained(id) { const d=definition(); d.blocks.forEach(b=>b.configurationId='c'); return acceptRun(d,{},[{version:1,id:'c',name:'C',model:'m',reasoning:'low'}],id); }

test('lifecycle status counts active, queued and retained holds without treating completed history as active',()=>{
 const service=new RunService({}); service.records.set('a',{status:'running'}); service.records.set('b',{status:'queued'}); service.records.set('c',{status:'succeeded'}); service.records.set('d',{status:'failed',cleanupPending:true});
 assert.deepEqual(service.lifecycleStatus(),{active:1,queued:1,pending:3,frozen:false});
});
test('confirmed shutdown freezes dispatch first, invokes every selected cancellation and flushes once',async()=>{
 const service=new RunService({}); const calls=[]; service.records.set('a',{id:'a',status:'running'}); service.records.set('b',{id:'b',status:'queued'}); service.ready.add('b');
 service.cancel=async id=>{assert.equal(service.frozen,true);calls.push(id);service.records.get(id).status='cancelled';}; service.flush=async()=>{calls.push('flush');};
 const first=service.shutdown(); assert.equal(service.shutdown(),first); await first;
 assert.deepEqual(calls,['a','b','flush']); assert.equal(service.ready.size,0); assert.equal(service.lifecycleStatus().pending,0);
});
test('frozen admission and dispatch guards reject late requests without creating or starting a record',async()=>{
 const service=new RunService({}); service.frozen=true; await assert.rejects(service.launch({}, {}, []),/quitting/); assert.equal(service.records.size,0);
 service.records.set('waiting',{id:'waiting',status:'queued'}); service.ready.add('waiting'); assert.equal(service.dispatchOne(),false);
});
test('shutdown attempts every owned cleanup despite a failed terminal save and remains frozen/recoverable',async()=>{
 const cleaned=[]; const service=new RunService({async reconcileCleanup(id){cleaned.push(id);}}, {async save(run){if(run.id==='bad')throw new Error('Disk full');return run;}});
 for(const id of ['bad','good']) {const run=retained(id);run.status='failed';run.cleanupPending=true;service.records.set(id,run);}
 await assert.rejects(service.shutdown(),/history|pending/); assert.deepEqual(cleaned,['bad','good']); assert.equal(service.records.get('bad').cleanupPending,true); assert.equal(service.records.get('good').cleanupPending,false); assert.equal(service.frozen,true);
});
test('dismissed quit confirmation leaves cancellation and exit boundaries untouched',async()=>{
 const window=new Window();const root=window.document.createElement('div');let quits=0;const warnings=[];
 mountQuit(root,{pending:()=>true,status:async()=>({pending:2}),confirm:message=>{warnings.push(message);return false;},quit:async()=>{quits++;},closed(){assert.fail();}});
 await root.querySelector('button').onclick(new window.Event('click'));assert.equal(quits,0);assert.match(warnings[0],/2.*active|2.*pending/);assert.match(root.textContent,/Kept running/);
});
test('confirmed quit control invokes the quit boundary and reports closed only after it resolves',async()=>{
 const window=new Window();const root=window.document.createElement('div');const calls=[];
 mountQuit(root,{pending:()=>true,status:async()=>({pending:1}),confirm:()=>true,quit:async confirmed=>{calls.push(['quit',confirmed]);},closed(){calls.push(['closed']);}});
 await root.querySelector('button').onclick(new window.Event('click'));assert.deepEqual(calls,[['quit',true],['closed']]);assert.match(root.textContent,/Close this tab/);
});
test('beforeunload warns only from supplied pending state and never calls server quit',()=>{
 const window=new Window();const root=window.document.createElement('div');let pending=true;
 mountQuit(root,{pending:()=>pending,status:async()=>({pending:1}),confirm:()=>false,quit:async()=>assert.fail(),closed(){assert.fail();}});
 const warning=new window.Event('beforeunload',{cancelable:true});window.dispatchEvent(warning);assert.equal(warning.defaultPrevented,true);
 pending=false;const idle=new window.Event('beforeunload',{cancelable:true});window.dispatchEvent(idle);assert.equal(idle.defaultPrevented,false);
});
test('quit API requires fresh confirmation if work became pending and never freezes on rejection',async()=>{
 const authority={host:'127.0.0.1:3000',token:'t'};let quits=0;const request=()=>new Request('http://'+authority.host+'/api/quit',{method:'POST',headers:{host:authority.host,origin:'http://'+authority.host,'x-workflow-token':'t','content-type':'application/json'},body:JSON.stringify({confirmed:false})});
 const response=await handleRequest(request(),authority,{}, {lifecycle:{status:()=>({pending:1}),async quit(){quits++;}}});assert.equal(response.status,409);assert.equal(quits,0);
});
test('SIGINT and SIGTERM handlers both invoke the graceful boundary and surface failures without forced exit',async()=>{
 const source=new EventEmitter();let quits=0;const errors=[];const uninstall=installShutdownSignals(source,async()=>{quits++;throw new Error('Pending cleanup');},error=>errors.push(error.message));
 source.emit('SIGINT');source.emit('SIGTERM');await new Promise(resolve=>setImmediate(resolve));assert.equal(quits,2);assert.deepEqual(errors,['Pending cleanup','Pending cleanup']);uninstall();assert.equal(source.listenerCount('SIGTERM'),0);
});
test('HTTP exit waits for reply finish or disconnect, closes once and handles an already disconnected client',async()=>{
 const reply=new EventEmitter();reply.destroyed=false;let closes=0;closeAfterReply(reply,async()=>{closes++;},()=>assert.fail());assert.equal(closes,0);
 reply.emit('close');reply.emit('finish');assert.equal(closes,1);assert.equal(reply.listenerCount('finish'),0);
 const gone=new EventEmitter();gone.destroyed=true;closeAfterReply(gone,async()=>{closes++;},()=>assert.fail());assert.equal(closes,2);
});
