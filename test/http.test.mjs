import { test } from 'node:test';
import assert from 'node:assert/strict';
import { authorize, handleRequest } from '../src/http.ts';
import { definition } from './fixtures.mjs';
test('one API save forwards an explicit definition to a fake store', async () => {
 const writes = [];
 const store = { async save(d) { writes.push(d); return d; }, async list() { throw new Error('unexpected list'); }, async get() { throw new Error('unexpected get'); } };
 const request = new Request('http://127.0.0.1:4317/api/definitions/workflow-1', { method: 'PUT', headers: { host: authority.host, origin: 'http://127.0.0.1:4317', 'x-workflow-token': authority.token, 'content-type': 'application/json' }, body: JSON.stringify(definition()) });
 const response = await handleRequest(request, authority, store);
 assert.equal(response.status, 200); assert.deepEqual(writes, [definition()]); assert.deepEqual(await response.json(), definition());
});
const authority = { host: '127.0.0.1:4317', token: 'test-session' };
test('local API refuses foreign hosts, origins and missing session authority', () => {
 const valid = { host: authority.host, origin: 'http://127.0.0.1:4317', 'x-workflow-token': authority.token };
 assert.equal(authorize(new Request('http://127.0.0.1:4317/api/definitions', { headers: valid }), authority), true);
 assert.equal(authorize(new Request('http://127.0.0.1:4317/api/definitions', { headers: { ...valid, host: 'evil.test' } }), authority), false);
 assert.equal(authorize(new Request('http://127.0.0.1:4317/api/definitions', { headers: { ...valid, origin: 'https://evil.test' } }), authority), false);
 assert.equal(authorize(new Request('http://127.0.0.1:4317/api/definitions', { headers: { host: authority.host } }), authority), false);
});
