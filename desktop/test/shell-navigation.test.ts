import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
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
    const appearance = target.querySelector('select#appearance') as unknown as import('happy-dom').HTMLSelectElement;
    assert.ok(appearance, 'Settings exposes a labelled Appearance selector');
    assert.equal(target.querySelector('label[for="appearance"]')?.textContent?.trim(), 'Appearance');
    assert.deepEqual([...appearance.options].map(option => option.textContent), ['Dark', 'Light']);
    assert.equal(appearance.value, 'dark');
    const appearanceCalls = [...calls];
    appearance.value = 'light'; appearance.dispatchEvent(new window.Event('change', { bubbles: true })); await settle();
    assert.equal(window.document.documentElement.dataset.theme, 'light');
    assert.equal(window.localStorage.getItem('workflow-factory.theme'), 'light');
    assert.deepEqual(calls, appearanceCalls, 'Appearance never invokes a domain command');
    appearance.value = 'dark'; appearance.dispatchEvent(new window.Event('change', { bubbles: true })); await settle();
    assert.equal(window.document.documentElement.dataset.theme, 'dark');
    const { initializeTheme } = await import('../src/lib/appearance.ts');
    const storage = window.localStorage;
    for (const saved of [null, 'light', 'dark', '{invalid', 'system']) {
      if (saved === null) storage.removeItem('workflow-factory.theme');
      else storage.setItem('workflow-factory.theme', saved);
      initializeTheme(); await settle();
      const expected = saved === 'light' ? 'light' : 'dark';
      assert.equal(appearance.value, expected, `Restored preference: ${saved}`);
      assert.equal(window.document.documentElement.dataset.theme, expected);
      appearance.value = 'light'; appearance.dispatchEvent(new window.Event('change', { bubbles: true })); await settle();
      initializeTheme(); await settle();
      assert.equal(appearance.value, 'light', 'Reopening restores the explicit choice');
    }
    const storageDescriptor = Object.getOwnPropertyDescriptor(window, 'localStorage');
    try {
      Object.defineProperty(window, 'localStorage', { configurable: true, get() { throw new Error('blocked'); } });
      initializeTheme(); await settle();
      assert.equal(appearance.value, 'dark');
      appearance.value = 'light'; appearance.dispatchEvent(new window.Event('change', { bubbles: true })); await settle();
      assert.equal(window.document.documentElement.dataset.theme, 'light', 'Storage failure cannot block switching');
    } finally {
      if (storageDescriptor) Object.defineProperty(window, 'localStorage', storageDescriptor);
      else delete (window as any).localStorage;
    }
    assert.deepEqual(calls, appearanceCalls, 'Every appearance variant is presentation-only');
    const { transpile } = await import('typescript');
    const bootstrap = transpile(readFileSync(new URL('../src/main.ts', import.meta.url), 'utf8').replace(/^import .*\n/gm, ''));
    for (const query of ['', '?signin']) {
      storage.setItem('workflow-factory.theme', 'light');
      window.document.documentElement.dataset.theme = 'dark';
      let mounted = 0;
      new Function('initializeTheme', 'mount', 'location', 'document', 'App', 'SignIn', bootstrap)(initializeTheme, (view: string) => {
        assert.equal(window.document.documentElement.dataset.theme, 'light', 'Common bootstrap restores before either content mount');
        assert.equal(view, query ? 'signin' : 'app'); mounted++;
      }, { href: `https://local.invalid/${query}` }, window.document, 'app', 'signin');
      assert.equal(mounted, 1);
    }
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
