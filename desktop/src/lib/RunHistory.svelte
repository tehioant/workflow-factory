<script lang="ts">
  import { onDestroy } from 'svelte';
  import RunInspection from './RunInspection.svelte';
  import type { Commands } from './commands.ts';
  import type { HistoryPage, RunDetail } from './transport.ts';
  import { runStateLabel } from './run-presentation.ts';
  let { commands, definitionId, onselect }: { commands: Commands; definitionId: string; onselect?: (detail: RunDetail) => void } = $props();
  let page = $state<HistoryPage|null>(null), detail = $state<RunDetail|null>(null);
  let cursor = $state<string|null>(null), selected = $state('');
  let listBusy = $state(false), detailBusy = $state(false);
  let listError = $state(''), detailError = $state('');
  let listGeneration=0, detailGeneration=0, alive=true;
  onDestroy(()=>{alive=false;listGeneration++;detailGeneration++;});
  function clearSelection() { detailGeneration++; selected=''; detail=null; detailError=''; detailBusy=false; }
  async function readPage(workflow:string, boundary:string|null) {
    const generation=++listGeneration;
    clearSelection(); page=null; listError=''; listBusy=true;
    const current=()=>alive&&generation===listGeneration&&workflow===definitionId&&boundary===cursor;
    try {
      const value=await commands.listRunHistory(workflow,boundary,20);
      if(!current())return;
      if(value.definitionId!==workflow||value.cursor!==boundary||value.entries.length>20)throw Error('identity');
      page=value;
    } catch { if(current()){page=null;listError='Retained run list unavailable. Retry this page.';} }
    finally { if(current())listBusy=false; }
  }
  $effect(()=>{const workflow=definitionId;const boundary=cursor;void readPage(workflow,boundary);});
  async function selectRun(id:string) {
    const workflow=definitionId, boundary=cursor, generation=++detailGeneration;
    selected=id; detail=null; detailError=''; detailBusy=true;
    const current=()=>alive&&generation===detailGeneration&&workflow===definitionId&&boundary===cursor&&selected===id;
    try {
      const value=await commands.runDetail(workflow,id);
      if(!current())return;
      if(value.id!==id||value.definitionId!==workflow||value.definition.definition.id!==workflow)throw Error('identity');
      detail=value;
      onselect?.(value);
    } catch { if(current()){detail=null;detailError='Captured run unavailable. Retry the selected run.';} }
    finally {if(current())detailBusy=false;}
  }
  function navigate(boundary:string|null) {listGeneration++;clearSelection();page=null;cursor=boundary;}
