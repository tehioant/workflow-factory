import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { Window } from 'happy-dom';
import { compileComponent } from './compile-component.ts';
import { createCommands } from '../src/lib/commands.ts';

test('Settings capacity refusal retains entered limits without queue mutation or polling', async () => {
  const window = new Window();
  for (const key of ['document', 'Node', 'Element', 'HTMLElement', 'Text', 'Comment', 'Event', 'MouseEvent', 'HTMLMediaElement']) {
    Object.defineProperty(globalThis, key, { configurable: true, value: (window as any)[key] });
  }
  Object.defineProperty(globalThis, 'window', { configurable: true, value: window });
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: window.navigator });
  const { mount, unmount, flushSync } = await import(new URL('../node_modules/svelte/src/index-client.js', import.meta.url).href);
  const code = await compileComponent(new URL('../src/lib/QueueCapacity.svelte', import.meta.url));
  const Component = (await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64'))).default;
  const calls: any[] = [];
  let rejectSave: (reason: unknown) => void;
  const commands = createCommands((async (command: string, args: any) => {
    calls.push({ command, args });
    if (command === 'queue_status') return { settings: { active: 4, waiting: 16 }, occupied: 3, waiting: 1, entries: [] };
    assert.equal(command, 'save_queue_settings', 'unrelated mutation forbidden');
    return new Promise((_resolve, reject) => { rejectSave = reject; });
  }) as any);
  const target = window.document.createElement('main');
  window.document.body.append(target);
  let component: any;
  const settle = async () => { await new Promise(resolve => setImmediate(resolve)); flushSync(); };
  try {
    flushSync(() => { component = mount(Component, { target, props: { commands } }); });
    await settle();
    const [input, waiting] = target.querySelectorAll('input');
    assert.equal(input.value, '4'); assert.equal(waiting.value, '16');
    assert.equal(target.querySelector('label')!.htmlFor, input.id);
    input.value = '1'; input.dispatchEvent(new window.Event('input', { bubbles: true })); flushSync();
    const save = target.querySelector('button')!;
    assert.equal(save.textContent, 'Save capacities');
    save.click(); flushSync();
    assert.deepEqual(calls, [{ command: 'queue_status', args: undefined }, { command: 'save_queue_settings', args: { settings: { active: 1, waiting: 16 } } }]);
    assert.ok(save.disabled); assert.ok(input.disabled); save.click();
    assert.equal(calls.length, 2);
    rejectSave!({ message: 'Capacity cannot be reduced below occupied or waiting counts.' });
    await settle();
    assert.ok(target.textContent.includes('cannot be reduced'));
    assert.equal(input.value, '1'); assert.equal(waiting.value, '16');
    assert.equal(save.disabled, false); assert.equal(calls.length, 2);
    await settle(); assert.equal(input.value, '1'); assert.equal(calls.length, 2, 'Settings does not poll or retry a failed save');
    const app = await readFile(new URL('../src/App.svelte', import.meta.url), 'utf8');
    assert.match(app, /destination === 'settings'[\s\S]*<QueueCapacity \{commands\} \/>/);
    await unmount(component); component = null;
    assert.equal(target.textContent, '');
  } finally {
    if (component) await unmount(component);
    target.remove(); window.happyDOM.abort();
  }
});
