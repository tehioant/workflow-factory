import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { compileComponent } from './compile-component.ts';

const moduleUrl = (source: string) => 'data:text/javascript;base64,' + Buffer.from(source).toString('base64');

test('configuration navigation exposes its destination without a domain mutation', async () => {
  const window = new Window();
  for (const key of ['document', 'Node', 'Element', 'HTMLElement', 'Text', 'Comment', 'Event', 'MouseEvent', 'HTMLMediaElement', 'MutationObserver', 'KeyboardEvent']) {
    Object.defineProperty(globalThis, key, { configurable: true, value: (window as any)[key] });
  }
  Object.defineProperty(globalThis, 'window', { configurable: true, value: window });
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: window.navigator });
  const { mount, unmount, flushSync } = await import(new URL('../node_modules/svelte/src/index-client.js', import.meta.url).href);
  const calls: string[] = [];
  (globalThis as any).__canvasShellCommands = new Proxy({}, {
    get(_target, name: string) {
      if (['list', 'listConfigurations'].includes(name)) return async () => { calls.push(name); return []; };
      if (name === 'queueStatus') return async () => ({settings:{active:2,waiting:16},occupied:0,waiting:0,entries:[]});
      if (name === 'saveQueueSettings') return async (value:unknown) => { calls.push(JSON.stringify(value)); };
      if (name === 'runStatus') return async () => { calls.push(name); return null; };
      return () => { assert.fail(`Unexpected domain command: ${name}`); };
    },
  });
  const overrides = new Map<string, string>();
  overrides.set('./lib/commands.ts', moduleUrl('export const createCommands=()=>globalThis.__canvasShellCommands;'));
  overrides.set('@tauri-apps/api/event', moduleUrl('export const listen=async()=>()=>{};'));
  overrides.set('@tauri-apps/api/core', moduleUrl('export const invoke=()=>{throw Error("Native invocation forbidden");};export class Channel {}'));
  const unrelated = moduleUrl('export default function(){return {open(){}};}');
  for (const name of ['RunHistory', 'QueuePanel', 'RunPanel', 'RunWorkspace', 'AuthReadiness', 'Canvas', 'InputForm', 'RuntimeSettings']) {
    overrides.set(`./lib/${name}.svelte`, unrelated);
  }
  const App = (await import(moduleUrl(await compileComponent(new URL('../src/App.svelte', import.meta.url), overrides)))).default;
  const target = window.document.createElement('main');
  window.document.body.append(target);
  let component: any;
  const settle = async () => { for (let i = 0; i < 8; i++) await Promise.resolve(); flushSync(); };
  try {
    flushSync(() => { component = mount(App, { target }); });
    await settle();
    const trigger = [...target.querySelectorAll('button')].find(button => button.getAttribute('aria-label') === 'Configurations' || button.textContent?.trim() === 'Configurations');
    assert.ok(trigger, 'The shell provides a labelled Configurations destination');
    const before = [...calls];
    trigger.click();
    await settle();
    assert.equal(trigger.getAttribute('aria-current'), 'page');
    const destination = target.querySelector('[aria-label="Reusable Codex configurations"]');
    assert.ok(destination, 'The real configuration manager is reachable');
    assert.equal(destination.closest('[hidden]'), null);
    assert.deepEqual(calls, before, 'Destination selection is presentation-only');
    [...target.querySelectorAll('button')].find(b=>b.getAttribute('aria-label')==='Settings')!.click();await settle();
    assert.equal(target.querySelectorAll('#queue-active').length,1);
    const capacity=target.querySelector('#queue-active') as HTMLInputElement;capacity.value='3';capacity.dispatchEvent(new window.Event('input',{bubbles:true}));await settle();
    [...target.querySelectorAll('button')].find(b=>b.textContent==='Save capacities')!.click();await settle();
    assert.equal(calls.at(-1),JSON.stringify({active:3,waiting:16}));
    // The action starts on a detached fragment, as a fresh Svelte overlay does.
    const {focusScope}=await import('../src/lib/editor-presentation.ts');
    trigger.focus();const overlay=window.document.createElement('div'),first=window.document.createElement('button'),last=window.document.createElement('button');overlay.append(first,last);
    for(const b of [first,last])b.getClientRects=()=>overlay.isConnected?[{}] as any:[] as any;
    let closed=0;const scope=focusScope(overlay as any,{enabled:true,close:()=>closed++});target.append(overlay);await settle();
    assert.equal(window.document.activeElement,first);
    last.focus();last.dispatchEvent(new window.KeyboardEvent('keydown',{key:'Tab',bubbles:true,cancelable:true}));assert.equal(window.document.activeElement,first);
    first.dispatchEvent(new window.KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));assert.equal(closed,1);
    scope.destroy();overlay.remove();assert.equal(window.document.activeElement,trigger);
  } finally {
    if (component) await unmount(component);
    delete (globalThis as any).__canvasShellCommands;
    window.happyDOM.abort();
  }
});
