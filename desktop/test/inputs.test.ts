import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { compileComponent } from './compile-component.ts';
import { createCommands } from '../src/lib/commands.ts';
import { insertInputReference } from '../src/lib/inputs.ts';

const moduleUrl = (source: string) => 'data:text/javascript;base64,' + Buffer.from(source).toString('base64');
const window = new Window();
async function fixture() {
 for (const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','KeyboardEvent','MouseEvent','HTMLMediaElement','MutationObserver']) Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});
 Object.defineProperty(globalThis,'window',{configurable:true,value:window});
 const svelte = await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
 const target = window.document.createElement('main'); window.document.body.append(target);
 return {window,target,...svelte,settle:async()=>{for(let i=0;i<12;i++)await Promise.resolve();svelte.flushSync();}};
}

test('input form preparation uses only prepared values in a narrow command',async()=>{
 let actual:unknown; const commands=createCommands(async<T>(command:string,args?:Record<string,unknown>)=>{actual={command,args};return {values:{topic:'text'},prompts:{a:'text'}} as T;});
 const definition={schemaVersion:1, isolation: 'perRun' as const,id:'prepared',name:'Draft',blocks:[],connections:[],inputs:[]};
 const prepared=await commands.prepare(definition,{topic:'text'});
 assert.deepEqual(actual,{command:'prepare_inputs',args:{definition,supplied:{topic:'text'}}});assert.deepEqual(prepared.prompts,{a:'text'});
 const {window,target,mount,unmount,flushSync,settle}=await fixture();
 const Component=(await import(moduleUrl(await compileComponent(new URL('../src/lib/RunPreparation.svelte',import.meta.url))))).default;
 const input=(name:string)=>({name,required:false,default:null});
 const old={definition:{...definition,inputs:[input('old'),input('same')]},revision:1};
 const chosen={definition:{...definition,inputs:[input('new'),input('same')]},revision:2};
 let component:any,loads=0;const supplied:any[]=[];
 try {
  flushSync(()=>component=mount(Component,{target,props:{commands:{load:async()=>++loads===1?old:chosen,prepare:async(d:any,v:any)=>{assert.deepEqual(d,chosen.definition);supplied.push(v);}},definition,revision:1,dirty:false,locked:false,pending:false,admissionError:'',onsave:()=>assert.fail('save forbidden'),onstart:async(d:any,v:any)=>{assert.deepEqual(d,chosen);supplied.push(v);},onretry:()=>assert.fail('retry forbidden'),onclose:()=>{},onsettings:()=>{}}}));
  const click=(text:string)=>{const b=[...target.querySelectorAll('button')].find(b=>b.textContent!.includes(text));assert.ok(b);b.click();};
  click('Prepare saved revision');await settle();
  for(const [name,value] of [['old','obsolete'],['same','{{input.literal}}\nunchanged']]){const field=target.querySelector(`#run-input-${name}`)! as HTMLTextAreaElement;field.value=value;field.dispatchEvent(new window.Event('input',{bubbles:true}));}flushSync();
  click('Change revision choice');await settle();click('Prepare saved revision');await settle();
  assert.equal(target.querySelector('#run-input-old'),null);
  assert.equal((target.querySelector('#run-input-same') as HTMLTextAreaElement).value,'{{input.literal}}\nunchanged');
  click('Start revision 2');await settle();
  assert.deepEqual(supplied,[{same:'{{input.literal}}\nunchanged'},{same:'{{input.literal}}\nunchanged'}]);
 } finally {if(component)await unmount(component);target.remove();}
});

test('picker inserts a reference at the selected prompt range, not a value',async()=>{
 assert.equal(insertInputReference('Hello world',6,11,'topic'),'Hello {{input.topic}}');
 const {window,target,mount,unmount,flushSync,settle}=await fixture();
 const Component=(await import(moduleUrl(await compileComponent(new URL('../src/lib/BlockInspector.svelte',import.meta.url))))).default;
 const block={id:'b',name:'B',prompt:'Own replace tail',position:{x:0,y:0},configurationId:'missing'};
 const definition={schemaVersion:1,id:'prepared',name:'Prepared',isolation:'perRun',inputs:[{name:'topic',required:true,default:null}],blocks:[{...block,id:'a',name:'A'},block,{...block,id:'c',name:'C'}],connections:[{source:'a',target:'b'},{source:'b',target:'c'}]};
 let component:any;const patches:any[]=[];
 const props={definition,block,configurations:[],disabled:false,onpatch:(patch:any)=>{patches.push(patch);const field=target.querySelector('#prompt') as HTMLTextAreaElement;if(patch.prompt)field.value=patch.prompt;},onremove:()=>assert.fail('remove forbidden')};
 try {
  flushSync(()=>component=mount(Component,{target,props}));await settle();
  const field=target.querySelector('#prompt') as HTMLTextAreaElement;field.focus();field.setSelectionRange(4,11);
  const trigger=[...target.querySelectorAll('button')].find(b=>b.textContent!.includes('Insert reference'))!;
  trigger.click();await settle();const cancel=[...target.querySelectorAll('button')].find(b=>b.textContent==='Cancel')!;cancel.focus();
  let escaped=0;target.addEventListener('keydown',()=>escaped++);
  cancel.dispatchEvent(new window.KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));await settle();
  assert.equal(target.querySelector('.reference-menu'),null);assert.equal(document.activeElement,trigger);assert.equal(escaped,0);assert.deepEqual(patches,[]);
  trigger.click();await settle();[...target.querySelectorAll('button')].find(b=>b.textContent==='Cancel')!.click();await settle();assert.equal(document.activeElement,trigger);
  trigger.click();await settle();const select=target.querySelector('#prompt-reference') as HTMLSelectElement;
  assert.deepEqual([...select.options].map(o=>o.value),['','input.topic','output.a.finalResponse']);
  // happy-dom does not match option:checked; Svelte's binding queries it.
  const query=select.querySelector.bind(select);
  select.querySelector=((selector:string)=>selector.startsWith(':checked') ? select.selectedOptions[0] ?? null : query(selector)) as any;
  select.value='output.a.finalResponse';select.dispatchEvent(new window.Event('change',{bubbles:true}));await settle();
  assert.equal(select.value,'output.a.finalResponse');
  assert.equal([...target.querySelectorAll('button')].find(b=>b.textContent==='Insert at saved caret')!.disabled,false);
  [...target.querySelectorAll('button')].find(b=>b.textContent==='Insert at saved caret')!.click();await settle();
  assert.deepEqual(patches,[{prompt:'Own {{output.a.finalResponse}} tail'}]);assert.equal(document.activeElement,field);assert.equal(field.selectionStart,30);assert.equal(field.selectionEnd,30);
  const config=target.querySelector('#block-config')!;assert.equal(config.getAttribute('aria-invalid'),'true');assert.ok(target.querySelector('#'+config.getAttribute('aria-describedby'))?.textContent?.includes('unavailable'));
  await unmount(component);component=null;
  for(const prompt of ['{{input.absent}}','{{output.b.finalResponse}}','{{output.c.finalResponse}}','{{output.gone.finalResponse}}','{{output.a.raw}}','{{input.topic','{{ input.topic }}']){
   flushSync(()=>component=mount(Component,{target,props:{...props,block:{...block,prompt}}}));await settle();
   const control=target.querySelector('#prompt')!;assert.equal(control.getAttribute('aria-invalid'),'true',prompt);assert.ok(target.querySelector('#'+control.getAttribute('aria-describedby'))?.textContent, prompt);
   await unmount(component);component=null;
  }
  flushSync(()=>component=mount(Component,{target,props:{...props,block:{...block,prompt:'{{input.topic}} {{output.a.finalResponse}}'}}}));await settle();assert.notEqual(target.querySelector('#prompt')!.getAttribute('aria-invalid'),'true');
  await unmount(component);component=null;
  const Properties=(await import(moduleUrl(await compileComponent(new URL('../src/lib/DefinitionProperties.svelte',import.meta.url))))).default;
  flushSync(()=>component=mount(Properties,{target,props:{definition:{...definition,blocks:[{...block,prompt:'{{input.topic}}'}]},onchange:()=>assert.fail('Referenced declaration must remain unchanged')}}));await settle();
  const name=target.querySelector('#input-name-0') as HTMLInputElement;name.value='renamed';name.dispatchEvent(new window.Event('input',{bubbles:true}));await settle();
  assert.equal(name.getAttribute('aria-invalid'),'true');assert.equal(name.getAttribute('aria-describedby'),'input-action-error');assert.ok(target.querySelector('#input-action-error')!.textContent!.includes('before renaming'));
  const remove=[...target.querySelectorAll('button')].find(b=>b.textContent==='Remove input topic')!;remove.click();await settle();assert.equal(remove.getAttribute('aria-describedby'),'input-action-error');assert.ok(target.querySelector('#input-action-error')!.textContent!.includes('before removing'));
 } finally {if(component)await unmount(component);target.remove();}
});
