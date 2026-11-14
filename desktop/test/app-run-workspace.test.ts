import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Window } from 'happy-dom';
import { compileComponent } from './compile-component.ts';

const moduleUrl = (source: string) => 'data:text/javascript;base64,' + Buffer.from(source).toString('base64');

test('App captured-canvas inspection preserves the exact draft and selection on return', async () => {
  const window = new Window();
  for (const key of ['document', 'Node', 'Element', 'HTMLElement', 'Text', 'Comment', 'Event', 'MouseEvent', 'HTMLMediaElement', 'MutationObserver']) Object.defineProperty(globalThis, key, { configurable: true, value: (window as any)[key] });
  Object.defineProperty(globalThis, 'window', { configurable: true, value: window });
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: window.navigator });
  const { mount, unmount, flushSync } = await import(new URL('../node_modules/svelte/src/index-client.js', import.meta.url).href);
  const draft = { schemaVersion: 1, id: 'workflow', name: 'Unsaved draft', isolation: 'perRun', blocks: [{ id: 'same', name: 'Draft block', prompt: 'Draft prompt', position: { x: 71, y: 23 }, configurationId: 'current' }], connections: [], inputs: [] };
  const detail = { id: 'retained', definitionId: 'workflow', definition: { revision: 2, definition: { ...draft, name: 'Captured workflow', blocks: [{ ...draft.blocks[0], name: 'Historical block', prompt: 'Historical template', position: { x: 9, y: 8 }, configurationId: 'historical' }] } }, state: 'succeeded', cleanupPending: false, diagnostic: '', finalResponse: 'Captured output', legacy: false, legacyConfiguration: null, imageId: 'captured-image', inputs: { values: {}, prompts: { same: 'Captured prepared' } }, blocks: [{ blockId: 'same', configuration: { revision: 3, config: { id: 'historical', name: 'Historical settings', model: 'captured-model', reasoning: 'high' } }, result: { blockId: 'same', state: 'succeeded', resolvedPrompt: 'Actual historical prompt', finalResponse: 'Captured output' } }] };
  const calls: string[] = [];
  (globalThis as any).__appRunCommands = new Proxy({}, { get(_target, name: string) { return async () => { calls.push(name); if (name === 'list' || name === 'listConfigurations') return []; if (name === 'runStatus') return null; assert.fail(`Unexpected command ${name}`); }; } });
  (globalThis as any).__appDraft = draft;
  const editorUrl = new URL('../src/lib/editor.ts', import.meta.url).href;
  const overrides = new Map<string, string>([
    ['./lib/commands.ts', moduleUrl('export const createCommands=()=>globalThis.__appRunCommands;')],
    ['./lib/editor.ts', moduleUrl(`export * from ${JSON.stringify(editorUrl)}; export const emptyEditor=()=>({working:globalThis.__appDraft,revision:7,dirty:true,error:''});`)],
    ['@tauri-apps/api/event', moduleUrl('export const listen=async()=>()=>{};')],
    ['@tauri-apps/api/core', moduleUrl('export const invoke=()=>{throw Error("Native invocation forbidden")};export class Channel {}')],
    ['./lib/Canvas.svelte', moduleUrl('export default function(_anchor,props){globalThis.__appCanvas=props;}')],
    ['./lib/RunWorkspace.svelte', moduleUrl('export default function(_anchor,props){globalThis.__appWorkspace=props;return {open(){globalThis.__appOpened=true;}};}')],
  ]);
  const unrelated = moduleUrl('export default function(){return {};}');
  for (const name of ['RunHistory', 'QueuePanel', 'RunPanel', 'AuthReadiness', 'RuntimeSettings']) overrides.set(`./lib/${name}.svelte`, unrelated);
  // Capture the real caller contract while mounting the actual run component.
  const workspaceUrl = moduleUrl(await compileComponent(new URL('../src/lib/RunWorkspace.svelte', import.meta.url), overrides));
  overrides.set('./lib/RunWorkspace.svelte', moduleUrl(`import Actual from ${JSON.stringify(workspaceUrl)};export default function(anchor,props){globalThis.__appWorkspace=props;return Actual(anchor,props);}`));
  const App = (await import(moduleUrl(await compileComponent(new URL('../src/App.svelte', import.meta.url), overrides)))).default;
  const target = window.document.createElement('main'); window.document.body.append(target);
  let component: any;
  const settle = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); flushSync(); };
  try {
    flushSync(() => { component = mount(App, { target }); }); await settle();
    const workspace = (globalThis as any).__appWorkspace;
    assert.ok(workspace, 'Actual App mounts the RunWorkspace contract');
    const canvas = (globalThis as any).__appCanvas;
    flushSync(() => canvas.onselect('same'));
    const before = JSON.parse(JSON.stringify(canvas.definition));
    const selection = canvas.selectedId;
    const beforeCalls = [...calls];
    flushSync(() => workspace.oninspect(detail, 'same')); await settle();
    assert.match(target.textContent!, /Read-only run snapshot.*retained.*revision 2/s);
    assert.equal((globalThis as any).__appCanvas.disabled, true);
    assert.deepEqual((globalThis as any).__appCanvas.definition, detail.definition.definition);
    assert.equal((globalThis as any).__appCanvas.configurations[0].config.model, 'captured-model');
    assert.ok(target.querySelector('[aria-label="Captured block snapshot"]')?.textContent?.includes('Actual historical prompt'));
    assert.equal(target.querySelector('[aria-label="Block inspector"]'), null);
    assert.ok([...target.querySelectorAll('button')].filter(b => /^(Save|▷ Run workflow|＋ Add block)$/.test(b.textContent!.trim())).every(b => b.disabled));
    // Even a misbehaving graph adapter cannot mutate a captured or retained draft.
    flushSync(() => (globalThis as any).__appCanvas.onchange({ ...draft, name: 'Forbidden snapshot mutation' }));
    const back = [...target.querySelectorAll('button')].find(b => b.textContent === 'Return to editing'); assert.ok(back);
    back.click(); await settle();
    assert.deepEqual((globalThis as any).__appCanvas.definition, before);
    assert.equal((globalThis as any).__appCanvas.selectedId, selection);
    assert.match(target.textContent!, /Unsaved changes/);
    assert.deepEqual(calls, beforeCalls, 'Inspection and return perform no load/save/admission commands');
  } finally {
    if (component) await unmount(component);
    for (const key of ['__appRunCommands', '__appDraft', '__appWorkspace', '__appCanvas', '__appOpened']) delete (globalThis as any)[key];
    window.happyDOM.abort();
  }
});
