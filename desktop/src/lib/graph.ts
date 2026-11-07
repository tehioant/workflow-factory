import type { Node, Edge } from '@xyflow/svelte';
import type { Definition } from './transport.ts';
export function toFlow(d: Definition): { nodes: Node[]; edges: Edge[] } {
 return { nodes:d.blocks.map(b=>({id:b.id,type:'ai',position:{...b.position},data:{label:b.name}})), edges:d.connections.map((c,i)=>({id:`connection-${i}`,source:c.source,target:c.target})) };
}
export function fromFlow(d: Definition, nodes: Node[], edges: Edge[]): Definition {
 return {...d,blocks:nodes.flatMap(n=>{const b=d.blocks.find(b=>b.id===n.id); return b?[{...b,position:{x:n.position.x,y:n.position.y}}]:[];}),connections:edges.map(e=>({source:e.source,target:e.target}))};
}
