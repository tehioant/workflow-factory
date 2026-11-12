import {test} from 'node:test';
import assert from 'node:assert/strict';
import {Window} from 'happy-dom';
import {readFile} from 'node:fs/promises';
import {compileComponent} from './compile-component.ts';
test('d13_mounted_shutdown_decision_retains_drafts_and_requires_fresh_consent',async()=>{
 const capability=JSON.parse(await readFile(new URL('../src-tauri/capabilities/main.json',import.meta.url),'utf8'));
 assert.deepEqual(capability.windows,['main']);
 assert.deepEqual(capability.permissions.filter((p:string)=>p.startsWith('core:')).sort(),['core:event:allow-listen','core:event:allow-unlisten']);
 const signinCapability=JSON.parse(await readFile(new URL('../src-tauri/capabilities/signin.json',import.meta.url),'utf8'));
 assert.ok(!signinCapability.permissions.some((p:string)=>p.startsWith('core:event:')));
 const window=new Window();for(const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','MouseEvent','HTMLMediaElement','MutationObserver'])Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});Object.defineProperty(globalThis,'window',{configurable:true,value:window});Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
 const {mount,unmount,flushSync}=await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
 const Component=(await import('data:text/javascript;base64,'+Buffer.from(await compileComponent(new URL('./shutdown-harness.svelte',import.meta.url))).toString('base64'))).default;
 const target=window.document.createElement('main');window.document.body.append(target);let component:any;const calls:string[]=[];let saveOK=false,finishOK=false;let pending:any;let confirmation:any;let confirmMode='ok';
 const settle=async()=>{for(let i=0;i<10;i++)await Promise.resolve();flushSync();};
 const click=(name:string)=>{const b=Array.from(target.querySelectorAll('button')).find(b=>b.textContent===name)!;assert.ok(b);b.click();};
 const commands:any={openSessionSignIn:async()=>{calls.push('setup-recovery');},shutdownPreview:()=>{calls.push('preview');return new Promise(resolve=>pending=resolve);},shutdownConfirm:async(token:string,consent:boolean)=>{assert.equal(token,'fresh');assert.equal(consent,true);calls.push('cancel');if(confirmMode==='refuse')throw new Error('state changed');if(confirmMode==='defer')return new Promise(resolve=>confirmation=resolve);return false;},shutdownFinish:async()=>{calls.push('finish');if(!finishOK)throw new Error('cleanup pending');}};
 const props={commands,dirty:true,busy:false,identity:'draft-1',save:async()=>{calls.push('save');return saveOK;},onstay:()=>calls.push('stay'),onfreeze:()=>calls.push('freeze')};
 try{
  flushSync(()=>{component=mount(Component,{target,props});});click('Stay');await settle();assert.deepEqual(calls,['stay']);calls.length=0;
  click('Save drafts (keep open)');await settle();assert.deepEqual(calls,['save']);assert.ok(target.textContent.includes('Drafts retained'));assert.ok(!calls.includes('cancel'));
  saveOK=true;click('Save drafts (keep open)');await settle();assert.deepEqual(calls,['save','save']);
  click('Inspect fresh shutdown state');pending({token:'fresh',work:2,workers:1,setup:true,retained:true,frozen:false});await settle();
  click('Confirm safe shutdown');await settle();assert.ok(!calls.includes('cancel'));
  const checkbox=target.querySelector('input')!;checkbox.checked=true;checkbox.dispatchEvent(new window.Event('change',{bubbles:true}));await settle();click('Confirm safe shutdown');await settle();assert.deepEqual(calls.slice(-3),['cancel','freeze','finish']);assert.ok(target.textContent.includes('cleanup pending'));
  finishOK=true;click('Recheck completion and quit');await settle();assert.equal(calls.at(-1),'finish');assert.equal(calls.filter(c=>c==='cancel').length,1);
  await unmount(component);component=null;
  // A refusal before freeze does not lock the shell or claim accepted consent.
  confirmMode='refuse';flushSync(()=>{component=mount(Component,{target,props});});
  click('Inspect fresh shutdown state');pending({token:'fresh',work:1,workers:1,setup:false,retained:false,frozen:false});await settle();
  let check=target.querySelector('input')!;check.checked=true;check.dispatchEvent(new window.Event('change',{bubbles:true}));await settle();
  const freezes=calls.filter(c=>c==='freeze').length;
  click('Confirm safe shutdown');await settle();assert.equal(calls.filter(c=>c==='freeze').length,freezes);assert.ok(target.textContent.includes('state changed'));
  confirmMode='defer';click('Confirm safe shutdown');await settle();flushSync(()=>component.changeIdentity('draft-2'));confirmation(true);await settle();
  assert.equal(calls.filter(c=>c==='freeze').length,freezes+1);assert.ok(target.textContent.includes('selected recovery'));assert.ok(target.textContent.includes('Admission is frozen'));assert.equal(target.querySelector('[role=dialog]')?.getAttribute('aria-modal'),'false');click('Open setup recovery');await settle();assert.equal(calls.at(-1),'setup-recovery');
  await unmount(component);component=null;
  flushSync(()=>{component=mount(Component,{target,props});});click('Inspect fresh shutdown state');pending({token:'fresh',work:1,workers:1,setup:false,retained:false,frozen:false});await settle();
  check=target.querySelector('input')!;check.checked=true;check.dispatchEvent(new window.Event('change',{bubbles:true}));await settle();click('Confirm safe shutdown');await settle();
  await unmount(component);component=null;const beforeLate=calls.length;confirmation(true);await settle();assert.deepEqual(calls.slice(beforeLate),['freeze']);
  // Mount the real App, replacing only unrelated panels and native boundaries.
  const moduleUrl=(source:string)=>'data:text/javascript;base64,'+Buffer.from(source).toString('base64');
  const stub=moduleUrl('export default function() {return {shutdownState:()=>({dirty:false,busy:false,identity:""}),saveDraft:async()=>true};}');
  const overrides=new Map<string,string>();
  for(const name of ['RunHistory','QueuePanel','IsolationSelect','RunPanel','OutputReferencePicker','AuthReadiness','Canvas','InputForm','RuntimeSettings','ConfigurationManager'])overrides.set(`./lib/${name}.svelte`,stub);
  let listener:(()=>void)|undefined,subscriptionResolve:((stop:()=>void)=>void)|undefined,subscriptionReject:((error:unknown)=>void)|undefined,stopped=0;
  (globalThis as any).__d13Listen=(event:string,callback:()=>void)=>{assert.equal(event,'shutdown-requested');assert.ok(capability.permissions.includes('core:event:allow-listen'));listener=callback;return new Promise((resolve,reject)=>{subscriptionResolve=resolve;subscriptionReject=reject;});};
  (globalThis as any).__d13Invoke=async(command:string)=>{if(command==='run_status')return null;if(command==='list_definitions'||command==='list_configurations')return [];throw new Error('Unexpected App command '+command);};
  overrides.set('@tauri-apps/api/event',moduleUrl('export const listen=(...args)=>globalThis.__d13Listen(...args);'));
  overrides.set('@tauri-apps/api/core',moduleUrl('export const invoke=(...args)=>globalThis.__d13Invoke(...args);export class Channel {}'));
  const App=(await import(moduleUrl(await compileComponent(new URL('../src/App.svelte',import.meta.url),overrides)))).default;
  flushSync(()=>{component=mount(App,{target});});await settle();
  subscriptionResolve!(()=>{assert.ok(capability.permissions.includes('core:event:allow-unlisten'));stopped++;});await settle();
  // All normal native routes converge on this same shutdown-requested consumer.
  for(const route of ['close','native Quit','framework ExitRequested']){listener!();await settle();assert.ok(target.querySelector('[aria-label="Safe application shutdown"]'),route);click('Stay');await settle();assert.equal(target.querySelector('[role=dialog]'),null);}
  await unmount(component);component=null;assert.equal(stopped,1);
  flushSync(()=>{component=mount(App,{target});});await settle();subscriptionReject!(new Error('sensitive native detail'));await settle();
  assert.ok(target.textContent.includes('Native shutdown requests are unavailable'));assert.ok(!target.textContent.includes('sensitive native detail'));click('Open safe shutdown');await settle();assert.ok(target.querySelector('[role=dialog]'));
  await unmount(component);component=null;
  flushSync(()=>{component=mount(App,{target});});await settle();await unmount(component);component=null;const detached=target.textContent;
  subscriptionResolve!(()=>{stopped++;});await settle();listener!();await settle();assert.equal(stopped,2);assert.equal(target.textContent,detached);
  delete (globalThis as any).__d13Listen;delete (globalThis as any).__d13Invoke;
 }finally{if(component)await unmount(component);window.happyDOM.abort();}
});
