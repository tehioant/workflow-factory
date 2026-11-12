import {test} from 'node:test';
import assert from 'node:assert/strict';
import {Window} from 'happy-dom';
import {compileComponent} from './compile-component.ts';
test('d10_mounted_selected_run_updates_resynchronize_and_release',async()=>{
 const window=new Window();for(const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','MouseEvent','HTMLMediaElement','MutationObserver'])Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});Object.defineProperty(globalThis,'window',{configurable:true,value:window});Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
 const {mount,unmount,flushSync}=await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
 const Component=(await import('data:text/javascript;base64,'+Buffer.from(await compileComponent(new URL('../src/lib/RunInspection.svelte',import.meta.url))).toString('base64'))).default;
 let component:any;let notify:any;const pending:any[]=[],closed:any[]=[],calls:any[]=[];let captured=false,lists=0;let deferList=false,listPending:any;
 const response=(overrides:any={})=>({run:'run-a',subscription:'sub-a',cursor:'opaque-1',generation:'generation-a',floor:'0',high:'1',resync:true,incomplete:false,available:true,more:false,entries:[{sequence:'1',block:'a',kind:'command',text:'actual <img src=x> café'}],...overrides});
 const commands:any={inspectSubscribe:(workflow:string,run:string,callback:any)=>{calls.push(['subscribe',workflow,run]);notify=callback;return new Promise(resolve=>pending.push(resolve));},inspectPage:(...args:any[])=>{calls.push(['page',...args]);return new Promise(resolve=>pending.push(resolve));},inspectClose:(...args:any[])=>{closed.push(args);return Promise.resolve();},inspectArtifacts:async()=>{lists++;const result={run:'run-a',generation:'capture-a',available:captured,complete:captured,cursor:null,entries:captured?[{id:'opaque-file',name:'result <script>.txt'}]:[]};return deferList?new Promise(resolve=>{listPending=resolve;}):result;},inspectArtifact:(...args:any[])=>{calls.push(['artifact',...args]);return new Promise(resolve=>pending.push(resolve));}};
 const target=window.document.createElement('main');window.document.body.append(target);const settle=async()=>{for(let i=0;i<8;i++)await Promise.resolve();flushSync();};
 try{
  flushSync(()=>{component=mount(Component,{target,props:{commands,definitionId:'workflow-a',runId:'run-a'}});});assert.deepEqual(calls,[['subscribe','workflow-a','run-a']]);
  notify({run:'run-a',subscription:'sub-a',generation:'generation-a',floor:'0',high:'2'});pending.shift()(response());await settle();assert.equal(calls.filter(c=>c[0]==='page').length,1);
  assert.equal(lists,1);captured=true;for(let i=0;i<100;i++)notify({run:'run-a',subscription:'sub-a',generation:'generation-a',floor:'0',high:'3'});assert.equal(calls.filter(c=>c[0]==='page').length,1);
  pending.shift()(response({cursor:'opaque-2',high:'2'}));await settle();assert.equal(calls.filter(c=>c[0]==='page').length,2);
  pending.shift()(response({cursor:'opaque-3',generation:'generation-b',floor:'9',high:'10',entries:[{sequence:'10',block:'a',kind:'error',text:'resynchronized <script> text'}]}));await settle();assert.ok(target.textContent.includes('Snapshot resynchronized'));assert.ok(target.textContent.includes('resynchronized <script> text'));assert.ok(!target.textContent.includes('actual <img'));assert.equal(target.querySelectorAll('img,script').length,0);
  assert.ok(lists>=2,'matching capture notice refreshes unavailable artifact snapshot');notify({run:'other',subscription:'sub-a'});notify({run:'run-a',subscription:'other'});assert.equal(pending.length,0);
  const file=Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('result <script>'))!;file.click();await settle();assert.deepEqual(calls.at(-1),['artifact','workflow-a','run-a','sub-a','capture-a','opaque-file',null]);pending.shift()({run:'other',generation:'capture-a',id:'opaque-file',text:'wrong secret',cursor:null});await settle();assert.ok(!target.textContent.includes('wrong secret'));assert.ok(target.textContent.includes('Artifact text unavailable'));
  const reload=Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Reload retained'))!;reload.click();await settle();assert.equal(calls.at(-1)!.at(-1),null);const stale=pending.shift();await unmount(component);component=null;assert.deepEqual(closed,[['workflow-a','run-a','sub-a']]);stale(response());await settle();assert.equal(target.textContent,'');
  flushSync(()=>{component=mount(Component,{target,props:{commands,definitionId:'workflow-a',runId:'run-a'}});});const initial=pending.shift();await unmount(component);component=null;initial(response({subscription:'late-sub'}));await settle();assert.deepEqual(closed.at(-1),['workflow-a','run-a','late-sub']);
  // Capture already durable before the initial snapshot; later log notices must
  // preserve the selected text/page and never reload an available capture.
  flushSync(()=>{component=mount(Component,{target,props:{commands,definitionId:'workflow-a',runId:'run-a'}});});pending.shift()(response());await settle();
  const positive=Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('result <script>'))!;positive.click();await settle();pending.shift()({run:'run-a',generation:'capture-a',id:'opaque-file',text:'selected text page',cursor:'text-next'});await settle();
  const listCount=lists;notify({run:'run-a',subscription:'sub-a',generation:'generation-a',high:'9'});pending.shift()(response());await settle();assert.equal(lists,listCount);assert.ok(target.textContent.includes('selected text page'));
  await unmount(component);component=null;
  // An automatic initial artifact read may complete after unmount; no stale UI,
  // queued refresh or selected-text mutation may survive that response.
  deferList=true;captured=false;flushSync(()=>{component=mount(Component,{target,props:{commands,definitionId:'workflow-a',runId:'run-a'}});});pending.shift()(response());await settle();notify({run:'run-a',subscription:'sub-a',high:'10'});const beforeStale=calls.length;await unmount(component);component=null;listPending({run:'run-a',generation:'capture-late',available:true,complete:true,cursor:null,entries:[{id:'late',name:'late stale artifact'}]});await settle();assert.equal(target.textContent,'');assert.equal(calls.length,beforeStale);
 }finally{if(component)await unmount(component);window.happyDOM.abort();}
});
