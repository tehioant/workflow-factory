import { test } from 'node:test';
import assert from 'node:assert/strict';
import { DockerRuntime, blockResources, removeResources, agentArguments } from '../src/runtime.ts';
import { validateDefinition } from '../src/definition.ts';
import { definition } from './fixtures.mjs';
import { resolveBlockPrompt } from '../src/flow.ts';
import { Window } from 'happy-dom';
import { mountEditor } from '../src/public/editor.ts';
const base = { id: 'r1', agent: 'wf-r1-agent', network: 'wf-r1-private', egress: 'wf-r1-egress', proxy: 'wf-r1-proxy', workspace: '/private/r1/workspace', credentials: '/private/r1/credentials', image: 'sha256:fake', uid: '1000:1000' };
test('block isolation changes only agent identity while workflow isolation shares one agent', () => {
 const a = blockResources({ ...base, isolation: 'block' }, 'a'), b = blockResources({ ...base, isolation: 'block' }, 'b');
 assert.notEqual(a.agent, b.agent); assert.equal(a.workspace, b.workspace); assert.equal(a.network, b.network);
 assert.equal(blockResources(base, 'a').agent, blockResources(base, 'b').agent);
 assert.equal(agentArguments(a).filter(x => x.startsWith('type=bind'))[0], agentArguments(b).filter(x => x.startsWith('type=bind'))[0]);
 const other = blockResources({ ...base, id: 'r2', workspace: '/private/r2/workspace', isolation: 'block' }, 'a'); assert.notEqual(other.workspace, a.workspace); assert.notEqual(other.agent, a.agent);
});
test('cleanup targets owned block agents and helper without deleting workspace or retained output', async () => {
 const calls = [];
 await removeResources({ ...base, agents: ['wf-r1-block-a','wf-r1-block-b'] }, async args => { calls.push(args); return { code: 0 }; }, async path => calls.push(['credentials',path]));
 assert.deepEqual(calls.slice(0,3), [['rm','-f','wf-r1-block-a'],['rm','-f','wf-r1-block-b'],['rm','-f','wf-r1-proxy']]);
 assert.ok(!calls.flat().includes(base.workspace));
 const d = definition(); d.blocks[1].prompt = '{{blocks.a.final}}';
 assert.equal(resolveBlockPrompt({ definition: d, blocks: { a: { status: 'succeeded', output: 'Retained after removal' } } }, d.blocks[1]), 'Retained after removal');
});
test('workflow-only isolation setting preserves old definitions and refuses unknown modes', () => {
 assert.equal(validateDefinition(definition()).isolation, undefined);
 assert.equal(validateDefinition({ ...definition(), isolation: 'block' }).isolation, 'block');
 assert.throws(() => validateDefinition({ ...definition(), isolation: 'unrestricted' }), /isolation/);
 const mixed = definition(); mixed.blocks[0].isolation = 'block'; assert.throws(() => validateDefinition(mixed), /workflow level/);
});

test('workflow properties expose one isolation setting and update only the working definition', () => {
 const window = new Window(); const root = window.document.createElement('main'); const d = definition(); const editor = mountEditor(root, d, {});
 const select = root.querySelector('[data-isolation]'); select.value = 'block'; select.dispatchEvent(new window.Event('change'));
 assert.equal(editor.snapshot().isolation, 'block'); assert.equal(d.isolation, undefined); assert.equal(root.querySelectorAll('[data-isolation]').length, 1);
});

test('a single block adapter preserves captured output if its container removal fails', async () => {
 const r = { ...base, isolation: 'block', agents: [] }; const calls = [];
 const stream = '{"type":"item.completed","item":{"type":"agent_message","text":"Retained"}}\n{"type":"turn.completed"}';
 const runtime = new DockerRuntime('/fake/resources', async args => { calls.push(args); return args[0] === 'rm' ? { code: 1, stderr: 'daemon unavailable' } : { code: 0, stdout: stream, stderr: '' }; }, async () => '{"tokens":{"access_token":"fake-credential"}}');
 runtime.resources.set(r.id, r); runtime.startAgent = async agent => { assert.equal(agent.agent, 'wf-r1-block-a'); };
 const result = await runtime.execute(r, 'One prompt', { version: 1, id: 'c', name: 'C', model: 'gpt-5.4', reasoning: 'low' }, 'a');
 assert.equal(result.output, 'Retained'); assert.match(result.error, /cleanup failed/); assert.deepEqual(r.agents, ['wf-r1-block-a']);
 assert.deepEqual(calls.at(-1), ['rm','-f','wf-r1-block-a']);
});
