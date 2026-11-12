<script lang="ts">
 import {onMount} from 'svelte';
 import type {Commands} from './commands.ts';
 import type {QueueStatus,QueueEntry} from './transport.ts';
 import {errorMessage} from './editor.ts';
 let {commands,onselect}:{commands:Commands;onselect:(entry:QueueEntry)=>void}=$props();
 let queue=$state<QueueStatus|null>(null);let active=$state(2);let waiting=$state(16);
 let error=$state('');let readError=$state('');let saving=$state(false);let reading=false;let alive=true;let generation=0;
 async function refresh(){if(reading)return;reading=true;const version=++generation;try{const value=await commands.queueStatus();if(alive&&version===generation){if(!queue){active=value.settings.active;waiting=value.settings.waiting;}queue=value;readError='';}}catch(e){if(alive&&version===generation)readError=errorMessage(e);}finally{reading=false;}}
 async function save(){if(saving)return;saving=true;error='';try{await commands.saveQueueSettings({active,waiting});if(alive)await refresh();}catch(e){if(alive)error=errorMessage(e);}finally{if(alive)saving=false;}}
 onMount(()=>{let timer:ReturnType<typeof setTimeout>;async function poll(){await refresh();if(alive)timer=setTimeout(poll,1000);}void poll();return()=>{alive=false;generation++;clearTimeout(timer);};});
</script>
<section class="notice" aria-label="Global run queue">
 <h2>Global run queue</h2>
 {#if queue}
 <p role="status">Occupied {queue.occupied} / {queue.settings.active} · Waiting {queue.waiting} / {queue.settings.waiting}</p>
 <p>Cleanup and persistence holds remain occupied. FIFO head is never bypassed. Credential wait requires an independently acquired app-owned session; no automatic login, retry or failover.</p>
 <ul>{#each queue.entries as entry (entry.id)}<li><button onclick={()=>onselect(entry)}>Run {entry.id} · {entry.state}</button> {entry.diagnostic}{#if entry.cleanupPending} · Ownership hold{/if}{#if entry.state==='queued'&&queue.occupied<queue.settings.active} · Credential wait{/if}</li>{/each}</ul>
 {:else}<p>Loading bounded queue…</p>{/if}
 <label for="queue-active">Active capacity (1–8)</label><input id="queue-active" type="number" min="1" max="8" step="1" bind:value={active}/>
 <label for="queue-waiting">Waiting capacity (0–64)</label><input id="queue-waiting" type="number" min="0" max="64" step="1" bind:value={waiting}/>
 <button disabled={saving||!queue} onclick={save}>Save capacities</button><button onclick={refresh}>Refresh queue (read-only)</button>
 {#if error}<p role="alert">{error}</p>{/if}{#if readError}<p role="alert">{readError}</p>{/if}
</section>
