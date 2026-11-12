<script lang="ts">
 import type { Definition } from './transport.ts';
 let {definition,blockId,field,disabled=false,onprompt}:{definition:Definition;blockId:string;field:HTMLTextAreaElement|undefined;disabled?:boolean;onprompt:(prompt:string)=>void}=$props();
 let choice=$state('');
 let ancestors=$derived.by(()=>{
   const ids=new Set(definition.blocks.map(b=>b.id));
   const incoming=new Map<string,string>(),outgoing=new Map<string,string>();
   if(ids.size!==definition.blocks.length||definition.connections.length!==definition.blocks.length-1)return [];
   for(const c of definition.connections){if(!ids.has(c.source)||!ids.has(c.target)||c.source===c.target||incoming.has(c.target)||outgoing.has(c.source))return [];incoming.set(c.target,c.source);outgoing.set(c.source,c.target);}
   const roots=definition.blocks.filter(b=>!incoming.has(b.id));if(roots.length!==1)return [];
   const order:string[]=[];let id:string|undefined=roots[0].id;
   while(id){if(order.includes(id))return [];order.push(id);id=outgoing.get(id);}
   if(order.length!==ids.size)return [];
   const index=order.indexOf(blockId);return index<0?[]:order.slice(0,index).map(id=>definition.blocks.find(b=>b.id===id)!);
 });
 function insert(){
   if(disabled||!field||!ancestors.some(b=>b.id===choice))return;
   const prompt=definition.blocks.find(b=>b.id===blockId)?.prompt;if(prompt===undefined)return;
   const reference=`{{output.${choice}.finalResponse}}`;
   const start=field.selectionStart,end=field.selectionEnd;
   onprompt(prompt.slice(0,start)+reference+prompt.slice(end));field.focus();
 }
</script>
<label for="output-reference">Predecessor final response</label>
<select id="output-reference" {disabled} value={choice} onchange={(event)=>{choice=event.currentTarget.value;}}><option value="">Choose predecessor</option>{#each ancestors as block}<option value={block.id}>{block.name} · finalResponse</option>{/each}</select>
<button disabled={disabled||!ancestors.some(b=>b.id===choice)} onclick={insert}>Insert predecessor response</button>