import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolveInputs, validateDefinition } from '../src/definition.ts';
import { Window } from 'happy-dom';
import { mountRunInputs } from '../src/public/inputs.ts';
import { mountEditor } from '../src/public/editor.ts';
import { definition } from './fixtures.mjs';

test('one input resolution substitutes supplied values and defaults without changing the definition', () => {
 const d = { ...definition(), inputs: [
  { name: 'topic', type: 'text', required: true },
  { name: 'tone', type: 'text', required: false, default: 'brief' },
  { name: 'notes', type: 'text', required: false },
 ] };
 d.blocks[0].prompt = 'Write about {{inputs.topic}}: {{inputs.tone}} / {{inputs.notes}}';
 const before = structuredClone(d);
 assert.deepEqual(resolveInputs(d, { topic: 'Forests' }), {
  inputs: { topic: 'Forests', tone: 'brief', notes: '' },
  prompts: { a: 'Write about Forests: brief / ', b: 'Summarize this' },
 });
 assert.deepEqual(d, before);
});

test('required blanks and unknown/non-text supplied values are rejected', () => {
 const d = { ...definition(), inputs: [{ name: 'topic', type: 'text', required: true }] };
 assert.throws(() => resolveInputs(d, {}), /Required input: topic/);
 assert.throws(() => resolveInputs(d, { topic: '  ' }), /Required input/);
 assert.throws(() => resolveInputs(d, { topic: 42 }), /Invalid text/);
 assert.throws(() => resolveInputs(d, { extra: 'x' }), /Unknown workflow input/);
});

test('declarations reject duplicate names and dangling references while v1 without inputs stays readable', () => {
 assert.deepEqual(validateDefinition(definition()), definition());
 const input = { name: 'topic', type: 'text', required: false };
 assert.throws(() => validateDefinition({ ...definition(), inputs: [input, input] }), /duplicate input/);
 const d = definition(); d.blocks[0].prompt = '{{inputs.missing}}';
 assert.throws(() => validateDefinition(d), /Unknown workflow input: missing/);
 assert.throws(() => validateDefinition({ ...definition(), inputs: [{ ...input, name: 'bad.name' }] }), /Invalid/);
 assert.throws(() => validateDefinition({ ...definition(), inputs: [{ ...input, name: undefined }] }), /Invalid/);
 assert.throws(() => validateDefinition({ ...definition(), inputs: [{ ...input, name: true }] }), /Invalid/);
});

test('run values remain literal rather than recursively resolving inserted references', () => {
 const d = { ...definition(), inputs: [{ name: 'topic', type: 'text', required: true }] };
 d.blocks[0].prompt = '{{inputs.topic}}';
 assert.equal(resolveInputs(d, { topic: '{{inputs.other}} $&' }).prompts.a, '{{inputs.other}} $&');
});

test('reference picker inserts a token at the caret without copying the default', () => {
 const window = new Window(); const root = window.document.createElement('main');
 const d = { ...definition(), inputs: [{ name: 'topic', type: 'text', required: false, default: 'Secret forest' }] };
 const editor = mountEditor(root, d, {});
 const prompt = root.querySelector('textarea[aria-label^="Prompt"]'); prompt.setSelectionRange(0, 0);
 const picker = root.querySelector('[data-reference]'); picker.value = 'topic'; picker.dispatchEvent(new window.Event('change'));
 assert.equal(editor.snapshot().blocks[0].prompt, '{{inputs.topic}}Summarize this');
 assert.equal(d.blocks[0].prompt, 'Summarize this');
});

test('renaming a block whose ID matches an input does not relabel that input reference', () => {
 const window = new Window(); const root = window.document.createElement('main');
 const d = { ...definition(), inputs: [{ name: 'a', type: 'text', required: false }] };
 mountEditor(root, d, {});
 const name = root.querySelector('[aria-label="Block name"]'); name.value = 'Draft'; name.dispatchEvent(new window.Event('input'));
 assert.equal(root.querySelector('[data-reference] option[value="a"]').textContent, 'a');
});

test('run form blocks a missing required input before the launch boundary', async () => {
 const window = new Window(); const root = window.document.createElement('main'); let calls = 0;
 const d = { ...definition(), inputs: [{ name: 'topic', type: 'text', required: true }] };
 const form = mountRunInputs(root, d, async () => { calls++; });
 await form.onsubmit(new window.Event('submit'));
 assert.equal(calls, 0); assert.match(root.textContent, /Required input: topic/);
});

test('run form resolves default text and calls only the injected launch boundary', async () => {
 const window = new Window(); const root = window.document.createElement('main'); const calls = [];
 const d = { ...definition(), inputs: [{ name: 'tone', type: 'text', required: false, default: 'brief' }] };
 const form = mountRunInputs(root, d, async (...args) => { calls.push(args); });
 await form.onsubmit(new window.Event('submit'));
 assert.deepEqual(calls, [[d, { tone: 'brief' }]]); assert.notEqual(calls[0][0], d);
 assert.match(root.textContent, /Run accepted/);
});

test('input declaration editor updates defaults and required status in its working copy', () => {
 const window = new Window(); const root = window.document.createElement('main');
 const d = definition(); const editor = mountEditor(root, d, {});
 root.querySelector('[data-add-input]').click();
 const name = root.querySelector('[aria-label="Input name"]'); name.value = 'topic'; name.dispatchEvent(new window.Event('input'));
 const required = root.querySelector('[type="checkbox"]'); required.checked = true; required.dispatchEvent(new window.Event('change'));
 const defaults = root.querySelector('[aria-label="Default text"]'); defaults.value = 'Forest'; defaults.dispatchEvent(new window.Event('input'));
 assert.deepEqual(editor.snapshot().inputs, [{ name: 'topic', type: 'text', required: true, default: 'Forest' }]);
 assert.equal(d.inputs, undefined);
});
