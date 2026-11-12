import {test} from 'node:test';
import assert from 'node:assert/strict';
import {Window} from 'happy-dom';
import {compileComponent} from './compile-component.ts';
test('d13_mounted_shutdown_decision_retains_drafts_and_requires_fresh_consent',async()=>{
 const window=new Window();for(const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','MouseEvent','HTMLMediaElement','MutationObserver'])Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});Object.defineProperty(globalThis,'window',{configurable:true,value:window});Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
 const {mount,unmount,flushSync}=await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
 const Component=(await import('data:text/javascript;base64,'+Buffer.from(await compileComponent(new URL('../src/lib/ShutdownDialog.svelte',import.meta.url))).toString('base64'))).default;
 const target=window.document.createElement('main');window.document.body.append(target);let component:any;const calls:string[]=[];let saveOK=false,finishOK=false;let pending:any;
 const settle=async()=>{for(let i=0;i<10;i++)await Promise.resolve();flushSync();};
 const click=(name:string)=>{const b=Array.from(target.querySelectorAll('button')).find(b=>b.textContent===name)!;assert.ok(b);b.click();};
 const commands:any={shutdownPreview:()=>{calls.push('preview');return new Promise(resolve=>pending=resolve);},shutdownConfirm:async(token:string,consent:boolean)=>{assert.equal(token,'fresh');assert.equal(consent,true);calls.push('cancel');},shutdownFinish:async()=>{calls.push('finish');if(!finishOK)throw new Error('cleanup pending');}};
 const props={commands,dirty:true,busy:false,identity:'draft-1',save:async()=>{calls.push('save');return saveOK;},onstay:()=>calls.push('stay'),onfreeze:()=>calls.push('freeze')};
 try{
  flushSync(()=>{component=mount(Component,{target,props});});click('Stay');await settle();assert.deepEqual(calls,['stay']);calls.length=0;
  click('Save drafts (keep open)');await settle();assert.deepEqual(calls,['save']);assert.ok(target.textContent.includes('Drafts retained'));assert.ok(!calls.includes('cancel'));
  saveOK=true;click('Save drafts (keep open)');await settle();assert.deepEqual(calls,['save','save']);
  click('Inspect fresh shutdown state');pending({token:'fresh',work:2,workers:1,setup:true,retained:true,frozen:false});await settle();
  click('Confirm safe shutdown');await settle();assert.ok(!calls.includes('cancel'));
  const checkbox=target.querySelector('input')!;checkbox.checked=true;checkbox.dispatchEvent(new window.Event('change',{bubbles:true}));await settle();click('Confirm safe shutdown');await settle();assert.deepEqual(calls.slice(-3),['cancel','freeze','finish']);assert.ok(target.textContent.includes('cleanup pending'));
  finishOK=true;click('Recheck completion and quit');await settle();assert.equal(calls.at(-1),'finish');assert.equal(calls.filter(c=>c==='cancel').length,1);
 }finally{if(component)await unmount(component);window.happyDOM.abort();}
});
