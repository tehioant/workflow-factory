<script lang="ts">
 import { SvelteFlow, Background, Controls, BackgroundVariant, MarkerType, type Node, type Edge } from '@xyflow/svelte';
 import '@xyflow/svelte/dist/style.css';
 import AiNode from './AiNode.svelte';
 import { toFlow, fromFlow } from './graph.ts';
 import { sequence } from './editor-presentation.ts';
 import type { Definition, SavedConfiguration } from './transport.ts';
 let {definition,disabled=false,selectedId='',configurations=[],onchange,onselect}:{definition:Definition;disabled?:boolean;selectedId?:string;configurations?:SavedConfiguration[];onchange:(d:Definition)=>void;onselect:(id:string)=>void}=$props();
 let nodes=$state<Node[]>([]); let edges=$state<Edge[]>([]);
 $effect(()=>{
   const flow=toFlow(definition), order=sequence(definition);
   nodes=flow.nodes.map(node=>{const block=definition.blocks.find(block=>block.id===node.id)!; return {...node,selected:node.id===selectedId,data:{...node.data,prompt:block.prompt,configuration:configurations.find(saved=>saved.config.id===block.configurationId)?.config.name??'Not configured',ordinal:order.length?order.indexOf(node.id)+1:null}};});
   edges=flow.edges.map(edge=>({...edge,type:'default',markerEnd:{type:MarkerType.ArrowClosed},style:'stroke: var(--color-control-border); stroke-width: 2px;'}));
 });
 const nodeTypes={ai:AiNode};
 function sync(){if(!disabled) onchange(fromFlow(definition,nodes,edges));}
</script>
<svelte:window onkeyup={(event)=>{if(!disabled && ['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.key) && event.target instanceof Element && !event.target.closest('input,textarea,select,[contenteditable=true]') && event.target.closest('.svelte-flow__node, .svelte-flow__selection-wrapper')) sync();}} />
<div class="canvas" aria-label="Workflow canvas">
 <SvelteFlow bind:nodes bind:edges {nodeTypes} nodesDraggable={!disabled} nodesConnectable={!disabled} elementsSelectable={!disabled} onselectionchange={({nodes})=>{if(!disabled && nodes[0] && nodes[0].id!==selectedId) onselect(nodes[0].id);}} onnodeclick={({node})=>{if(!disabled) onselect(node.id);}} onnodedragstop={sync} onconnect={(c)=>{if(!disabled) onchange({...definition,connections:[...definition.connections,{source:c.source,target:c.target}]});}} deleteKey={null} fitView minZoom={0.2} maxZoom={1.5} fitViewOptions={{padding:0.2,maxZoom:1}} colorMode="dark">
  <Background variant={BackgroundVariant.Dots} gap={24} size={1} /><Controls showLock={false} />
 </SvelteFlow>
</div>
