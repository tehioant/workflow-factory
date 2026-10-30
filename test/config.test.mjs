import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolveConfigurations, validateConfiguration } from '../src/configuration.ts';
import { readSubscriptionAuth } from '../src/auth.ts';
import { definition } from './fixtures.mjs';
import { JsonStore } from '../src/store.ts';
import { handleRequest } from '../src/http.ts';
import { Window } from 'happy-dom';
import { mountEditor } from '../src/public/editor.ts';
import { constants } from 'node:fs';
const config = () => ({ version: 1, id: 'careful', name: 'Careful', model: 'gpt-5.4', reasoning: 'medium' });
test('configuration resolution captures each referenced nonsecret setting independently', () => {
 const d = definition(); d.blocks.forEach(b => b.configurationId = 'careful'); const c = config();
 const resolved = resolveConfigurations(d, [c]); c.model = 'changed';
 assert.equal(resolved.a.model, 'gpt-5.4'); assert.notEqual(resolved.a, resolved.b);
 assert.throws(() => resolveConfigurations(d, []), /configuration/);
});
test('configuration contract rejects arbitrary CLI settings or malformed model values', () => {
 assert.throws(() => validateConfiguration({ ...config(), model: '--host' }), /model/);
 assert.throws(() => validateConfiguration({ ...config(), apiKey: 'not-allowed' }), /Unsupported/);
 assert.throws(() => validateConfiguration({ ...config(), reasoning: 'unbounded' }), /reasoning/);
});
test('missing or keyring-only credentials produce a safe actionable error', async () => {
 let flags;
 const disk = { async open(path, options) { flags = options; throw new Error('secret material must not leak'); } };
 await assert.rejects(readSubscriptionAuth('/fake/auth.json', disk), error => /file-based ChatGPT login/.test(error.message) && !error.message.includes('secret material'));
 assert.ok(flags & constants.O_NONBLOCK); assert.ok(flags & constants.O_NOFOLLOW);
});
test('credential provider accepts subscription tokens only and closes its private file', async () => {
 let closed = false;
 const disk = { async open() { return { async stat() { return { isFile: () => true, size: 200, uid: process.getuid?.(), mode: 0o600 }; }, async readFile() { return JSON.stringify({ auth_mode: 'chatgpt', tokens: { access_token: 'fake-access', refresh_token: 'fake-refresh', id_token: 'fake-id', account_id: 'fake-account' } }); }, async close() { closed = true; } }; } };
 assert.equal(JSON.parse(await readSubscriptionAuth('/fake/auth.json', disk)).auth_mode, 'chatgpt'); assert.equal(closed, true);
 const apiDisk = { async open() { return { ...await disk.open(), async readFile() { return '{"OPENAI_API_KEY":"fake-api"}'; } }; } };
 await assert.rejects(readSubscriptionAuth('/fake/auth.json', apiDisk), /file-based ChatGPT login/);
});

test('configuration is atomically persisted through the shared private-store boundary', async () => {
 let content; const calls = [];
 const disk = { async mkdir(path, options) { assert.equal(options.mode, 0o700); }, async open(path, flags, mode) { assert.equal(mode, 0o600); return { async writeFile(text) { content = text; }, async sync() { calls.push('sync'); }, async close() {} }; }, async rename() { calls.push('rename'); }, async unlink() {} };
 await new JsonStore('/fake/configs', validateConfiguration, disk).save(config());
 assert.deepEqual(JSON.parse(content), config()); assert.deepEqual(calls, ['sync', 'rename']);
});

test('configuration API writes only validated nonsecret settings to its injected store', async () => {
 const authority = { host: '127.0.0.1:4317', token: 'session' }; const calls = [];
 const request = new Request('http://127.0.0.1:4317/api/configurations', { method: 'PUT', headers: { host: authority.host, origin: `http://${authority.host}`, 'x-workflow-token': authority.token, 'content-type': 'application/json' }, body: JSON.stringify(config()) });
 const response = await handleRequest(request, authority, {}, { configurations: { async save(c) { calls.push(c); return c; } } });
 assert.equal(response.status, 200); assert.deepEqual(calls, [config()]);
});

test('block configuration selector stores a reusable reference rather than inline settings', async () => {
 const window = new Window(); const root = window.document.createElement('main'); window.document.body.append(root);
 const editor = mountEditor(root, definition(), { configurations: { async list() { return [config()]; } } });
 await new Promise(resolve => setImmediate(resolve));
 const select = root.querySelector('[aria-label^="Codex configuration"]'); select.value = 'careful'; select.dispatchEvent(new window.Event('change'));
 assert.equal(editor.snapshot().blocks[0].configurationId, 'careful'); assert.equal(editor.snapshot().blocks[0].model, undefined);
});
