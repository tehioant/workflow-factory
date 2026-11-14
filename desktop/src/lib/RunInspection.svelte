<script lang="ts">
 import { onMount } from 'svelte';
 import type { Commands } from './commands.ts';
 import type { InspectionPage, Notice, ArtifactPage, ArtifactText } from './transport.ts';
 let {commands,definitionId,runId,view='all'}:{commands:Commands;definitionId:string;runId:string;view?:'all'|'diagnostics'|'artifacts'}=$props();
 let page=$state<InspectionPage|null>(null), artifacts=$state<ArtifactPage|null>(null), text=$state<ArtifactText|null>(null);
 let error=$state(''),busy=$state(false);let subscription=$state('');let alive=true,queued=false,flight=false,generation=0;
 const initial=(()=>({workflow:definitionId,run:runId}))();const {workflow,run}=initial;
 const current=()=>alive&&workflow===definitionId&&run===runId;
 async function artifactList(cursor:string|null=null,automatic=false){
  if(automatic&&artifacts?.available)return;const request=++generation;try{const value=await commands.inspectArtifacts(workflow,run,subscription,cursor);if(current()&&request===generation){if(value.run!==run)throw Error();artifacts=value;text=null;}}catch{if(current()&&request===generation)error='Artifacts unavailable or incomplete.';}
 }
 async function readArtifact(id:string,cursor:string|null=null){
  if(!artifacts)return;const capture=artifacts.generation,request=++generation;
  try{const value=await commands.inspectArtifact(workflow,run,subscription,capture,id,cursor);if(current()&&request===generation){if(value.run!==run||value.id!==id||value.generation!==capture)throw Error();text=value;}}catch{if(current()&&request===generation)error='Artifact text unavailable. Reload the capture.';}
 }
 async function refresh(reset=false){
  if(!current()||!subscription)return;if(flight){queued=true;return;}flight=true;busy=true;
  try{const value=await commands.inspectPage(workflow,run,subscription,reset?null:page?.cursor??null);if(!current())return;if(value.run!==run||value.subscription!==subscription||value.entries.length>32)throw Error();page=value;error='';if(!artifacts?.available)await artifactList(null,true);}
  catch{if(current())error='Inspection unavailable. Reload to resynchronize.';}
  finally{flight=false;if(current()){busy=false;if(queued){queued=false;void refresh();}}}
 }
 function notice(value:Notice){if(!current()||value.run!==run)return;if(!subscription){queued=true;return;}if(value.subscription!==subscription)return;queued=true;if(!flight){queued=false;void refresh();}}
 onMount(()=>{
  flight=true;busy=true;
  void commands.inspectSubscribe(workflow,run,notice).then(async value=>{
   if(!current()){void commands.inspectClose(workflow,run,value.subscription).catch(()=>{});return;}
   subscription=value.subscription;if(value.run!==run||value.entries.length>32)throw Error();page=value;await artifactList();
  }).catch(()=>{if(current())error='Inspection unavailable. Close and reopen this run.';}).finally(()=>{flight=false;if(current()){busy=false;if(queued){queued=false;void refresh();}}});
  return()=>{alive=false;generation++;if(subscription)void commands.inspectClose(workflow,run,subscription).catch(()=>{});};
 });
</script>
<section class="run-inspection" aria-label="Selected run inspection" aria-busy={busy}>
 {#if error}<p role="alert">{error} Any previously displayed page is stale until a successful reload.</p>{/if}
 {#if view !== 'artifacts'}
 <h3>Run diagnostics · read-only</h3>
 <p>Selected run {runId}. Known seed/current credentials redacted; arbitrary secrets and undiscoverable intermediate credentials are not covered.</p>
 {#if page?.resync}<p role="status">Snapshot resynchronized to retained generation and floor.</p>{/if}
 {#if !page?.available}<p>Logs unavailable or not yet committed.</p>{/if}
 {#if page?.incomplete}<p>Retained diagnostics incomplete or truncated.</p>{/if}
 {#each page?.entries??[] as entry (entry.sequence)}<p>{entry.block} · {entry.kind} · {entry.sequence}</p><pre>{entry.text}</pre>{/each}
 <button disabled={busy||!subscription} onclick={()=>refresh(true)}>Reload retained logs</button>
 <button disabled={busy||!page?.more} onclick={()=>refresh()}>Next log page</button>
 {/if}
 {#if view !== 'diagnostics'}
 <h4>Immutable sanitized text artifacts</h4>
 <p>Selected run {runId}. Only retained sanitized text is available; unavailable or withheld bytes are not reconstructed.</p>
 {#if !artifacts?.available}<p>Artifacts unavailable or incomplete; no retained-byte claim.</p>{/if}
 {#if artifacts?.available && !artifacts.complete}<p role="status">Retained artifact capture incomplete. Available files do not establish a complete workspace capture.</p>{/if}
 {#if artifacts?.available && artifacts.entries.length === 0}<p>No text artifacts retained on this page.</p>{/if}
 {#each artifacts?.entries??[] as entry (entry.id)}<button onclick={()=>readArtifact(entry.id)}>{entry.name}</button>{/each}
 <button disabled={!subscription} onclick={()=>artifactList()}>Reload artifact list</button>
 <button disabled={!artifacts?.cursor} onclick={()=>artifactList(artifacts!.cursor)}>Next artifacts</button>
 {#if text}<pre>{text.text}</pre><button disabled={!text.cursor} onclick={()=>readArtifact(text!.id,text!.cursor)}>Next text page</button>{/if}
 {/if}
</section>
<style>
 .run-inspection { min-width: 0; color: var(--color-text, #EDF0E8); font-size: 14px; }
 h3, h4 { font-size: 14px; margin: 0 0 12px; }
 h4 { margin-top: 24px; }
 p { margin: 8px 0; overflow-wrap: anywhere; }
 pre { font: 13px/1.6 var(--font-mono, FreeMono, monospace); white-space: pre-wrap; overflow-wrap: anywhere; padding: 12px; background: var(--color-canvas, #141615); border-radius: 6px; }
 button { color: inherit; font: inherit; min-height: 32px; padding: 6px 10px; margin: 4px 8px 4px 0; border: 1px solid var(--color-control-border, #79846F); border-radius: 7px; background: var(--color-raised, #242824); cursor: pointer; }
 button:hover:not(:disabled) { background: var(--color-selection-fill, #2B3521); }
 button:disabled { opacity: .6; cursor: not-allowed; }
 button:focus-visible { outline: 2px solid var(--color-accent, #D1F56A); outline-offset: 3px; }
 [role=alert] { color: var(--color-warning, #E4C17B); }
</style>
