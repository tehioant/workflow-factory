import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { mountHome, mountEditor } from '../src/public/editor.ts';
import { definition } from './fixtures.mjs';
test('renaming a block immediately refreshes connection choices and labels', () => {
 const { root, api, window } = setup(); const editor = mountEditor(root, definition(), api);
 const name = root.querySelector('[aria-label="Block name"]'); name.value = 'Draft'; name.dispatchEvent(new window.Event('input'));
 assert.equal(editor.snapshot().blocks[0].name, 'Draft');
 assert.equal(root.querySelector('[aria-label="From block"] option').textContent, 'Draft');
 assert.match(root.querySelector('.edges').textContent, /Draft → Block b/);
});

test('removing a block also removes its connected edges', () => {
 const { root, api } = setup(); const editor = mountEditor(root, definition(), api);
 root.querySelector('[data-remove-block]').click();
 assert.deepEqual(editor.snapshot().blocks.map(b=>b.id), ['b']); assert.deepEqual(editor.snapshot().connections, []);
});

test('connecting two blocks establishes directional order on the canvas', () => {
 const { root, api, window } = setup(); const editor = mountEditor(root, { ...definition(), connections: [] }, api);
 root.querySelector('[aria-label="From block"]').value = 'a'; root.querySelector('[aria-label="To block"]').value = 'b';
 root.querySelector('[data-connect]').dispatchEvent(new window.Event('submit', { cancelable: true }));
 assert.deepEqual(editor.snapshot().connections, [{ from: 'a', to: 'b' }]);
 assert.equal(root.querySelectorAll('svg path[data-edge]').length, 1);
});

test('editing one AI prompt changes the definition without mutating its loaded record', () => {
 const { root, api, window } = setup(); const original = definition(); const editor = mountEditor(root, original, api);
 const prompt = root.querySelector('textarea'); prompt.value = 'Write a concise summary'; prompt.dispatchEvent(new window.Event('input'));
 assert.equal(editor.snapshot().blocks[0].prompt, 'Write a concise summary');
 assert.equal(original.blocks[0].prompt, 'Summarize this');
 const unload = new window.Event('beforeunload', { cancelable: true }); window.dispatchEvent(unload);
 assert.equal(unload.defaultPrevented, true);
});

test('save sends the current definition to a fake persistence boundary', async () => {
 const { root, api, saved } = setup(); mountEditor(root, definition(), api);
 root.querySelector('[data-save]').click(); await new Promise(resolve=>setImmediate(resolve));
 assert.deepEqual(saved, [definition()]); assert.match(root.querySelector('[role="status"]').textContent, /Saved/);
});

test('dragging one block handle updates its canvas position', () => {
 const { root, api, window } = setup(); mountEditor(root, definition(), api);
 const handle = root.querySelector('.handle');
 handle.dispatchEvent(new window.PointerEvent('pointerdown', { clientX: 50, clientY: 60, pointerId: 1, button: 0, bubbles: true }));
 handle.dispatchEvent(new window.PointerEvent('pointermove', { clientX: 150, clientY: 160, pointerId: 1, bubbles: true }));
 handle.dispatchEvent(new window.PointerEvent('pointerup', { pointerId: 1, bubbles: true }));
 assert.equal(root.querySelector('.block').style.left, '140px');
 assert.equal(root.querySelector('.block').style.top, '150px');
});

test('palette drop adds one AI block at the canvas drop position', () => {
 const { root, api, window } = setup(); mountEditor(root, { ...definition(), blocks: [], connections: [] }, api);
 const drop = new window.Event('drop', { bubbles: true, cancelable: true });
 Object.assign(drop, { clientX: 120, clientY: 90, dataTransfer: { getData: () => 'ai' } });
 root.querySelector('.canvas').dispatchEvent(drop);
 assert.equal(root.querySelectorAll('.block').length, 1);
 assert.equal(root.querySelector('.block').style.left, '120px');
 assert.equal(root.querySelector('textarea').value, '');
});

function setup() {
 const window = new Window(); const root = window.document.createElement('main'); window.document.body.append(root);
 const saved = []; const api = { async list() { return [definition()]; }, async save(d) { saved.push(structuredClone(d)); return d; } };
 return { root, window, api, saved };
}
test('home lists saved workflows and selecting one opens its editable canvas', async () => {
 const { root, api } = setup(); await mountHome(root, api);
 const open = root.querySelector('[data-workflow]'); assert.equal(open.textContent, 'Research'); open.click();
 assert.equal(root.querySelector('[aria-label="Workflow name"]').value, 'Research');
});
test('new workflow action opens a named empty definition', async () => {
 const { root, api } = setup(); await mountHome(root, api); root.querySelector('[data-create]').click();
 assert.equal(root.querySelector('[aria-label="Workflow name"]').value, 'Untitled workflow');
 assert.equal(root.querySelectorAll('.block').length, 0);
});
