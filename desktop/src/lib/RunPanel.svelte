<script lang="ts">
 import { onMount } from 'svelte';
 import RunInspection from './RunInspection.svelte';
 import type { Commands } from './commands.ts';
 import type { RunStatus } from './transport.ts';
 import { errorMessage } from './editor.ts';
 let {commands,run,onstatus,definitionId=null}:{commands:Commands;run:RunStatus|null;definitionId?:string|null;onstatus:(run:RunStatus)=>void}=$props();
 let capturedWorkflow=$state<string|null>(null);
 let error=$state(''); let busy=$state(false); let alive=true; let version=0;
 async function recheck(){
   const id=run?.id;if(!id)return;const request=++version;
   try{const result=await commands.runStatus(id);if(alive&&version===request&&run?.id===id&&result){error='';onstatus(result);}}
   catch(e){if(alive&&version===request&&run?.id===id)error=errorMessage(e);}
 }
 onMount(()=>{
   const selected=run?.id;if(selected&&!definitionId&&commands.inspectWorkflow)void commands.inspectWorkflow(selected).then(value=>{if(alive&&run?.id===selected)capturedWorkflow=value;}).catch(()=>{});
   let timer:ReturnType<typeof setTimeout>;
   async function poll(){
     if(!alive)return;
     if(run&&(['admitted','running','held'].includes(run.state)||run.cleanupPending))await recheck();
     if(alive)timer=setTimeout(poll,1000);
   }
   timer=setTimeout(poll,1000);
   return()=>{alive=false;version++;clearTimeout(timer);};
 });
 async function stop(){if(!run||busy)return;const id=run.id;const request=++version;busy=true;try{const result=await commands.stopRun(id);if(alive&&version===request&&run?.id===id)onstatus(result);}catch(e){if(alive&&version===request)error=errorMessage(e);}finally{if(alive)busy=false;}}
</script>
<section class="notice" aria-label="Run status">
 <h2>Sequential run</h2>
 {#if run}
 <p role="status">{run.state} · {run.id}</p>
 {#if run.diagnostic}<p>{run.diagnostic}</p>{/if}
 {#if run.cleanupPending}<p role="alert">Cleanup or session ownership is uncertain. Further execution is withheld.</p>{/if}
 {#each run.blocks??[] as block (block.blockId)}
 <section aria-label={`Block ${block.blockId}`}><h3>{block.blockId} · {block.state}</h3><h4>Resolved prompt</h4><pre>{block.resolvedPrompt}</pre>{#if block.finalResponse!==null}<h4>Final response</h4><pre>{block.finalResponse}</pre>{/if}</section>
 {/each}
 {#if !run.blocks?.length&&run.finalResponse!==null}<h3>Final response</h3><pre>{run.finalResponse}</pre>{/if}
 {#if definitionId||capturedWorkflow}{#key run.id}<RunInspection {commands} definitionId={(definitionId||capturedWorkflow)!} runId={run.id}/>{/key}{/if}
 <button disabled={busy||!['admitted','running','held'].includes(run.state)} onclick={stop}>Stop this run</button>
 <button onclick={recheck}>Recheck status / retry retained output storage</button>
 {:else}<p>No retained run.</p>{/if}
 {#if error}<p role="alert">{error}</p>{/if}
</section>
