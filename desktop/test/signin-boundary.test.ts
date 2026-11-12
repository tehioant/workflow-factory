import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { compile } from 'svelte/compiler';
import { readFile, writeFile, mkdtemp, rm } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

test('failed setup start leaves the mounted dedicated cancellation boundary operable', async () => {
  const window = new Window();
  for (const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','MouseEvent','HTMLMediaElement']) Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});
  Object.defineProperty(globalThis,'window',{configurable:true,value:window});
  Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
  const { mount, unmount, flushSync } = await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
  const filename = new URL('../src/lib/SignIn.svelte', import.meta.url);
  const compiled = compile(await readFile(filename,'utf8'),{filename:filename.pathname,generate:'client'}).js.code;
  const code = compiled.replace(/import (['"])([^'"]+)\1/g, (_all, _quote, specifier) => `import ${JSON.stringify(import.meta.resolve(specifier))}`).replace(/from (['"])([^'"]+)\1/g, (_all, _quote, specifier) => `from ${JSON.stringify(specifier==='svelte'?new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href:specifier.startsWith('.') ? new URL(specifier,filename).href : import.meta.resolve(specifier))}`);
  const directory = await mkdtemp(join(tmpdir(), 'signin-boundary-'));
  let component: any; let calls=0;
  try {
    const modulePath = join(directory,'component.mjs'); await writeFile(modulePath,code);
    const Component = (await import(pathToFileURL(modulePath).href)).default;
    const target=window.document.createElement('main'); window.document.body.append(target);
    const commands={begin:async()=>{calls++;return {state:'cleanupRequired',url:null,code:null,message:'Start failed; owned resources held.'};},read:async()=>({state:'idle',url:null,code:null,message:'No pending login.'}),cancel:async()=>{throw Error('not a journey');},retire:async()=>{throw Error('not a journey');}};
    flushSync(()=>{component=mount(Component,{target,props:{commands}});});
    await Promise.resolve(); await Promise.resolve(); flushSync();
    assert.equal(calls,0);
    const begin=Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Begin'))!;
    begin.click(); await Promise.resolve(); await Promise.resolve(); flushSync();
    assert.equal(calls,1);
    assert.ok(target.textContent.includes('cleanupRequired'));
    const cancel=Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Cancel setup'))!;
    assert.equal(cancel.disabled,false);
    assert.equal(Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Retire'))!.disabled,true);
    await unmount(component);
    commands.read=async()=>({state:'pending',url:null,code:null,message:'Previously pending login.'});
    flushSync(()=>{component=mount(Component,{target,props:{commands}});});
    await Promise.resolve(); await Promise.resolve(); flushSync();
    assert.equal(calls,1);
    assert.equal(Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Cancel setup'))!.disabled,false);
    assert.ok(target.textContent.includes('Previously pending login.'));
  } finally { if(component) await unmount(component); await rm(directory,{recursive:true,force:true}); window.happyDOM.abort(); }
});
