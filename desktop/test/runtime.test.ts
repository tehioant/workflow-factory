import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { compile } from 'svelte/compiler';
import { readFile, writeFile, mkdtemp, rm } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createCommands } from '../src/lib/commands.ts';

test('manual runtime actions invoke only narrow commands and distinguish runtime checks from authorization', async () => {
  const calls: unknown[] = [];
  let readiness = { status: 'missing', message: 'Install explicitly', imageId: null as string | null, isolation: 'notChecked', executionReady: false };
  const plan = { program: '/usr/bin/docker', argv: ['--host', 'unix:///var/run/docker.sock', 'build'], instructions: 'Review first' };
  const commands = createCommands(async <T>(command: string, args?: Record<string, unknown>) => {
    assert.ok(['runtime_readiness', 'runtime_setup_plan'].includes(command), 'no authentication, security or execution effects');
    calls.push({ command, args });
    return (command === 'runtime_readiness' ? readiness : plan) as T;
  });
  assert.deepEqual(calls, []);
  assert.deepEqual(await commands.runtimeReadiness(), readiness);
  assert.deepEqual(await commands.runtimeSetupPlan(), plan);
  assert.deepEqual(calls, [{ command: 'runtime_readiness', args: undefined }, { command: 'runtime_setup_plan', args: undefined }]);
  calls.length = 0;

  const window = new Window();
  for (const key of ['document', 'Node', 'Element', 'HTMLElement', 'Text', 'Comment', 'Event', 'MouseEvent', 'HTMLMediaElement']) Object.defineProperty(globalThis, key, { configurable: true, value: (window as any)[key] });
  Object.defineProperty(globalThis, 'window', { configurable: true, value: window });
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: window.navigator });
  const { mount, unmount, flushSync } = await import(new URL('../node_modules/svelte/src/index-client.js', import.meta.url).href);
  const filename = new URL('../src/lib/RuntimeSettings.svelte', import.meta.url);
  const compiled = compile(await readFile(filename, 'utf8'), { filename: filename.pathname, generate: 'client' }).js.code;
  const code = compiled.replace(/import (['"])([^'"]+)\1/g, (_all, _quote, specifier) => `import ${JSON.stringify(import.meta.resolve(specifier))}`).replace(/from (['"])([^'"]+)\1/g, (_all, _quote, specifier) => `from ${JSON.stringify(specifier.startsWith('.') ? new URL(specifier, filename).href : import.meta.resolve(specifier))}`);
  const directory = await mkdtemp(join(tmpdir(), 'runtime-settings-'));
  let component: any;
  const settle = async () => { await new Promise(resolve => setImmediate(resolve)); flushSync(); };
  try {
    const modulePath = join(directory, 'component.mjs'); await writeFile(modulePath, code);
    const Component = (await import(pathToFileURL(modulePath).href)).default;
    const target = window.document.createElement('main'); window.document.body.append(target);
    flushSync(() => { component = mount(Component, { target, props: { commands } }); });
    await settle();
    assert.deepEqual(calls, [], 'mount must not probe Docker/authentication/security');
    assert.doesNotMatch(target.textContent, /Execution unavailable|Execution remains auth-blocked/);
    assert.match(target.textContent, /does not authorize execution/i);
    const check = Array.from(target.querySelectorAll('button')).find(button => button.textContent.includes('Check local Docker readiness'))!;
    readiness = { status: 'authUnsupported', message: 'Synthetic runtime observation; no credentials read.', imageId: 'sha256:' + 'a'.repeat(64), isolation: 'expectedNotMeasured', executionReady: false };
    check.click(); await settle();
    const status = target.querySelector('[role="status"]')?.textContent ?? '';
    assert.match(status, /Docker\/image prerequisites checked/);
    assert.match(status, /authentication and security not checked/i);
    assert.doesNotMatch(status, /authUnsupported|Execution unavailable/);
    assert.ok(status.includes(readiness.message));
    assert.ok(target.textContent.includes(readiness.imageId!));
    assert.match(target.textContent, /expectedNotMeasured/);
    assert.equal(Array.from(target.querySelectorAll('button')).some(button => /Run workflow/.test(button.textContent) && !button.disabled), false);
    assert.deepEqual(calls, [{ command: 'runtime_readiness', args: undefined }]);

    readiness = { ...readiness, status: 'stopped', message: 'Start Docker Desktop on macOS and check socket permissions.', imageId: null, isolation: 'notChecked' };
    check.click(); await settle();
    assert.match(target.querySelector('[role="status"]')?.textContent ?? '', /stopped: Start Docker Desktop on macOS and check socket permissions/);
    const showPlan = Array.from(target.querySelectorAll('button')).find(button => button.textContent.includes('Show setup instructions'))!;
    showPlan.click(); await settle();
    assert.ok(target.textContent.includes(plan.instructions));
    assert.equal(target.querySelector('pre')?.textContent, JSON.stringify(plan.argv, null, 2));
    assert.deepEqual(calls, [{ command: 'runtime_readiness', args: undefined }, { command: 'runtime_readiness', args: undefined }, { command: 'runtime_setup_plan', args: undefined }]);
  } finally {
    if (component) await unmount(component);
    await rm(directory, { recursive: true, force: true });
    window.happyDOM.abort();
  }
});
