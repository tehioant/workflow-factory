import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdtemp, rm } from 'node:fs/promises';
import { compile } from 'svelte/compiler';
import {compileComponent} from './compile-component.ts';
import { Window } from 'happy-dom';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { pathToFileURL } from 'node:url';
import { editGraph } from '../src/lib/editor.ts';

test('p3 mounted inspector picker inserts only predecessor final response at the caret', async()=>{
 const window=new Window();
 Object.defineProperty(globalThis,'MutationObserver',{configurable:true,value:window.MutationObserver});
 for(const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','MouseEvent','HTMLMediaElement'])Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});
 Object.defineProperty(globalThis,'window',{configurable:true,value:window});Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
 const {mount,unmount,flushSync}=await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
 const filename=new URL('../src/lib/OutputReferencePicker.svelte',import.meta.url);
 const code=await compileComponent(filename);
 const directory=await mkdtemp(join(tmpdir(),'output-picker-'));let component:any;
 try{
  const modulePath=join(directory,'picker.mjs');await writeFile(modulePath,code);const Component=(await import(pathToFileURL(modulePath).href)).default;
  for(const invalid of [false,true]){
   const definition:any={schemaVersion:1, isolation: 'perRun' as const,id:'prepared',name:'Prepared',inputs:[],blocks:['c','b','a'].map(id=>({id,name:id.toUpperCase(),prompt:id==='b'?'Own replace tail':'Other',position:{x:0,y:0},configurationId:null})),connections:invalid?[]:[{source:'a',target:'b'},{source:'b',target:'c'}]};
   let editor:any={working:definition,revision:1,dirty:false,error:''};const target=window.document.createElement('main');window.document.body.append(target);const field=window.document.createElement('textarea');field.value='Own replace tail';field.setSelectionRange(4,11);target.append(field);let changes=0;
   flushSync(()=>{component=mount(Component,{target,props:{definition,blockId:'b',field,onprompt:(prompt:string)=>{changes++;editor=editGraph(editor,{type:'update',id:'b',patch:{prompt}});}}});});
   const select=target.querySelector('select')!;assert.deepEqual(Array.from(select.options).map(o=>o.value),invalid?['']:['','a']);
   const button=target.querySelector('button')!;
   if(!invalid){select.value='a';select.dispatchEvent(new window.Event('change',{bubbles:true}));flushSync();button.click();flushSync();assert.equal(changes,1);assert.equal(editor.working.blocks.find((b:any)=>b.id==='b').prompt,'Own {{output.a.finalResponse}} tail');assert.equal(editor.dirty,true);assert.equal(definition.blocks.find((b:any)=>b.id==='b').prompt,'Own replace tail');}
   else{assert.equal(button.disabled,true);assert.equal(changes,0);assert.equal(editor.dirty,false);}
   await unmount(component);component=null;target.remove();
  }
 }finally{if(component)await unmount(component);await rm(directory,{recursive:true,force:true});window.happyDOM.abort();}
});

test('ambiguous admission preserves the exact pending payload across every dirty choice', async()=>{
 const source=await readFile(new URL('../src/App.svelte',import.meta.url),'utf8');
 const body=source.slice(source.indexOf("  async function begin("),source.indexOf('\n  async function refresh()')).replace("mode:'save'|'saved'|'retry'","mode").replaceAll('$state.snapshot','snapshot');
 const original={requestId:'prepared-ambiguous-uuid',definitionId:'prepared',expectedRevision:3,expectedConfigurationRevision:2,supplied:{topic:'original'}};
 const calls:any[]=[];
 const make=new Function('commands','pending','snapshot',`let activeInspectionWorkflow=null;let pendingStart=pending;let busy=false,runReady=true,editor={working:{id:'changed'},dirty:true},supplied={topic:'changed'},runError='',run=null,dirtyStart=true;const errorMessage=String;${body};return {begin,pending:()=>pendingStart};`);
 const harness=make({startRun:async(request:any)=>{calls.push(request);throw Error('reply lost');},load:()=>{throw Error('new work forbidden');}},original,(x:any)=>structuredClone(x));
 for(const mode of ['save','saved']){await harness.begin(mode);assert.deepEqual(harness.pending(),original);assert.equal(calls.length,0);}
 await harness.begin('retry');assert.deepEqual(calls,[original]);assert.deepEqual(harness.pending(),original);
 for(const label of ['Save and start','Start last saved revision','Stay']){
  const button=source.split('<button ').find(s=>s.split('</button>')[0].endsWith(`>${label}`));
  assert.ok(button?.split('onclick')[0].includes('pendingStart!==null'),label);
 }
});

test('held and long-running panels remain refreshable and reject stale or unmounted reads', async()=>{
 const window=new Window();
 for(const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','MouseEvent','HTMLMediaElement'])Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});
 Object.defineProperty(globalThis,'window',{configurable:true,value:window});Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
 const {mount,unmount,flushSync}=await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
 const filename=new URL('../src/lib/RunPanel.svelte',import.meta.url);
 const code=await compileComponent(filename);
 const directory=await mkdtemp(join(tmpdir(),'run-panel-review-'));let component:any;
 const realSet=globalThis.setTimeout,realClear=globalThis.clearTimeout;let tick:(()=>Promise<void>)|undefined;
 try{
  const modulePath=join(directory,'component.mjs');await writeFile(modulePath,code);const Component=(await import(pathToFileURL(modulePath).href)).default;
  const target=window.document.createElement('main');window.document.body.append(target);let reads=0;const statuses:any[]=[];let resolveRead:((x:any)=>void)|undefined;
  const held={id:'prepared-held',state:'held',cleanupPending:true,finalResponse:null,diagnostic:'',blocks:[{blockId:'a',state:'succeeded',resolvedPrompt:'Own A <script>A</script>',finalResponse:'A <img src=x onerror=bad>'},{blockId:'b',state:'failed',resolvedPrompt:'Own B',finalResponse:null}]};
  const effects:string[]=[];
  const {createCommands}=await import('../src/lib/commands.ts');
  const commands=createCommands((async(command:string,args:any)=>{effects.push(command);assert.equal(args.id,held.id);if(command==='run_status'){reads++;return held;}if(command==='retry_run_output')return {...held,finalResponse:'Durable selected output'};throw Error('unrelated dispatch forbidden');}) as any);
  globalThis.setTimeout=((fn:any)=>{tick=fn;return 1;}) as any;globalThis.clearTimeout=(()=>{}) as any;
  flushSync(()=>{component=mount(Component,{target,props:{commands,run:held,onstatus:(s:any)=>statuses.push(s)}});});
  assert.equal(target.querySelectorAll('script,img').length,0);assert.ok(target.textContent.includes('A <img src=x onerror=bad>'));assert.ok(target.textContent.indexOf('a · succeeded')<target.textContent.indexOf('b · failed'));assert.ok(target.textContent.includes('Own B'));
  for(let i=0;i<125;i++){assert.ok(tick);await tick!();}assert.equal(reads,125);
  assert.deepEqual(effects,['inspect_workflow',...Array(125).fill('run_status')]);
  const retry=Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Retry retained output storage'));assert.ok(retry);
  retry.click();await Promise.resolve();await Promise.resolve();flushSync();assert.equal(effects.at(-1),'retry_run_output');assert.equal(statuses.at(-1).finalResponse,'Durable selected output');
  const recheck=Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Recheck'))!;assert.ok(recheck);
  const app=await readFile(new URL('../src/App.svelte',import.meta.url),'utf8');
  const selectBody=app.slice(app.indexOf('    const generation=++runSelection;'),app.indexOf('\n  async function begin('));
  const select=new Function('commands',`let runSelection=0,run=null,activeInspectionWorkflow=null,runError='';const errorMessage=String;return async function(entry){${selectBody.slice(0,selectBody.lastIndexOf('  }'))}}`)(commands);
  await select({id:held.id,definitionId:'captured-workflow'});assert.equal(effects.at(-1),'run_status');
  const pending:((x:any)=>void)[]=[];
  commands.runStatus=()=>new Promise(resolve=>{pending.push(resolve);});
  recheck.click();recheck.click();await Promise.resolve();
  const prior=statuses.length;pending[1](held);await Promise.resolve();await Promise.resolve();pending[0](held);await Promise.resolve();await Promise.resolve();assert.equal(statuses.length,prior+1);
  commands.runStatus=()=>new Promise(resolve=>{resolveRead=resolve;});recheck.click();await Promise.resolve();
  await unmount(component);component=null;const before=statuses.length;resolveRead!(held);await Promise.resolve();assert.equal(statuses.length,before);
  const queued={...held,id:'prepared-queued',state:'queued',cleanupPending:false,blocks:[]};const queuedCalls:string[]=[];
  const queuedCommands=createCommands((async(command:string,args:any)=>{if(command==='inspect_workflow')throw Error('prepared unavailable capture');assert.equal(args.id,queued.id);queuedCalls.push(command);if(command==='run_status')return queued;if(command==='retry_run_output')return {...queued,finalResponse:'Queued selected durable output'};throw Error('unrelated dispatch forbidden');}) as any);
  flushSync(()=>{component=mount(Component,{target,props:{commands:queuedCommands,run:queued,onstatus:(s:any)=>statuses.push(s)}});});
  await tick!();assert.deepEqual(queuedCalls,['run_status']);
  Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Retry retained output storage'))!.click();await Promise.resolve();await Promise.resolve();flushSync();
  assert.deepEqual(queuedCalls,['run_status','retry_run_output']);assert.equal(statuses.at(-1).id,queued.id);assert.equal(statuses.at(-1).finalResponse,'Queued selected durable output');
  await unmount(component);component=null;
 }finally{globalThis.setTimeout=realSet;globalThis.clearTimeout=realClear;if(component)await unmount(component);await rm(directory,{recursive:true,force:true});window.happyDOM.abort();}
});

test('f2 mounted workflow isolation selection changes only the working copy', async()=>{
 const window=new Window();
 for(const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','MouseEvent','HTMLMediaElement'])Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});
 Object.defineProperty(globalThis,'window',{configurable:true,value:window});Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
 const {mount,unmount,flushSync}=await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
 const filename=new URL('../src/lib/IsolationSelect.svelte',import.meta.url);
 const compiled=compile(await readFile(filename,'utf8'),{filename:filename.pathname,generate:'client'}).js.code;
 const code=compiled.replace(/import (['"])([^'"]+)\1/g,(_a,_q,s)=>`import ${JSON.stringify(import.meta.resolve(s))}`).replace(/from (['"])([^'"]+)\1/g,(_a,_q,s)=>`from ${JSON.stringify(s==='svelte'?new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href:import.meta.resolve(s))}`);
 const directory=await mkdtemp(join(tmpdir(),'isolation-select-'));let component:any;
 try{
  const modulePath=join(directory,'component.mjs');await writeFile(modulePath,code);const Component=(await import(pathToFileURL(modulePath).href)).default;
  const saved={isolation:'perRun'};let editor={working:saved,dirty:false,error:'old'};
  const target=window.document.createElement('main');window.document.body.append(target);
  flushSync(()=>{component=mount(Component,{target,props:{value:editor.working.isolation,onchange:(isolation:string)=>{editor={...editor,working:{...editor.working,isolation},dirty:true,error:''};}}});});
  const select=target.querySelector('select')!;assert.equal(target.querySelector('label')!.htmlFor,select.id);assert.equal(select.value,'perRun');assert.deepEqual(Array.from(select.options).map(o=>o.value),['perRun','perBlock']);
  select.value='perBlock';select.dispatchEvent(new window.Event('change',{bubbles:true}));flushSync();
  assert.deepEqual(editor,{working:{isolation:'perBlock'},dirty:true,error:''});assert.equal(saved.isolation,'perRun');
  const app=await readFile(new URL('../src/App.svelte',import.meta.url),'utf8');assert.match(app,/<IsolationSelect value={editor.working.isolation}/);assert.match(app,/working:{\.\.\.editor.working,isolation},dirty:true/);
 }finally{if(component)await unmount(component);await rm(directory,{recursive:true,force:true});window.happyDOM.abort();}
});




test('queued and active runs project scoped capacity and admission errors', async()=>{
 const window=new Window();
 for(const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','MouseEvent','HTMLMediaElement'])Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});
 Object.defineProperty(globalThis,'window',{configurable:true,value:window});Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
 const {mount,unmount,flushSync}=await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
 const code=await compileComponent(new URL('../src/lib/QueuePanel.svelte',import.meta.url));
 const directory=await mkdtemp(join(tmpdir(),'queue-projection-'));let component:any;
 const realSet=globalThis.setTimeout,realClear=globalThis.clearTimeout;let tick:any;
 try{
  const modulePath=join(directory,'queue.mjs');await writeFile(modulePath,code);const Component=(await import(pathToFileURL(modulePath).href)).default;
  const target=window.document.createElement('main');window.document.body.append(target);
  const entries=[{id:'oldest',definitionId:'workflow-a',state:'queued',diagnostic:'Credential wait <img>',cleanupPending:false},{id:'other',definitionId:'workflow-b',state:'running',diagnostic:'',cleanupPending:false},{id:'held',definitionId:'workflow-c',state:'held',diagnostic:'Persistence hold',cleanupPending:true},{id:'approval',definitionId:'workflow-d',state:'authorizationRequired',diagnostic:'Native confirmation required',cleanupPending:false}];
  const projection={settings:{active:4,waiting:16},occupied:3,waiting:1,entries};let reads=0;const writes:any[]=[];const selected:any[]=[];
  const commands={queueStatus:async()=>{reads++;return projection;},saveQueueSettings:async(value:any)=>{writes.push(value);throw {message:'Capacity cannot be reduced below occupied or waiting counts.'};}};
  globalThis.setTimeout=((fn:any)=>{tick=fn;return 1;}) as any;globalThis.clearTimeout=(()=>{}) as any;
  flushSync(()=>{component=mount(Component,{target,props:{commands,onselect:(entry:any)=>selected.push(entry),admissionError:"Waiting queue is full. No run was accepted."}});});
  const settle=async()=>{await Promise.resolve();await Promise.resolve();flushSync();};await settle();
  assert.ok(target.textContent.includes('Waiting queue is full. No run was accepted.'));assert.ok(target.textContent.includes('Occupied 3 / 4'));assert.ok(target.textContent.includes('Waiting 1 / 16'));assert.ok(target.textContent.includes('Credential wait <img>'));assert.ok(target.textContent.includes('authorizationRequired'));assert.ok(target.textContent.includes('Ownership hold'));assert.equal(target.querySelectorAll('img,script').length,0);
  const choose=Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Run oldest'))!;choose.click();flushSync();assert.deepEqual(selected,[entries[0]]);
  const input=target.querySelector('#queue-active') as HTMLInputElement;input.value='1';input.dispatchEvent(new window.Event('input',{bubbles:true}));flushSync();
  Array.from(target.querySelectorAll('button')).find(b=>b.textContent==='Save capacities')!.click();await settle();assert.deepEqual(writes,[{active:1,waiting:16}]);assert.ok(target.textContent.includes('cannot be reduced'));assert.equal(input.value,'1');
  await tick();await settle();assert.equal(reads,2);assert.equal(writes.length,1);assert.equal(input.value,'1');
  await unmount(component);component=null;assert.equal(target.textContent,'');
  const app=await readFile(new URL('../src/App.svelte',import.meta.url),'utf8');assert.match(app,/<QueuePanel \{commands\} onselect=\{selectQueued\} admissionError=\{runError\}/);assert.match(app,/selected\?\.id===entry.id/);assert.match(app,/activeInspectionWorkflow=entry.definitionId/);
 }finally{globalThis.setTimeout=realSet;globalThis.clearTimeout=realClear;if(component)await unmount(component);await rm(directory,{recursive:true,force:true});window.happyDOM.abort();}
});

test('fhistory mounted selected capture rejects stale identities and renders only escaped text', async()=>{
 const window=new Window();
 for(const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','MouseEvent','HTMLMediaElement'])Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});
 Object.defineProperty(globalThis,'window',{configurable:true,value:window});Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
 const {mount,unmount,flushSync}=await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
 const {createCommands}=await import('../src/lib/commands.ts');
 const filename=new URL('../src/lib/RunHistory.svelte',import.meta.url);
 const code=await compileComponent(filename);
 const directory=await mkdtemp(join(tmpdir(),'run-history-'));let component:any;
 try{
  const modulePath=join(directory,'component.mjs');await writeFile(modulePath,code);const Component=(await import(pathToFileURL(modulePath).href)).default;
  const target=window.document.createElement('main');window.document.body.append(target);
  const pending:any[]=[];
  const commands=createCommands(((command:string,args:any)=>command.startsWith('inspect_')?Promise.reject(Error('prepared unavailable inspection')):new Promise((resolve,reject)=>pending.push({command,args,resolve,reject}))) as any);
  commands.inspectSubscribe=async()=>{throw Error('prepared unavailable inspection');};
  const settle=async()=>{await Promise.resolve();await Promise.resolve();flushSync();};
  const page=(cursor:string|null=null)=>({definitionId:'workflow',cursor,entries:['a','b'].map(id=>({id,state:'held',inputSummary:'雪 <img src=x>',unavailable:false})),nextCursor:cursor?null:'b',unavailableRecords:true,unknownScopeRecords:true});
  const capture=(id:string)=>({id,definitionId:'workflow',definition:{revision:8,definition:{id:'workflow',name:'Captured <img src=x>',isolation:'perBlock',blocks:[{id:'first',name:'Captured first',prompt:'Own <script>bad</script>'}],connections:[],inputs:[]}},inputs:{values:{topic:'Captured input'},prompts:{first:'Prepared <img>'}},imageId:'retained-image',state:'held',cleanupPending:true,diagnostic:'held <script>',finalResponse:'Durable <img onerror=bad>',legacy:false,legacyConfiguration:null,blocks:[{blockId:'first',configuration:{revision:4,config:{name:'Settings <img>',model:'captured-model',reasoning:'high'}},result:{state:'succeeded',resolvedPrompt:'Actual <script>',finalResponse:'Predecessor durable'}},{blockId:'successor',configuration:null,result:null}]});
  const click=(text:string)=>{const button=Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes(text));assert.ok(button,text);button.click();flushSync();};
  flushSync(()=>{component=mount(Component,{target,props:{commands,definitionId:'workflow'}});});
  assert.deepEqual(pending[0].args,{definitionId:'workflow',cursor:null,limit:20});assert.equal(pending[0].command,'list_run_history');pending[0].resolve(page());await settle();
  assert.ok(target.textContent.includes('unsafe identities'));assert.ok(target.textContent.includes('may belong to other workflows'));
  click('Run a');assert.equal(pending[1].command,'run_detail');assert.deepEqual(pending[1].args,{definitionId:'workflow',id:'a'});
  click('Run b');pending[1].resolve(capture('a'));await settle();assert.ok(target.textContent.includes('Loading captured details'));assert.ok(!target.textContent.includes('captured-model'));
  pending[2].resolve(capture('b'));await settle();
  assert.ok(target.textContent.includes('captured-model / high'));assert.ok(target.textContent.includes('revision 8'));assert.ok(target.textContent.includes('Actual <script>'));assert.ok(target.textContent.includes('Predecessor durable'));assert.ok(target.textContent.includes('Not executed'));assert.equal(target.querySelectorAll('script,img').length,0);assert.ok(target.textContent.includes('Durable <img onerror=bad>'));
  assert.ok(!Array.from(target.querySelectorAll('button')).some(b=>/Stop|Start|execute/i.test(b.textContent)));
  click('Run a');const obsolete=pending.at(-1);click('Run b');const current=pending.at(-1);obsolete.reject(Error('obsolete secret'));await settle();assert.ok(target.textContent.includes('Loading captured details'));assert.ok(!target.textContent.includes('unavailable. Retry'));
  current.resolve(capture('a'));await settle();assert.ok(target.textContent.includes('Captured run unavailable'));assert.ok(!target.textContent.includes('captured-model'));
  click('Retry captured');pending.at(-1).reject(Error('current private failure'));await settle();assert.ok(target.textContent.includes('Captured run unavailable'));assert.ok(!target.textContent.includes('private failure'));
  click('Run a');const oldDetail=pending.at(-1);click('Older runs');const older=pending.at(-1);assert.deepEqual(older.args,{definitionId:'workflow',cursor:'b',limit:20});oldDetail.resolve(capture('a'));await settle();assert.ok(!target.textContent.includes('Captured run a'));
  older.resolve(page('b'));await settle();click('Newest page');const newest=pending.at(-1);newest.resolve(page());await settle();
  click('Refresh history');const oldPage=pending.at(-1); // Stale list finalizers must not hide a newer busy state.
  // The accessible control is disabled while loading; a workflow remount invalidates all requests.
  await unmount(component);component=null;oldPage.resolve(page());await settle();assert.equal(target.textContent,'');
  flushSync(()=>{component=mount(Component,{target,props:{commands,definitionId:'workflow'}});});pending.at(-1).resolve({...page(),definitionId:'other'});await settle();assert.ok(target.textContent.includes('Retained run list unavailable'));
  click('Refresh history');pending.at(-1).resolve(page());await settle();click('Run a');const unmounted=pending.at(-1);await unmount(component);component=null;unmounted.resolve(capture('a'));await settle();assert.equal(target.textContent,'');
  const app=await readFile(new URL('../src/App.svelte',import.meta.url),'utf8');assert.match(app,/<RunHistory \{commands\} definitionId=\{editor.working.id\}/);
 }finally{if(component)await unmount(component);await rm(directory,{recursive:true,force:true});window.happyDOM.abort();}
});
