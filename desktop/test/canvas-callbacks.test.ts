import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Window, type HTMLElement as TestElement } from 'happy-dom';
import { compile } from 'svelte/compiler';
import { fromFlow } from '../src/lib/graph.ts';

const source = readFileSync(new URL('../src/lib/Canvas.svelte', import.meta.url), 'utf8');

test('canvas compiles without accessibility warnings', () => {
  const { warnings } = compile(source, { filename: 'Canvas.svelte', generate: 'client' });
  assert.deepEqual(warnings.filter(warning => warning.code.startsWith('a11y_')).map(warning => warning.code), []);
});

// Execute the actual component callbacks with only their captured boundaries injected.
test('busy canvas sync cannot publish domain changes', () => {
  const body = source.match(/function sync\(\)\{([^\n]*)\}/)![1];
  let published = 0;
  new Function('disabled', 'onchange', 'fromFlow', 'definition', 'nodes', 'edges', body)(true, () => published++, fromFlow, {}, [], []);
  assert.equal(published, 0);
});

test('selection wrapper arrow key publishes group positions', async () => {
  const window = new Window();
  const wrapper = window.document.createElement('div');
  wrapper.className = 'svelte-flow__selection-wrapper';
  const handler = source.match(/onkeyup=\{(.*?)\} \/>/)![1];
  let published = 0;
  const callback = new Function('disabled', 'sync', 'Element', `return (${handler});`)(false, () => published++, window.Element);
  callback({ key: 'ArrowRight', target: wrapper });
  assert.equal(published, 1);
  await window.happyDOM.abort();
});

