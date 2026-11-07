import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { compile } from 'svelte/compiler';
import { readFile, writeFile, mkdtemp, rm } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

test('mounted input form ignores a preparation invalidated by definition changes', async () => {
  const window = new Window();
  for (const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','MouseEvent','HTMLMediaElement']) Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});
  Object.defineProperty(globalThis,'window',{configurable:true,value:window});
  Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
  const { mount, unmount, flushSync } = await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
  const { proxy } = await import('svelte/internal/client');
  const filename = new URL('../src/lib/InputForm.svelte', import.meta.url);
  const compiled = compile(await readFile(filename,'utf8'),{filename:filename.pathname,generate:'client'}).js.code;
  const code = compiled.replace(/import (['"])([^'"]+)\1/g, (_all, _quote, specifier) => `import ${JSON.stringify(import.meta.resolve(specifier))}`).replace(/from (['"])([^'"]+)\1/g, (_all, _quote, specifier) => `from ${JSON.stringify(specifier.startsWith('.') ? new URL(specifier,filename).href : import.meta.resolve(specifier))}`);
  const directory = await mkdtemp(join(tmpdir(), 'input-form-'));
  let component: any;
  try {
    const modulePath = join(directory,'component.mjs'); await writeFile(modulePath,code);
    const Component = (await import(pathToFileURL(modulePath).href)).default;
    let resolve: (value: unknown)=>void = ()=>{};
    const props = proxy({definition:{schemaVersion:1,id:'old',name:'Old',blocks:[],connections:[],inputs:[]},commands:{prepare:()=>new Promise(r=>{resolve=r;})}});
    const target=window.document.createElement('main'); window.document.body.append(target);
    flushSync(()=>{component=mount(Component,{target,props});});
    target.querySelector('form')!.dispatchEvent(new window.Event('submit',{bubbles:true,cancelable:true}));
    flushSync(()=>{props.definition={...props.definition,id:'new',name:'New'};});
    resolve({values:{},prompts:{old:'STALE PROMPT'}}); await Promise.resolve(); await Promise.resolve(); flushSync();
    assert.equal(target.textContent.includes('STALE PROMPT'),false);
    assert.equal(target.querySelector('button')!.disabled,false);
  } finally { if(component) await unmount(component); await rm(directory,{recursive:true,force:true}); window.happyDOM.abort(); }
});
