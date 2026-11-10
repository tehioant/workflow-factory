import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Window } from 'happy-dom';
import { fromFlow } from '../src/lib/graph.ts';
const source = readFileSync(new URL('../src/lib/Canvas.svelte',import.meta.url),'utf8');
// Execute the actual component callbacks with only their captured boundaries injected.
test('busy canvas sync cannot publish domain changes', () => {
  const body = source.match(/function sync\(\)\{([^\n]*)\}/)![1];
  let published=0;
  new Function('disabled','onchange','fromFlow','definition','nodes','edges',body)(true,()=>published++,fromFlow,{},[],[]);
  assert.equal(published,0);
});
test('selection wrapper arrow key publishes group positions', () => {
  const window=new Window(); const wrapper=window.document.createElement('div'); wrapper.className='svelte-flow__selection-wrapper';
  const handler=source.match(/onkeyup=\{(.*?)\} \/>/)![1];
  let published=0;
  const callback=new Function('disabled','sync','Element',`return (${handler});`)(false,()=>published++,window.Element);
  callback({key:'ArrowRight',target:wrapper});
  assert.equal(published,1);
});
