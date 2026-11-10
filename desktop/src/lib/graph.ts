import type { Node, Edge } from '@xyflow/svelte';
import type { Definition } from './transport.ts';

export function toFlow(definition: Definition): { nodes: Node[]; edges: Edge[] } {
  return {
    nodes: definition.blocks.map(block => ({
      id: block.id,
      type: 'ai',
      position: { ...block.position },
      data: { label: block.name },
    })),
    edges: definition.connections.map((connection, index) => ({
      id: `connection-${index}`,
      source: connection.source,
      target: connection.target,
    })),
  };
}

export function fromFlow(definition: Definition, nodes: Node[], edges: Edge[]): Definition {
  return {
    ...definition,
    blocks: nodes.flatMap(node => {
      const block = definition.blocks.find(block => block.id === node.id);
      if (!block) return [];
      return [{ ...block, position: { x: node.position.x, y: node.position.y } }];
    }),
    connections: edges.map(edge => ({ source: edge.source, target: edge.target })),
  };
}
