import { test } from 'node:test';
import assert from 'node:assert/strict';
import { DefinitionStore } from '../src/store.ts';
import { definition } from './fixtures.mjs';
const missing = () => Object.assign(new Error('missing'), { code: 'ENOENT' });
const fakeDisk = (seed) => {
  const files = new Map(seed ? [['/private/definitions/workflow-1.json', JSON.stringify(seed)]] : []);
  const calls = [];
  return { calls, files, async mkdir(...args) { calls.push(['mkdir', ...args]); }, async readFile(p) { if (!files.has(p)) throw missing(); return files.get(p); }, async readdir() { return [...files.keys()].map(p => p.split('/').at(-1)); }, async open(p, flags, mode) { calls.push(['open',p,flags,mode]); return { async writeFile(body) { files.set(p,body); }, async sync() { calls.push(['sync']); }, async close() {} }; }, async rename(a,b) { calls.push(['rename',a,b]); files.set(b,files.get(a)); files.delete(a); }, async unlink(p) { files.delete(p); } };
};
test('store saves complete definitions through a private atomic host write', async () => {
 const disk = fakeDisk(); await new DefinitionStore('/private/definitions', disk).save(definition());
 assert.deepEqual(JSON.parse(disk.files.get('/private/definitions/workflow-1.json')), definition());
 assert.equal(disk.calls.find(c=>c[0]==='open')[3], 0o600);
 assert.ok(disk.calls.findIndex(c=>c[0]==='sync') < disk.calls.findIndex(c=>c[0]==='rename'));
});
test('store reopens a supplied durable definition without losing properties', async () => {
 assert.deepEqual(await new DefinitionStore('/private/definitions', fakeDisk(definition())).get('workflow-1'), definition());
});
test('store lists saved definitions from mocked host storage', async () => {
 assert.deepEqual(await new DefinitionStore('/private/definitions', fakeDisk(definition())).list(), [definition()]);
});
