import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { compile } from 'svelte/compiler';
import { readFile, writeFile, mkdtemp, rm } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createCommands } from '../src/lib/commands.ts';

test('managed session metadata is explicit-only and never authorizes execution', async () => {
  const calls: string[] = [];
  const legacy = { executionEnabled: false, credentialSource: 'codexFile', credentialAvailability: 'available', status: 'unsupportedConcurrentAuth', guidance: 'Availability is not authentication.' };
  let metadata = { available: 2, withheld: 1, guidance: 'Synthetic managed sessions require native checks before use.' };
  let fail = false;
  const commands = createCommands(async <T>(command: string, args?: Record<string, unknown>) => {
    assert.equal(args, undefined);
    calls.push(command);
    if (command === 'auth_readiness') return legacy as T;
    assert.equal(command, 'managed_session_status', 'no sign-in, credential inspection or execution effects');
    if (fail) throw new Error('synthetic-private-backend-detail');
    return metadata as T;
  });
  assert.deepEqual(calls, []);
  assert.deepEqual(await commands.authReadiness(), legacy, 'legacy IPC remains available only on explicit request');
  assert.deepEqual(calls, ['auth_readiness']);
  calls.length = 0;

  const window = new Window();
  for (const key of ['document', 'Node', 'Element', 'HTMLElement', 'Text', 'Comment', 'Event', 'MouseEvent', 'HTMLMediaElement']) Object.defineProperty(globalThis, key, { configurable: true, value: (window as any)[key] });
  Object.defineProperty(globalThis, 'window', { configurable: true, value: window });
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: window.navigator });
  const { mount, unmount, flushSync } = await import(new URL('../node_modules/svelte/src/index-client.js', import.meta.url).href);
  const filename = new URL('../src/lib/AuthReadiness.svelte', import.meta.url);
  const compiled = compile(await readFile(filename, 'utf8'), { filename: filename.pathname, generate: 'client' }).js.code;
  const code = compiled.replace(/import (['"])([^'"]+)\1/g, (_all, _quote, specifier) => `import ${JSON.stringify(import.meta.resolve(specifier))}`).replace(/from (['"])([^'"]+)\1/g, (_all, _quote, specifier) => `from ${JSON.stringify(specifier.startsWith('.') ? new URL(specifier, filename).href : import.meta.resolve(specifier))}`);
  const directory = await mkdtemp(join(tmpdir(), 'auth-readiness-'));
  let component: any;
  const settle = async () => { await new Promise(resolve => setImmediate(resolve)); flushSync(); };
  try {
    const modulePath = join(directory, 'component.mjs'); await writeFile(modulePath, code);
    const Component = (await import(pathToFileURL(modulePath).href)).default;
    const target = window.document.createElement('main'); window.document.body.append(target);
    flushSync(() => { component = mount(Component, { target, props: { commands } }); });
    await settle();
    assert.deepEqual(calls, [], 'mount must not inspect host credentials or session metadata');
    const check = Array.from(target.querySelectorAll('button')).find(button => button.textContent.includes('Check managed sessions'));
    assert.ok(check, 'explicit metadata-only control');
    assert.match(target.textContent, /Open native managed sign-in/);
    assert.match(target.textContent, /not checked/i);
    assert.doesNotMatch(target.textContent, /~\/\.codex|not implemented|Execution unavailable/);
    check.click(); flushSync();
    assert.equal(check.disabled, true);
    check.click();
    await settle();
    assert.deepEqual(calls, ['managed_session_status']);
    assert.match(target.textContent, /Available: 2/);
    assert.match(target.textContent, /Withheld: 1/);
    assert.ok(target.textContent.includes(metadata.guidance));
    assert.match(target.textContent, /does not authorize execution/i);
    assert.equal(Array.from(target.querySelectorAll('button')).some(button => /Run workflow/.test(button.textContent) && !button.disabled), false);

    metadata = { available: 0, withheld: 2, guidance: 'Synthetic sessions withheld; use native managed sign-in for recovery.' };
    check.click(); await settle();
    assert.match(target.textContent, /Available: 0/);
    assert.match(target.textContent, /Withheld: 2/);
    assert.ok(target.textContent.includes(metadata.guidance));
    fail = true;
    check.click(); await settle();
    assert.match(target.querySelector('[role="alert"]')?.textContent ?? '', /Managed session status unavailable.*retry manually/i);
    assert.doesNotMatch(target.textContent, /Available: 0|Withheld: 2|Synthetic sessions withheld|synthetic-private-backend-detail/);
    assert.equal(check.disabled, false, 'manual retry remains available');
    assert.deepEqual(calls, ['managed_session_status', 'managed_session_status', 'managed_session_status']);
  } finally {
    if (component) await unmount(component);
    await rm(directory, { recursive: true, force: true });
    window.happyDOM.abort();
  }
});
