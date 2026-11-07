<script lang="ts">
 import { SvelteFlow, Background, Controls, type Node, type Edge } from '@xyflow/svelte';
 import '@xyflow/svelte/dist/style.css';
 import AiNode from './AiNode.svelte';
 import { toFlow, fromFlow } from './graph.ts';
 import type { Definition } from './transport.ts';
 let {definition,disabled=false,onchange,onselect}:{definition:Definition;disabled?:boolean;onchange:(d:Definition)=>void;onselect:(id:string)=>void}=$props();
 let nodes=$state<Node[]>([]); let edges=$state<Edge[]>([]);
 $effect(()=>{const flow=toFlow(definition); nodes=flow.nodes; edges=flow.edges;});
 const nodeTypes={ai:AiNode};
 function sync(){onchange(fromFlow(definition,nodes,edges));}
</script>
 <svelte:window onkeyup={(event)=>{if(!disabled && ['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.key) && event.target instanceof Element && event.target.closest('.svelte-flow__node')) sync();}} />
<div class="canvas" aria-label="Workflow canvas">
 <SvelteFlow bind:nodes bind:edges {nodeTypes} nodesDraggable={!disabled} nodesConnectable={!disabled} elementsSelectable={!disabled} onselectionchange={({nodes})=>{if(nodes[0]) onselect(nodes[0].id);}} onnodeclick={({node})=>onselect(node.id)} onnodedragstop={sync} onconnect={(c)=>{if(!disabled) onchange({...definition,connections:[...definition.connections,{source:c.source,target:c.target}]});}} deleteKey={null} fitView colorMode="dark">
  <Background /><Controls />
 </SvelteFlow>
</div>
