import { test } from 'node:test';
import assert from 'node:assert/strict';
import { acceptRun, beginBlock, admissionOccupied } from '../src/runs.ts';
import { DockerRuntime, dockerCommand, agentArguments, verifyPrivateNetwork, codexArguments, parseCodexOutput, removeResources, requireRuntime } from '../src/runtime.ts';
import { admittedTarget, clientHelloHostname, createEgressServer } from '../runtime/egress.mjs';
import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import { definition } from './fixtures.mjs';
import { Window } from 'happy-dom';
import { renderRun } from '../src/public/runs.ts';
const config = { version: 1, id: 'c', name: 'Config', model: 'gpt-5.4', reasoning: 'medium' };
const resources = { id: 'wf-123', network: 'wf-123-private', egress: 'wf-123-egress', agent: 'wf-123-agent', proxy: 'wf-123-proxy', workspace: '/private/wf-123/workspace', credentials: '/private/wf-123/credentials', image: 'sha256:abc', uid: '1000:1000' };
test('admission captures the full definition, input values and cloned nonsecret settings', () => {
 const d = definition(); d.blocks = [d.blocks[0]]; d.connections = []; d.blocks[0].configurationId = 'c';
 const run = acceptRun(d, {}, [config], 'run-1', '2026-10-30T00:00:00Z');
 d.blocks[0].prompt = 'later'; assert.equal(run.definition.blocks[0].prompt, 'Summarize this');
 assert.notEqual(run.configurations.a, config); assert.deepEqual(run.inputs, {});
});
test('agent argv enforces outer containment and mounts only the run workspace and credential staging', () => {
 const args = agentArguments(resources);
 assert.ok(args.includes('--read-only')); assert.ok(args.includes('--cap-drop=ALL')); assert.ok(args.includes('--security-opt=no-new-privileges'));
 assert.equal(args[args.indexOf('--network') + 1], resources.network);
 assert.ok(args.includes('--pull=never')); assert.ok(args.includes('--pids-limit=128')); assert.ok(args.includes('--memory=1g'));
 assert.deepEqual(args.filter(a => a.startsWith('type=bind')), [`type=bind,src=${resources.workspace},dst=/workspace`, `type=bind,src=${resources.credentials},dst=/run/codex`]);
 assert.ok(!args.some(a => /docker.sock|--privileged|network=host/.test(a)));
});
test('network inspection fails closed on ordinary bridge, IPv6 or missing isolation', () => {
 const value = { Driver: 'bridge', Internal: true, EnableIPv6: false, Options: { 'com.docker.network.bridge.gateway_mode_ipv4': 'isolated' } };
 verifyPrivateNetwork(value);
 assert.throws(() => verifyPrivateNetwork({ ...value, Internal: false }), /isolated/);
 assert.throws(() => verifyPrivateNetwork({ ...value, EnableIPv6: true }), /isolated/);
 assert.throws(() => verifyPrivateNetwork({ ...value, Options: {} }), /isolated/);
});
test('Codex command uses structured flags and stdin, never a shell or prompt argv', () => {
 const args = codexArguments(config); assert.ok(args.includes('--json')); assert.ok(args.includes('--ephemeral'));
 assert.ok(args.includes('danger-full-access')); assert.equal(args.at(-1), '-'); assert.ok(args.includes('model_reasoning_effort="medium"'));
});
test('result parser separates final agent message from diagnostics and refuses failed/malformed streams', () => {
 const events = [{ type: 'item.completed', item: { type: 'command_execution', aggregated_output: 'not the answer' } }, { type: 'item.completed', item: { type: 'agent_message', text: 'Final' } }, { type: 'turn.completed' }].map(JSON.stringify).join('\n');
 const result = parseCodexOutput(events, 0, 'sensitive stderr'); assert.equal(result.output, 'Final'); assert.ok(!result.logs.includes('sensitive stderr')); assert.ok(!result.logs.includes('not the answer'));
 const failed = parseCodexOutput(events, 1, ''); assert.match(failed.error, /Codex/); assert.equal(failed.output, 'Final'); assert.match(failed.logs, /turn.completed/);
 assert.match(parseCodexOutput('invalid', 0, '').error, /JSONL/);
 assert.match(parseCodexOutput('{"type":"turn.failed"}', 0, '').error, /Codex/);
});
test('scoped cleanup removes only supplied run containers and networks before credential removal', async () => {
 const calls = [];
 await removeResources(resources, async args => { calls.push(args); return { code: 0, stdout: '', stderr: '' }; }, async path => { calls.push(['credentials', path]); });
 assert.deepEqual(calls, [['rm', '-f', resources.agent], ['rm', '-f', resources.proxy], ['network', 'rm', resources.network], ['network', 'rm', resources.egress], ['credentials', resources.credentials]]);
});
test('missing or stopped runtime returns actionable failure without installer or pull', async () => {
 await assert.rejects(requireRuntime(async () => { throw new Error('private diagnostics'); }), /Docker.*running/);
});
test('egress accepts only exact AI TLS hosts and vetted public IPv4 destinations', async () => {
 const lookup = async () => [{ address: '104.18.32.7', family: 4 }];
 assert.deepEqual(await admittedTarget('chatgpt.com:443', lookup), { host: 'chatgpt.com', address: '104.18.32.7', port: 443 });
 await assert.rejects(admittedTarget('chatgpt.com.attacker.test:443', lookup));
 await assert.rejects(admittedTarget('chatgpt.com:80', lookup));
 await assert.rejects(admittedTarget('chatgpt.com:443', async () => [{ address: '127.0.0.1', family: 4 }]));
 await assert.rejects(admittedTarget('chatgpt.com:443', async () => [{ address: '169.254.169.254', family: 4 }]));
 await assert.rejects(admittedTarget('chatgpt.com:443', async () => [{ address: '192.88.99.1', family: 4 }]));
});

