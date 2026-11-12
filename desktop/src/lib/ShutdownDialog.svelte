<script lang="ts">
 import {onDestroy} from 'svelte';
 import type {Commands} from './commands.ts';
 import type {ShutdownPreview} from './transport.ts';
 import {errorMessage} from './editor.ts';
 let {commands,dirty,busy,identity,save,onstay,onfreeze}:{commands:Commands;dirty:boolean;busy:boolean;identity:string;save:()=>Promise<boolean>;onstay:()=>void;onfreeze:()=>void}=$props();
 let preview=$state<ShutdownPreview|null>(null),error=$state(''),working=$state(false),consent=$state(false),frozen=$state(false);
 let generation=0;
 let alive=true;
 onDestroy(()=>{alive=false;generation++;});
 let previewIdentity='';
 async function recheck(){if(working||busy)return;const request=++generation;const captured=identity;working=true;error='';try{const value=await commands.shutdownPreview();if(value.frozen){onfreeze();if(alive)frozen=true;}if(!alive)return;if(request===generation&&captured===identity){preview=value;previewIdentity=captured;consent=false;}else error='Edits changed. Recheck before choosing.';}catch(e){if(request===generation)error=errorMessage(e);}finally{if(request===generation)working=false;}}
 async function saveDrafts(){if(working||busy||frozen)return;working=true;error='';preview=null;try{if(!await save())error='Save failed. Drafts retained; no work cancelled.';}catch(e){error=errorMessage(e);}finally{working=false;}}
 async function confirm(){if(working||busy||!preview||!consent)return;const captured=identity;const token=preview.token;working=true;error='';try{if(previewIdentity!==identity||captured!==identity)throw new Error('Edits changed. Recheck.');const pending=await commands.shutdownConfirm(token,true);onfreeze();if(!alive)return;frozen=true;preview=null;if(pending)throw new Error('Admission frozen; cancellation or storage needs selected recovery. Keep the app open.');if(captured!==identity)throw new Error('Admission frozen; edits changed. Keep the app open.');await commands.shutdownFinish();}catch(e){if(alive)error=errorMessage(e);}finally{if(alive)working=false;}}
 async function finish(){if(working||busy)return;working=true;error='';try{await commands.shutdownFinish();}catch(e){error=errorMessage(e);}finally{working=false;}}
</script>
<div class="notice" role="dialog" aria-modal={!frozen} aria-label="Safe application shutdown">
 <h2>Quit Workflow Factory?</h2>
 <p>{dirty?'Workflow or configuration drafts are unsaved. Save first or explicitly discard on successful exit.':'No unsaved drafts.'} Staying changes neither edits nor execution.</p>
 {#if !frozen}
 <button disabled={working} onclick={()=>{generation++;onstay();}}>Stay</button>
 <button disabled={working||busy||!dirty} onclick={saveDrafts}>Save drafts (keep open)</button>
 <button disabled={working||busy} onclick={recheck}>Inspect fresh shutdown state</button>
 {#if preview}<p>{preview.work} queued/held records · {preview.workers} workers · runtime probe {preview.runtimePending?'pending':'returned'} · setup {preview.setup?'pending':'absent'} · retained settlement {preview.retained?'pending':'absent'}.</p>
 <label><input type="checkbox" bind:checked={consent} disabled={working||busy}/> Explicitly cancel queued/active work and setup; discard any remaining drafts only when exit is safe.</label>
 <button disabled={working||busy||!consent} onclick={confirm}>Confirm safe shutdown</button>{/if}
 {:else}<p>Admission is frozen. Keep this app open until workers, exact current-cache cleanup and final durable storage are confirmed. Use the selected run's Stop / cleanup / storage recovery controls; no automatic retry or resume.</p><button disabled={working||busy} onclick={finish}>Recheck completion and quit</button><button disabled={working||busy} onclick={()=>{void commands.openSessionSignIn().catch(e=>{if(alive)error=errorMessage(e);});}}>Open setup recovery</button>{/if}
 {#if error}<p role="alert">{error}</p>{/if}
</div>
