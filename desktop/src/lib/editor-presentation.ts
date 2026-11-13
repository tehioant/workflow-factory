import { tick } from 'svelte';
import type { Definition } from './transport.ts';

/** Return an order only when the whole graph is one unambiguous sequence. */
export function sequence(definition: Definition): string[] {
  const ids = new Set(definition.blocks.map(block => block.id));
  if (!ids.size || ids.size !== definition.blocks.length || definition.connections.length !== ids.size - 1) return [];
  const incoming = new Set<string>(), outgoing = new Map<string, string>();
  for (const edge of definition.connections) {
    if (!ids.has(edge.source) || !ids.has(edge.target) || edge.source === edge.target || incoming.has(edge.target) || outgoing.has(edge.source)) return [];
    incoming.add(edge.target); outgoing.set(edge.source, edge.target);
  }
  const roots = [...ids].filter(id => !incoming.has(id));
  if (roots.length !== 1) return [];
  const order: string[] = [];
  let current: string | undefined = roots[0];
  while (current) {
    if (order.includes(current)) return [];
    order.push(current); current = outgoing.get(current);
  }
  return order.length === ids.size ? order : [];
}

/** Focus ownership for modal dialogs and narrow side panels. */
export function focusScope(node: HTMLElement, options: { enabled: boolean; close: () => void }) {
  let previous: HTMLElement | null = null;
  let active = false;
  const controls = () => [...node.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), [tabindex="0"]')].filter(item => item.getClientRects().length);
  function update(next: typeof options) {
    options = next;
    if (options.enabled && !active) {
      previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      active = true;
      void tick().then(() => {
        if (!active || !node.isConnected) return;
        if (!node.hasAttribute('tabindex')) node.tabIndex = -1;
        (controls()[0] ?? node).focus();
      });
    } else if (!options.enabled && active) { active = false; previous?.focus(); }
  }
  function key(event: KeyboardEvent) {
    if (!active) return;
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); options.close(); }
    if (event.key === 'Tab') {
      const items = controls(), first = items[0], last = items.at(-1);
      if (!first) { event.preventDefault(); node.focus(); return; }
      if (event.shiftKey && (document.activeElement === first || !node.contains(document.activeElement))) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && (document.activeElement === last || !node.contains(document.activeElement))) { event.preventDefault(); first.focus(); }
    }
  }
  node.addEventListener('keydown', key); update(options);
  return { update, destroy() { node.removeEventListener('keydown', key); if (active) previous?.focus(); active = false; } };
}
