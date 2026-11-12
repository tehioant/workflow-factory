import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdtemp, rm } from 'node:fs/promises';
import { compile } from 'svelte/compiler';
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
 const compiled=compile(await readFile(filename,'utf8'),{filename:filename.pathname,generate:'client'}).js.code;
 const code=compiled.replace(/import (['"])([^'"]+)\1/g,(_a,_q,s)=>`import ${JSON.stringify(import.meta.resolve(s))}`).replace(/from (['"])([^'"]+)\1/g,(_a,_q,s)=>`from ${JSON.stringify(s==='svelte'?new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href:s.startsWith('.')?new URL(s,filename).href:import.meta.resolve(s))}`);
 const directory=await mkdtemp(join(tmpdir(),'output-picker-'));let component:any;
 try{
  const modulePath=join(directory,'picker.mjs');await writeFile(modulePath,code);const Component=(await import(pathToFileURL(modulePath).href)).default;
  for(const invalid of [false,true]){
   const definition:any={schemaVersion:1,id:'prepared',name:'Prepared',inputs:[],blocks:['c','b','a'].map(id=>({id,name:id.toUpperCase(),prompt:id==='b'?'Own replace tail':'Other',position:{x:0,y:0},configurationId:null})),connections:invalid?[]:[{source:'a',target:'b'},{source:'b',target:'c'}]};
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
 const make=new Function('commands','pending','snapshot',`let pendingStart=pending;let busy=false,runReady=true,editor={working:{id:'changed'},dirty:true},supplied={topic:'changed'},runError='',run=null,dirtyStart=true;const errorMessage=String;${body};return {begin,pending:()=>pendingStart};`);
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
 const compiled=compile(await readFile(filename,'utf8'),{filename:filename.pathname,generate:'client'}).js.code;
 const code=compiled.replace(/import (['"])([^'"]+)\1/g,(_a,_q,s)=>`import ${JSON.stringify(import.meta.resolve(s))}`).replace(/from (['"])([^'"]+)\1/g,(_a,_q,s)=>`from ${JSON.stringify(s==='svelte'?new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href:s.startsWith('.')?new URL(s,filename).href:import.meta.resolve(s))}`);
 const directory=await mkdtemp(join(tmpdir(),'run-panel-review-'));let component:any;
 const realSet=globalThis.setTimeout,realClear=globalThis.clearTimeout;let tick:(()=>Promise<void>)|undefined;
 try{
  const modulePath=join(directory,'component.mjs');await writeFile(modulePath,code);const Component=(await import(pathToFileURL(modulePath).href)).default;
  const target=window.document.createElement('main');window.document.body.append(target);let reads=0;const statuses:any[]=[];let resolveRead:((x:any)=>void)|undefined;
  const held={id:'prepared-held',state:'held',cleanupPending:true,finalResponse:null,diagnostic:'',blocks:[{blockId:'a',state:'succeeded',resolvedPrompt:'Own A <script>A</script>',finalResponse:'A <img src=x onerror=bad>'},{blockId:'b',state:'failed',resolvedPrompt:'Own B',finalResponse:null}]};
  const commands={runStatus:async()=>{reads++;return held;},stopRun:async()=>{throw Error('not a stop journey');}};
  globalThis.setTimeout=((fn:any)=>{tick=fn;return 1;}) as any;globalThis.clearTimeout=(()=>{}) as any;
  flushSync(()=>{component=mount(Component,{target,props:{commands,run:held,onstatus:(s:any)=>statuses.push(s)}});});
  assert.equal(target.querySelectorAll('script,img').length,0);assert.ok(target.textContent.includes('A <img src=x onerror=bad>'));assert.ok(target.textContent.indexOf('a · succeeded')<target.textContent.indexOf('b · failed'));assert.ok(target.textContent.includes('Own B'));
  for(let i=0;i<125;i++){assert.ok(tick);await tick!();}assert.equal(reads,125);
  const recheck=Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Recheck'))!;assert.ok(recheck);
  const pending:((x:any)=>void)[]=[];
  commands.runStatus=()=>new Promise(resolve=>{pending.push(resolve);});
  recheck.click();recheck.click();await Promise.resolve();
  const prior=statuses.length;pending[1](held);await Promise.resolve();await Promise.resolve();pending[0](held);await Promise.resolve();await Promise.resolve();assert.equal(statuses.length,prior+1);
  commands.runStatus=()=>new Promise(resolve=>{resolveRead=resolve;});recheck.click();await Promise.resolve();
  await unmount(component);component=null;const before=statuses.length;resolveRead!(held);await Promise.resolve();assert.equal(statuses.length,before);
 }finally{globalThis.setTimeout=realSet;globalThis.clearTimeout=realClear;if(component)await unmount(component);await rm(directory,{recursive:true,force:true});window.happyDOM.abort();}
});
