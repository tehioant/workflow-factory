import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { compileComponent } from './compile-component.ts';

const moduleUrl = (source: string) => 'data:text/javascript;base64,' + Buffer.from(source).toString('base64');

test('configuration navigation exposes its destination without a domain mutation', async () => {
  const window = new Window();
  for (const key of ['document', 'Node', 'Element', 'HTMLElement', 'Text', 'Comment', 'Event', 'MouseEvent', 'HTMLMediaElement', 'MutationObserver']) {
    Object.defineProperty(globalThis, key, { configurable: true, value: (window as any)[key] });
  }
  Object.defineProperty(globalThis, 'window', { configurable: true, value: window });
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: window.navigator });
  const { mount, unmount, flushSync } = await import(new URL('../node_modules/svelte/src/index-client.js', import.meta.url).href);
  const calls: string[] = [];
  (globalThis as any).__canvasShellCommands = new Proxy({}, {
    get(_target, name: string) {
      if (['list', 'listConfigurations'].includes(name)) return async () => { calls.push(name); return []; };
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
  } finally {
    if (component) await unmount(component);
    delete (globalThis as any).__canvasShellCommands;
    window.happyDOM.abort();
  }
});
