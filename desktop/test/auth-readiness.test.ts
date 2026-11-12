import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { compile } from 'svelte/compiler';
import { readFile, writeFile, mkdtemp, rm } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

test('manual credential inspection does not run at mount and keeps execution blocked', async () => {
  const window = new Window();
  for (const key of ['document','Node','Element','HTMLElement','Text','Comment','Event','MouseEvent','HTMLMediaElement']) Object.defineProperty(globalThis,key,{configurable:true,value:(window as any)[key]});
  Object.defineProperty(globalThis,'window',{configurable:true,value:window});
  Object.defineProperty(globalThis,'navigator',{configurable:true,value:window.navigator});
  const { mount, unmount, flushSync } = await import(new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href);
  const filename = new URL('../src/lib/AuthReadiness.svelte', import.meta.url);
  const compiled = compile(await readFile(filename,'utf8'),{filename:filename.pathname,generate:'client'}).js.code;
  const code = compiled.replace(/import (['"])([^'"]+)\1/g, (_all, _quote, specifier) => `import ${JSON.stringify(import.meta.resolve(specifier))}`).replace(/from (['"])([^'"]+)\1/g, (_all, _quote, specifier) => `from ${JSON.stringify(specifier.startsWith('.') ? new URL(specifier,filename).href : import.meta.resolve(specifier))}`);
  const directory = await mkdtemp(join(tmpdir(), 'auth-readiness-'));
  let component: any; let calls=0;
  try {
    const modulePath = join(directory,'component.mjs'); await writeFile(modulePath,code);
    const Component = (await import(pathToFileURL(modulePath).href)).default;
    const target=window.document.createElement('main'); window.document.body.append(target);
    const commands={authReadiness:async()=>{calls++;return {executionEnabled:false,credentialSource:'codexFile',credentialAvailability:'available',status:'unsupportedConcurrentAuth',guidance:'Subscription-shaped material available; not authenticated. Container staging and concurrent refresh unsupported.'};}};
    flushSync(()=>{component=mount(Component,{target,props:{commands}});});
    assert.equal(calls,0);
    const check=Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Check Codex'));
    assert.ok(check,'explicit manual inspection control');
    check.click(); await Promise.resolve(); await Promise.resolve(); flushSync();
    assert.equal(calls,1);assert.ok(target.textContent.includes('available'));assert.ok(target.textContent.includes('unsupported'));
    assert.equal(Array.from(target.querySelectorAll('button')).find(b=>b.textContent.includes('Run workflow'))!.disabled,true);
  } finally { if(component) await unmount(component); await rm(directory,{recursive:true,force:true}); window.happyDOM.abort(); }
});
