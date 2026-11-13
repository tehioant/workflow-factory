import type { AiBlock, RunDetail } from './transport.ts';

const labels: Record<string, string> = {
  queued: 'Queued', authorizationRequired: 'Authorization required', admitted: 'Admitted',
  running: 'Running', cancelling: 'Cancelling', succeeded: 'Completed', failed: 'Failed',
  stopped: 'Stopped', blocked: 'Blocked', held: 'Held',
};
export function runStateLabel(state: string | null | undefined): string {
  return state ? labels[state] ?? 'Unknown status' : 'Unknown status';
}
export function runNeedsAttention(state: string): boolean {
  return ['queued', 'authorizationRequired', 'admitted', 'running', 'cancelling', 'held'].includes(state);
}

// Claim execution order only for a complete, unambiguous sequential capture.
export function capturedOrder(detail: RunDetail): { blocks: AiBlock[]; sequential: boolean } {
  const { blocks, connections } = detail.definition.definition;
  const fallback = { blocks, sequential: false };
  if (!blocks.length || connections.length !== blocks.length - 1) return fallback;
  const ids = new Set(blocks.map(block => block.id));
  if (ids.size !== blocks.length) return fallback;
  const next = new Map<string, string>(), incoming = new Set<string>();
  for (const edge of connections) {
    if (!ids.has(edge.source) || !ids.has(edge.target) || next.has(edge.source) || incoming.has(edge.target)) return fallback;
    next.set(edge.source, edge.target); incoming.add(edge.target);
  }
  const roots = blocks.filter(block => !incoming.has(block.id));
  if (roots.length !== 1) return fallback;
  const ordered: AiBlock[] = [], visited = new Set<string>();
  let id: string | undefined = roots[0].id;
  while (id && !visited.has(id)) {
    visited.add(id); ordered.push(blocks.find(block => block.id === id)!); id = next.get(id);
  }
  return ordered.length === blocks.length && !id ? { blocks: ordered, sequential: true } : fallback;
}
