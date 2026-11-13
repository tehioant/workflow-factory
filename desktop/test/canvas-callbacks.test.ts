import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Window } from 'happy-dom';
import { fromFlow } from '../src/lib/graph.ts';
const source = readFileSync(new URL('../src/lib/Canvas.svelte',import.meta.url),'utf8');
// Execute the actual component callbacks with only their captured boundaries injected.
test('busy canvas sync cannot publish domain changes', () => {
  const body = source.match(/function sync\(\)\{([^\n]*)\}/)![1];
  let published=0;
  new Function('disabled','onchange','fromFlow','definition','nodes','edges',body)(true,()=>published++,fromFlow,{},[],[]);
  assert.equal(published,0);
});
test('selection wrapper arrow key publishes group positions', async () => {
  const window=new Window(); const wrapper=window.document.createElement('div'); wrapper.className='svelte-flow__selection-wrapper';
  const handler=source.match(/onkeyup=\{(.*?)\} \/>/)![1];
  let published=0;
  const callback=new Function('disabled','sync','Element',`return (${handler});`)(false,()=>published++,window.Element);
  callback({key:'ArrowRight',target:wrapper});
  assert.equal(published,1);
  for(const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','KeyboardEvent','MouseEvent','HTMLMediaElement','MutationObserver'])Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});
  Object.defineProperty(globalThis,'window',{configurable:true,value:window});
  const url=(s:string)=>'data:text/javascript;base64,'+Buffer.from(s).toString('base64');
  const {compileComponent}=await import('./compile-component.ts');
  const overrides=new Map([
    ['@xyflow/svelte',url('export function SvelteFlow(node,props){globalThis.__flow=props;} export function Background(){} export function Controls(){} export const BackgroundVariant={Dots:0},MarkerType={ArrowClosed:0};')],
    ['@xyflow/svelte/dist/style.css',url('')],['./AiNode.svelte',url('export default function(){}')]
  ]);
  const Component=(await import(url(await compileComponent(new URL('../src/lib/Canvas.svelte',import.meta.url),overrides)))).default;
  const {mount,unmount,flushSync}=await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
  const target=window.document.createElement('main');window.document.body.append(target);
  let component:any;
  try {
    flushSync(()=>component=mount(Component,{target,props:{definition:{id:'d',name:'D',schemaVersion:1,isolation:'perRun',blocks:[],connections:[],inputs:[]},onchange:()=>assert.fail('domain mutation forbidden'),onselect:()=>assert.fail('selection forbidden')}}));
    const canvas=target.querySelector('.canvas') as HTMLElement,flow=(globalThis as any).__flow;
    assert.equal(flow.panActivationKey,null);assert.deepEqual(flow.panOnDrag,[1,2]);
    const space=()=>new window.KeyboardEvent('keydown',{key:' ',code:'Space',bubbles:true,cancelable:true});
    const textarea=window.document.createElement('textarea');target.append(textarea);textarea.focus();textarea.dispatchEvent(space());flushSync();assert.deepEqual(flow.panOnDrag,[1,2]);
    canvas.focus();const key=space();canvas.dispatchEvent(key);flushSync();assert.equal(key.defaultPrevented,true);assert.equal(flow.panOnDrag,true);
    canvas.dispatchEvent(new window.KeyboardEvent('keyup',{code:'Space',bubbles:true}));flushSync();assert.deepEqual(flow.panOnDrag,[1,2]);
  } finally {if(component)await unmount(component);delete (globalThis as any).__flow;window.happyDOM.abort();}

});