test('canvas keyboard surface preserves editing and read-only boundaries', async () => {
  const window = new Window();
  for (const key of ['document', 'Node', 'Element', 'HTMLElement', 'Text', 'Comment', 'Event', 'KeyboardEvent', 'MouseEvent', 'HTMLMediaElement', 'MutationObserver']) {
    Object.defineProperty(globalThis, key, { configurable: true, value: (window as any)[key] });
  }
  Object.defineProperty(globalThis, 'window', { configurable: true, value: window });
  const url = (s: string) => 'data:text/javascript;base64,' + Buffer.from(s).toString('base64');
  const svelteClient = new URL('../node_modules/svelte/src/index-client.js', import.meta.url).href;
  // The graph boundary forwards HTML attributes/events to its application root, as Svelte Flow does.
  const graphBoundary = `
    import {onDestroy} from ${JSON.stringify(svelteClient)};
    export function SvelteFlow(anchor, props) {
      globalThis.__flow = props;
      const root = document.createElement('div');
      root.className = 'svelte-flow';
      root.setAttribute('role', 'application');
      root.tabIndex = props.tabindex ?? -1;
      for (const name of ['aria-label', 'aria-describedby']) {
        if (props[name]) root.setAttribute(name, props[name]);
      }
      for (const name of ['onkeydown', 'onfocusout', 'onpointerdown']) {
        root[name] = event => props[name]?.(event);
      }
      const pane = document.createElement('div');
      pane.className = 'svelte-flow__pane';
      root.append(pane);
      anchor.before(root);
      onDestroy(() => root.remove());
    }
    export function Background() {}
    export function Controls() {}
    export const BackgroundVariant = {Dots: 0}, MarkerType = {ArrowClosed: 0};
  `;
  const { compileComponent } = await import('./compile-component.ts');
  const overrides = new Map([
    ['@xyflow/svelte', url(graphBoundary)],
    ['@xyflow/svelte/dist/style.css', url('')],
    ['./AiNode.svelte', url('export default function(){}')]
  ]);
  const Component = (await import(url(await compileComponent(new URL('../src/lib/Canvas.svelte', import.meta.url), overrides)))).default;
  const { mount, unmount, flushSync } = await import(svelteClient);
  const target = window.document.createElement('main');
  window.document.body.append(target);
  let component: any;
  try {
    flushSync(() => component = mount(Component, { target, props: {
      definition: { id: 'd', name: 'D', schemaVersion: 1, isolation: 'perRun', blocks: [], connections: [], inputs: [] },
      onchange: () => assert.fail('domain mutation forbidden'),
      onselect: () => assert.fail('selection forbidden')
    } }));
    const container = target.querySelector('.canvas') as TestElement;
    const canvas = target.querySelector('.svelte-flow') as TestElement;
    const flow = (globalThis as any).__flow;
    assert.equal(target.querySelectorAll('[role=application]').length, 1);
    assert.equal(container.hasAttribute('tabindex'), false);
    assert.equal(canvas.tabIndex, 0);
    assert.equal(canvas.getAttribute('aria-label'), 'Workflow canvas');
    const help = window.document.getElementById(canvas.getAttribute('aria-describedby')!);
    assert.ok(help);
    assert.match(help.textContent!, /Tab.*blocks.*Enter.*select.*Arrow keys.*move.*Space.*pan/);

    const { setTheme } = await import('../src/lib/appearance.ts');
    for (const mode of ['light', 'dark'] as const) {
      setTheme(mode); flushSync(); assert.equal(flow.colorMode, mode);
    }
    assert.equal(flow.panActivationKey, null);
    assert.deepEqual(flow.panOnDrag, [1, 2]);
    const space = () => new window.KeyboardEvent('keydown', { key: ' ', code: 'Space', bubbles: true, cancelable: true });
    const textarea = window.document.createElement('textarea');
    target.append(textarea); textarea.focus();
    const outside = space(); textarea.dispatchEvent(outside); flushSync();
    assert.equal(outside.defaultPrevented, false);
    assert.deepEqual(flow.panOnDrag, [1, 2]);
    for (const tag of ['input', 'textarea', 'select', 'button', 'a', 'div']) {
      const control = window.document.createElement(tag);
      if (tag === 'div') control.setAttribute('contenteditable', 'true');
      canvas.append(control);
      const key = space(); control.dispatchEvent(key); flushSync();
      assert.equal(key.defaultPrevented, false, tag);
      assert.deepEqual(flow.panOnDrag, [1, 2], tag);
      control.remove();
    }
    canvas.focus();
    const key = space(); canvas.dispatchEvent(key); flushSync();
    assert.equal(key.defaultPrevented, true);
    assert.equal(flow.panOnDrag, true);
    canvas.dispatchEvent(new window.KeyboardEvent('keyup', { code: 'Space', bubbles: true })); flushSync();
    assert.deepEqual(flow.panOnDrag, [1, 2]);
    canvas.dispatchEvent(space()); flushSync();
    window.dispatchEvent(new window.Event('blur')); flushSync();
    assert.deepEqual(flow.panOnDrag, [1, 2]);
    canvas.dispatchEvent(space()); flushSync();
    textarea.focus(); flushSync();
    assert.deepEqual(flow.panOnDrag, [1, 2]);
    const pane = canvas.querySelector('.svelte-flow__pane')!;
    pane.dispatchEvent(new window.Event('pointerdown', { bubbles: true }));
    assert.equal(window.document.activeElement, canvas);

    await unmount(component); component = null;
    flushSync(() => component = mount(Component, { target, props: {
      definition: { id: 'captured', name: 'Captured', schemaVersion: 1, isolation: 'perRun', blocks: [], connections: [], inputs: [] },
      disabled: true,
      onchange: () => assert.fail('captured mutation forbidden'),
      onselect: () => assert.fail('captured selection forbidden')
    } }));
    const captured = (globalThis as any).__flow;
    const capturedCanvas = target.querySelector('.svelte-flow') as TestElement;
    assert.equal(target.querySelectorAll('[role=application]').length, 1);
    assert.equal(capturedCanvas.getAttribute('aria-label'), 'Captured workflow canvas (read-only)');
    const capturedHelp = window.document.getElementById(capturedCanvas.getAttribute('aria-describedby')!);
    assert.ok(capturedHelp);
    assert.match(capturedHelp.textContent!, /Read-only.*zoom controls/);
    assert.doesNotMatch(capturedHelp.textContent!, /Arrow keys.*move|Space.*pan/);
    const capturedSpace = space(); capturedCanvas.dispatchEvent(capturedSpace); flushSync();
    assert.equal(capturedSpace.defaultPrevented, false);
    assert.deepEqual(captured.panOnDrag, [1, 2]);
    for (const mode of ['light', 'dark'] as const) {
      setTheme(mode); flushSync();
      assert.equal(captured.colorMode, mode);
      assert.equal(captured.nodesDraggable, false);
      assert.equal(captured.nodesConnectable, false);
      assert.equal(captured.elementsSelectable, false);
      captured.onnodeclick({ node: { id: 'ignored' } });
    }
  } finally {
    if (component) await unmount(component);
    delete (globalThis as any).__flow;
    window.happyDOM.abort();
  }
});
