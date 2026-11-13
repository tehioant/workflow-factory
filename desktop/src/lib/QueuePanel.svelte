<script lang="ts">
 import {onMount} from 'svelte';
 import type {Commands} from './commands.ts';
 import type {QueueStatus,QueueEntry} from './transport.ts';
 import {errorMessage} from './editor.ts';
 let {commands,onselect,admissionError=""}:{commands:Commands;onselect:(entry:QueueEntry)=>void;admissionError?:string}=$props();
 let queue=$state<QueueStatus|null>(null);
 let readError=$state('');let reading=false;let alive=true;let generation=0;
 async function refresh(){if(reading)return;reading=true;const version=++generation;try{const value=await commands.queueStatus();if(alive&&version===generation){queue=value;readError='';}}catch(e){if(alive&&version===generation)readError=errorMessage(e);}finally{reading=false;}}
 onMount(()=>{let timer:ReturnType<typeof setTimeout>;async function poll(){await refresh();if(alive)timer=setTimeout(poll,1000);}void poll();return()=>{alive=false;generation++;clearTimeout(timer);};});
</script>
<section class="notice" aria-label="Global run queue">
 <h2>Global run queue</h2>
 {#if admissionError}<p role="alert">{admissionError}</p>{/if}
 {#if queue}
 <p role="status">Occupied {queue.occupied} / {queue.settings.active} · Waiting {queue.waiting} / {queue.settings.waiting}</p>
 <p>Cleanup and persistence holds remain occupied. FIFO head is never bypassed. Credential wait requires an independently acquired app-owned session; no automatic login, retry or failover.</p>
 <ul>{#each queue.entries as entry (entry.id)}<li><button onclick={()=>onselect(entry)}>Run {entry.id} · {entry.state}</button> {entry.diagnostic}{#if entry.cleanupPending} · Ownership hold{/if}{#if entry.state==='queued'&&queue.occupied<queue.settings.active} · Credential wait{/if}</li>{/each}</ul>
 {:else}<p>Loading bounded queue…</p>{/if}
 <button onclick={refresh}>Refresh queue (read-only)</button>
 {#if readError}<p role="alert">{readError}</p>{/if}
</section>