test('one execution adapter action sends stdin, redacts staged tokens and captures only final output', async () => {
 const calls = []; const events = [{ type: 'item.completed', item: { type: 'agent_message', text: 'Answer fake-token' } }, { type: 'turn.completed' }].map(JSON.stringify).join('\n');
 const runtime = new DockerRuntime('/fake/resources', async (...args) => { calls.push(args); return { code: 0, stdout: events, stderr: '' }; }, async () => JSON.stringify({ tokens: { access_token: 'fake-token' } }));
 runtime.resources.set(resources.id, resources);
 const result = await runtime.execute(resources, 'Own prompt', config);
 assert.equal(calls.length, 1); assert.equal(calls[0][1], 'Own prompt'); assert.equal(calls[0][0][4], resources.agent);
 assert.equal(result.output, 'Answer [credential redacted]');
});

test('run view renders supplied completed output and errors as text, not diagnostics or HTML', () => {
 const window = new Window(); const root = window.document.createElement('div');
 renderRun(root, { id: 'r', name: 'Run', status: 'failed', definition: definition(), blocks: { a: { status: 'succeeded', output: '<script>unsafe</script>', logs: 'turn.completed' } }, error: 'Runtime failed' });
 assert.equal(root.querySelector('[data-output]').textContent, '<script>unsafe</script>'); assert.equal(root.querySelector('script'), null);
 assert.match(root.textContent, /Runtime failed/); assert.equal(root.querySelector('[data-logs]').textContent, 'turn.completed');
});

test('cleanup failure never deletes credentials while owned containers may still be running', async () => {
 let removed = false;
 await assert.rejects(removeResources(resources, async () => ({ code: 1, stderr: 'Cannot connect to daemon', stdout: '' }), async () => { removed = true; }), /Could not stop/);
 assert.equal(removed, false);
});

test('pending or failed cleanup keeps admission occupied and prototype-like block IDs retain results', () => {
 const run = { status: 'running', blocks: {} };
 const result = beginBlock(run, '__proto__', 'Prompt'); result.status = 'succeeded'; result.output = 'Retained';
 assert.equal(JSON.parse(JSON.stringify(run)).blocks.__proto__.output, 'Retained');
 assert.equal(admissionOccupied(run), true);
 run.status = 'failed'; run.cleanupPending = true; assert.equal(admissionOccupied(run), true);
 run.cleanupPending = false; assert.equal(admissionOccupied(run), false);
});

function hello(host) {
 const name = Buffer.from(host); const entry = Buffer.concat([Buffer.from([0, name.length >> 8, name.length & 255]), name]);
 const list = Buffer.concat([Buffer.from([entry.length >> 8, entry.length & 255]), entry]);
 const ext = Buffer.concat([Buffer.from([0, 0, list.length >> 8, list.length & 255]), list]);
 const body = Buffer.concat([Buffer.from([3,3]), Buffer.alloc(32), Buffer.from([0,0,2,0x13,1,1,0,ext.length >> 8,ext.length & 255]), ext]);
 const handshake = Buffer.concat([Buffer.from([1, 0, body.length >> 8, body.length & 255]), body]);
 return Buffer.concat([Buffer.from([22,3,1,handshake.length >> 8,handshake.length & 255]), handshake]);
}
test('egress ClientHello gate binds TLS SNI and refuses non-TLS or absent hostnames', () => {
 assert.equal(clientHelloHostname(hello('chatgpt.com')), 'chatgpt.com');
 assert.equal(clientHelloHostname(hello('other.test')), 'other.test');
 assert.equal(clientHelloHostname(hello('chatgpt.com').subarray(0, 8)), undefined);
 assert.throws(() => clientHelloHostname(Buffer.from('GET / HTTP/1.1\r\n')), /TLS/);
});

test('CONNECT handler opens no upstream socket when TLS SNI differs from the admitted host', async () => {
 let connections = 0;
 const server = createEgressServer(async () => [{ address: '104.18.32.7', family: 4 }], () => { connections++; throw new Error('Upstream must not be opened'); });
 const client = new EventEmitter(); Object.assign(client, { setTimeout() {}, write() {}, end() {}, destroy() { this.destroyed = true; } });
 server.emit('connect', { url: 'chatgpt.com:443' }, client, hello('other.test'));
 await new Promise(resolve => setImmediate(resolve));
 assert.equal(connections, 0); assert.equal(client.destroyed, true);
});

test('Docker CLI children have isolated signal groups but remain referenced and explicitly abortable', async () => {
 const calls=[]; const controller=new AbortController();
 const command=dockerCommand('/fake/docker.sock',(...args)=>{
  calls.push(args);const child=new EventEmitter();Object.assign(child,{stdout:new PassThrough(),stderr:new PassThrough(),stdin:new PassThrough(),unref(){assert.fail('Owned clients must stay referenced');}});
  queueMicrotask(()=>child.emit('close',0));return child;
 });
 await command(['exec','owned-agent','codex'],undefined,1000,controller.signal);
 assert.equal(calls[0][0],'docker');assert.equal(calls[0][2].detached,true);assert.equal(calls[0][2].shell,false);assert.equal(calls[0][2].signal,controller.signal);assert.equal(calls[0][2].killSignal,'SIGKILL');
});