</script>
<section class="run-history" aria-label="Retained run history">
  <h2>Retained run history · read-only</h2>
  <p>Captured records, separate from editor and active run controls. Newest retained record first; execution dates were not captured. Select a captured run to inspect retained logs and text artifacts.</p>
  <div aria-busy={listBusy}>
    <button disabled={listBusy} onclick={()=>readPage(definitionId,cursor)}>Refresh history page</button>
    <button disabled={listBusy||cursor===null} onclick={()=>navigate(null)}>Newest page</button>
    <button disabled={listBusy||!page?.nextCursor} onclick={()=>navigate(page!.nextCursor)}>Older runs</button>
    {#if listError}<p role="alert">{listError}</p>{/if}
    {#if page?.unavailableRecords}<p role="status">Some retained records for this workflow have unsafe identities and are unavailable. Healthy runs remain listed.</p>{/if}
    {#if page?.unknownScopeRecords}<p role="status">Some retained records cannot be scoped safely (oversized or malformed capture). They may belong to other workflows and are not listed.</p>{/if}
    {#if listBusy}<p role="status">Loading retained runs…</p>{/if}
    <ul>{#each page?.entries??[] as entry (entry.id)}
      <li><button aria-pressed={selected===entry.id} onclick={()=>selectRun(entry.id)}>Run {entry.id} · {entry.state ? runStateLabel(entry.state) : 'Unavailable'}</button><p>{entry.inputSummary||'No captured input values'}</p>{#if entry.unavailable}<p>Captured details unavailable</p>{/if}</li>
    {:else}{#if page}<li>No retained runs on this page.</li>{/if}{/each}</ul>
  </div>
  {#if selected}<section aria-label="Selected captured run" aria-busy={detailBusy}>
    <h3>Captured run {selected}</h3>
    {#if detailBusy}<p role="status">Loading captured details…</p>{/if}
    {#if detailError}<p role="alert">{detailError}</p><button onclick={()=>selectRun(selected)}>Retry captured read</button>{/if}
    {#if detail && !onselect}
      <p>{detail.state} · Cleanup pending: {detail.cleanupPending?'yes':'no'}</p>
      <p>Captured definition: {detail.definition.definition.name} · revision {detail.definition.revision} · isolation {detail.definition.definition.isolation}</p>
      <p>Captured image: {detail.imageId}</p>
      <h4>Captured inputs</h4>{#each Object.entries(detail.inputs.values) as [name,value]}<p>{name}</p><pre>{value}</pre>{/each}
      <h4>Captured definition prompts</h4>{#each detail.definition.definition.blocks as block}<p>{block.name} · {block.id}</p><pre>{block.prompt}</pre><p>Input-prepared prompt (not proof of execution)</p><pre>{detail.inputs.prompts[block.id]??'Unavailable'}</pre>{/each}
      <h4>Captured connections and input declarations</h4>
      {#each detail.definition.definition.connections as connection}<p>{connection.source} → {connection.target}</p>{/each}
      {#each detail.definition.definition.inputs as input}<p>{input.name} · required: {input.required?'yes':'no'} · default: {input.default??'Unavailable'}</p>{/each}
      {#if detail.legacy}<p>Legacy capture: per-block settings and executed prompts unavailable.</p>{#if detail.legacyConfiguration}<p>Legacy captured configuration: {detail.legacyConfiguration.config.name} · revision {detail.legacyConfiguration.revision} · {detail.legacyConfiguration.config.model} / {detail.legacyConfiguration.config.reasoning}</p>{/if}{/if}
      {#each detail.blocks as block}
        <h4>Block {block.blockId}</h4>
        {#if block.configuration}<p>Captured configuration: {block.configuration.config.name} · revision {block.configuration.revision} · {block.configuration.config.model} / {block.configuration.config.reasoning}</p>{:else}<p>Per-block configuration unavailable</p>{/if}
        {#if block.result}<p>{block.result.state} · Actual retained resolved prompt</p><pre>{block.result.resolvedPrompt}</pre><p>Durable block response</p><pre>{block.result.finalResponse??'Unavailable'}</pre>{:else}<p>{detail.legacy?'Execution evidence unavailable':'Not executed: no retained block result or executed prompt'}</p>{/if}
      {/each}
      {#key detail.id}<RunInspection {commands} {definitionId} runId={detail.id}/>{/key}
      <h4>Retained final response</h4><pre>{detail.finalResponse??'Unavailable'}</pre>
      <h4>Retained diagnostic</h4><pre>{detail.diagnostic||'None captured'}</pre>
    {/if}
  </section>{/if}
</section>
<style>
 .run-history { min-width: 0; padding: 24px; background: var(--color-surface, #1B1E1B); color: var(--color-text, #EDF0E8); font-size: 14px; }
 h2 { font-size: 18px; margin: 0 0 12px; }
 h3 { font-size: 16px; margin: 20px 0 12px; }
 h4 { font-size: 14px; margin: 20px 0 8px; }
 p { margin: 8px 0; overflow-wrap: anywhere; }
 button { color: inherit; font: inherit; min-height: 32px; padding: 6px 10px; margin: 4px 8px 4px 0; border: 1px solid var(--color-control-border, #79846F); border-radius: 7px; background: var(--color-raised, #242824); cursor: pointer; overflow-wrap: anywhere; text-align: left; }
 button:hover:not(:disabled), button[aria-pressed=true] { background: var(--color-selection-fill, #2B3521); }
 button:disabled { opacity: .6; cursor: not-allowed; }
 button:focus-visible { outline: 2px solid var(--color-primary, #D1F56A); outline-offset: 3px; }
 ul { margin: 16px 0; padding: 0; list-style: none; }
 li { padding: 12px 0; border-bottom: 1px solid var(--color-divider, #343A33); }
 li p { color: var(--color-secondary, #8A9485); }
 pre { font: 13px/1.6 var(--font-mono, FreeMono, monospace); white-space: pre-wrap; overflow-wrap: anywhere; }
 [role=alert] { color: var(--color-warning, #E4C17B); }
</style>
