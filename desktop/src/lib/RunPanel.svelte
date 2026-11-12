<script lang="ts">
 import { onMount } from 'svelte';
 import type { Commands } from './commands.ts';
 import type { RunStatus } from './transport.ts';
 import { errorMessage } from './editor.ts';
 let {commands,run,onstatus}:{commands:Commands;run:RunStatus|null;onstatus:(run:RunStatus)=>void}=$props();
 let error=$state(''); let busy=$state(false);
 onMount(()=>{
   let alive=true; let polls=0; let timer:ReturnType<typeof setTimeout>;
   async function poll(){
     const id=run?.id;
     if(!id||!['admitted','running'].includes(run!.state)||polls++>=120)return;
     try {const result=await commands.runStatus(id);if(alive&&run?.id===id&&result)onstatus(result);}
     catch(e){if(alive&&run?.id===id)error=errorMessage(e);}
     if(alive)timer=setTimeout(poll,1000);
   }
   timer=setTimeout(poll,1000);
   return()=>{alive=false;clearTimeout(timer);};
 });
 async function stop(){if(!run||busy)return;const id=run.id;busy=true;try{const result=await commands.stopRun(id);if(run?.id===id)onstatus(result);}catch(e){error=errorMessage(e);}finally{busy=false;}}
</script>
<section class="notice" aria-label="Run status">
 <h2>Single-block run</h2>
 {#if run}
 <p role="status">{run.state} · {run.id}</p>
 {#if run.diagnostic}<p>{run.diagnostic}</p>{/if}
 {#if run.cleanupPending}<p role="alert">Cleanup or session ownership is uncertain. Further execution is withheld.</p>{/if}
 {#if run.finalResponse!==null}<h3>Final response</h3><pre>{run.finalResponse}</pre>{/if}
 <button disabled={busy||!['admitted','running','held'].includes(run.state)} onclick={stop}>Stop this run</button>
 {:else}<p>No retained run.</p>{/if}
 {#if error}<p role="alert">{error}</p>{/if}
</section>
