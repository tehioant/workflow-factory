<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type { Commands } from './commands.ts';
  import type { RunDetail, RunStatus } from './transport.ts';
  import { errorMessage } from './editor.ts';
  import { capturedOrder, runNeedsAttention, runStateLabel } from './run-presentation.ts';
  import RunInspection from './RunInspection.svelte';
  import CapturedRunBlock from './CapturedRunBlock.svelte';

  let { commands, run, definitionId, onstatus, oninspect }: {
    commands: Commands; run: RunStatus | null; definitionId: string | null;
    onstatus: (run: RunStatus) => void; oninspect: (detail: RunDetail, blockId: string) => void;
  } = $props();
  type Tab = 'Output' | 'Detail' | 'Diagnostics' | 'Inputs' | 'Artifacts';
  const tabs: Tab[] = ['Output', 'Detail', 'Diagnostics', 'Inputs', 'Artifacts'];
  let expanded = $state(false), tab = $state<Tab>('Output'), blockId = $state('');
  let detail = $state<RunDetail | null>(null), detailBusy = $state(false), busy = $state(false);
  let readError = $state(''), actionError = $state(''), detailError = $state(''), copyMessage = $state('');
  let alive = true, statusGeneration = 0, detailGeneration = 0, selection = '';
  let toggle: HTMLButtonElement;
  const identity = () => `${run?.id ?? ''}\u0000${definitionId ?? ''}`;
  const order = $derived(detail ? capturedOrder(detail) : null);
  const selected = $derived(detail?.blocks.find(block => block.blockId === blockId));
  const output = $derived(selected?.result?.finalResponse ?? null);
  const completed = $derived(run?.blocks.filter(block => block.state === 'succeeded').length ?? 0);

  export function open() { expanded = true; }
  function close() { expanded = false; toggle?.focus(); }
  $effect(() => {
    const key = identity();
    if (selection !== key) {
      selection = key; statusGeneration++; detailGeneration++;
      busy = false; detailBusy = false; detail = null; blockId = '';
      readError = ''; actionError = ''; detailError = ''; copyMessage = '';
    }
  });
  $effect(() => {
    const visible = expanded, id = run?.id, workflow = definitionId;
    if (visible && id) void readDetail(id, workflow);
    return () => { detailGeneration++; detailBusy = false; };
  });

  async function readDetail(id: string, workflow: string | null) {
    const key = identity(), generation = ++detailGeneration;
    detailBusy = true;
    const current = () => alive && expanded && key === identity() && generation === detailGeneration;
    try {
      const scope = workflow ?? await commands.inspectWorkflow(id);
      if (!current()) return;
      const value = await commands.runDetail(scope, id);
      if (!current()) return;
      if (value.id !== id || value.definitionId !== scope || value.definition.definition.id !== scope) throw Error('Captured run identity changed.');
      detail = value; detailError = '';
      if (!value.definition.definition.blocks.some(block => block.id === blockId)) blockId = capturedOrder(value).blocks[0]?.id ?? '';
    } catch { if (current()) { detail = null; detailError = 'Captured run unavailable. Retry the captured read; current editor settings are never substituted.'; } }
    finally { if (current()) detailBusy = false; }
  }
  async function recheck() {
    if (!run || busy) return;
    const id = run.id, key = identity(), generation = ++statusGeneration;
    busy = true;
    const current = () => alive && key === identity() && generation === statusGeneration;
    try {
      const value = await commands.runStatus(id);
      if (!current()) return;
      if (!value || value.id !== id) throw Error('Selected run unavailable or response identity changed.');
      readError = ''; onstatus(value);
      if (expanded && !detailBusy) void readDetail(id, definitionId);
    } catch (error) { if (current()) readError = errorMessage(error); }
    finally { if (current()) busy = false; }
  }
  async function action(invoke: (id: string) => Promise<RunStatus>) {
    if (!run || busy) return;
    const id = run.id, key = identity(), generation = ++statusGeneration;
    busy = true;
    const current = () => alive && key === identity() && generation === statusGeneration;
    try {
      const value = await invoke(id);
      if (!current()) return;
      if (value.id !== id) throw Error('Selected run response identity changed.');
      actionError = ''; readError = ''; onstatus(value);
      if (expanded) void readDetail(id, definitionId);
    } catch (error) { if (current()) actionError = errorMessage(error); }
    finally { if (current()) busy = false; }
  }
  async function authorize(id: string) {
    await commands.authorizeRun(id);
    const value = await commands.runStatus(id);
    if (!value) throw Error('Selected run unavailable.');
    return value;
  }
  async function copy() {
    if (output === null) return;
    const key = identity(), selectedBlock = blockId, value = output;
    try {
      await navigator.clipboard.writeText(value);
      if (alive && key === identity() && blockId === selectedBlock && output === value) copyMessage = 'Copied final response.';
    } catch { if (alive && key === identity() && blockId === selectedBlock) copyMessage = 'Copy failed. Select and copy the response manually.'; }
  }
  function selectBlock(id: string) { blockId = id; copyMessage = ''; }
  async function selectTab(event: KeyboardEvent, index: number) {
    let next = index;
    if (event.key === 'ArrowRight') next = (index + 1) % tabs.length;
    else if (event.key === 'ArrowLeft') next = (index + tabs.length - 1) % tabs.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = tabs.length - 1;
    else return;
    event.preventDefault();
    const list = event.currentTarget instanceof HTMLElement ? event.currentTarget.parentElement : null;
    tab = tabs[next]; await tick(); (list?.querySelectorAll<HTMLButtonElement>('[role=tab]')[next])?.focus();
  }
  onMount(() => {
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      if (!alive) return;
      if (expanded && run && (runNeedsAttention(run.state) || run.cleanupPending)) await recheck();
      if (alive) timer = setTimeout(poll, 1000);
    }
    timer = setTimeout(poll, 1000);
    return () => { alive = false; statusGeneration++; detailGeneration++; clearTimeout(timer); };
  });
</script>

<section class:expanded class="run-workspace" aria-label="Run workspace">
  <div class="run-strip">
    <button class="toggle" bind:this={toggle} aria-expanded={expanded} onclick={() => expanded ? close() : open()}>
      <span aria-hidden="true">{expanded ? '⌄' : '⌃'}</span> Run details
    </button>
    {#if run}<span class="state" data-state={run.state}>{runStateLabel(run.state)}</span><span class="run-id" title={run.id}>{run.id}</span>
      {#if detail}<span class="count">Revision {detail.definition.revision} · {completed}/{detail.definition.definition.blocks.length} blocks completed</span>{/if}
    {:else}<span class="secondary">No run selected</span>{/if}
    {#if expanded}<button class="close" aria-label="Close run details" onclick={close}>×</button>{/if}
  </div>
  {#if expanded}
    {#if run}
      <div class="run-controls" aria-busy={busy}>
        {#if runNeedsAttention(run.state)}<button disabled={busy} onclick={() => action(id => commands.stopRun(id))}>Cancel this run</button>{/if}
        {#if run.state === 'authorizationRequired'}<button disabled={busy} onclick={() => action(authorize)}>Authorize exact reserved run (native)</button>{/if}
        <button disabled={busy} onclick={recheck}>Recheck status</button>
        {#if run.cleanupPending}<button disabled={busy} onclick={() => action(id => commands.retryRunCleanup(id))}>Retry owned cleanup</button>{/if}
        <button disabled={busy} onclick={() => action(id => commands.retryRunOutput(id))}>Retry retained output storage</button>
        <span class="secondary">Selected run only · no model re-execution</span>
      </div>
      {#if run.diagnostic}<p class="message">{run.diagnostic}</p>{/if}
      {#if run.cleanupPending}<p class="hold" role="alert">Cleanup or session ownership is uncertain. Further execution is withheld, even if a model result is complete.</p>{/if}
      {#if actionError}<p class="error" role="alert">{actionError}</p>{/if}
      {#if readError}<p class="error" role="alert">{readError}</p>{/if}
      <div class="drawer-body">
        <nav class="block-list" aria-label="Captured run blocks">
          <h3>{order?.sequential ? 'Execution order' : 'Captured blocks'}</h3>
          {#if detailBusy}<p role="status">Refreshing capture…</p>{/if}
          {#each order?.blocks ?? [] as block (block.id)}
            {@const result = detail?.blocks.find(capture => capture.blockId === block.id)?.result}
            <button aria-current={blockId === block.id ? 'true' : undefined} onclick={() => selectBlock(block.id)}>
              <strong>{block.name}</strong><span>{result ? runStateLabel(result.state) : detail?.legacy ? 'Evidence unavailable' : 'Not started / result unavailable'}</span>
            </button>
          {/each}
          {#if detail && blockId}<button class="snapshot" onclick={() => detail && oninspect(detail, blockId)}>Inspect on captured canvas</button>{/if}
          {#if detailError}<p role="alert">{detailError}</p><button onclick={() => run && readDetail(run.id, definitionId)}>Retry captured read</button>{/if}
        </nav>
        <div class="run-content">
          <div class="tabs" role="tablist" aria-label="Run details tabs">
            {#each tabs as name, index}<button role="tab" aria-selected={tab === name} tabindex={tab === name ? 0 : -1} onkeydown={event => selectTab(event, index)} onclick={() => tab = name}>{name}</button>{/each}
          </div>
          <div class="tab-content" role="tabpanel" aria-label={tab} tabindex="0">
            {#if tab === 'Diagnostics' || tab === 'Artifacts'}
              {#if detail}{#key `${detail.definitionId}:${detail.id}:${tab}`}<RunInspection {commands} definitionId={detail.definitionId} runId={detail.id} view={tab === 'Diagnostics' ? 'diagnostics' : 'artifacts'}/>{/key}
              {:else}<p>Captured scope unavailable. Retained inspection cannot be opened safely.</p>{/if}
            {:else if !detail}<p>{detailBusy ? 'Loading captured run…' : 'Captured details unavailable.'}</p>
            {:else if tab === 'Detail'}<CapturedRunBlock {detail} {blockId}/>
            {:else if tab === 'Inputs'}
              <h3>Captured input values</h3>
              {#each Object.entries(detail.inputs.values) as [name, value]}<h4>{name}</h4><pre>{value}</pre>{:else}<p>No captured input values.</p>{/each}
              <p class="secondary">From this accepted run, not current workflow inputs.</p>
            {:else}
              <div class="output-heading"><h3>{order?.blocks.find(block => block.id === blockId)?.name ?? 'Selected block'} · final response</h3><button disabled={output === null} onclick={copy}>Copy</button></div>
              {#if output !== null}<pre class="output">{output}</pre>{:else}<p>No retained final response for this block. This does not imply successful execution.</p>{/if}
              {#if detail.legacy && detail.finalResponse !== null}<h4>Legacy run final response · block attribution unavailable</h4><pre>{detail.finalResponse}</pre>{/if}
              {#if copyMessage}<p role="status">{copyMessage}</p>{/if}
            {/if}
          </div>
        </div>
      </div>
    {:else}<p class="message">Select a retained run or explicitly start a saved workflow to inspect it here.</p>{/if}
  {/if}
</section>

<style>
  .run-workspace { min-width: 0; min-height: 44px; flex-shrink: 0; color: var(--color-text, #EDF0E8); background: var(--color-surface, #1B1E1B); border-top: 1px solid var(--color-divider, #343A33); font-size: 14px; }
  .run-workspace.expanded { height: min(510px, 45%); min-height: 0; display: flex; flex-direction: column; overflow: auto; }
  .run-strip { display: flex; align-items: center; gap: 12px; min-height: 44px; padding: 0 16px; flex-shrink: 0; }
  button { color: inherit; font: inherit; min-height: 32px; padding: 6px 10px; border: 1px solid var(--color-control-border, #79846F); border-radius: 7px; background: var(--color-raised, #242824); cursor: pointer; }
  button:hover:not(:disabled) { background: var(--color-selection-fill, #2B3521); }
  button:disabled { opacity: .6; cursor: not-allowed; }
  button:focus-visible, [tabindex]:focus-visible { outline: 2px solid var(--color-primary, #D1F56A); outline-offset: 3px; }
  .toggle { border: 0; background: transparent; padding-left: 0; font-weight: 700; white-space: nowrap; }
  .toggle span { padding-right: 8px; }
  .close { margin-left: auto; font-size: 20px; background: transparent; border: 0; }
  .state { color: var(--color-muted, #A0A89D); font-size: 13px; }
  .state[data-state=succeeded] { color: var(--color-success, #A4D5AA); }
  .state[data-state=held], .hold { color: var(--color-warning, #E4C17B); }
  .run-id { font: 13px var(--font-mono, FreeMono, monospace); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 240px; }
  .count, .secondary { color: var(--color-secondary, #8A9485); font-size: 13px; }
  .run-controls { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; padding: 8px 16px; border-top: 1px solid var(--color-divider, #343A33); }
  .message, .hold, .error { margin: 0; padding: 8px 16px; overflow-wrap: anywhere; }
  .error { color: var(--color-danger, #D7A49B); }
  .drawer-body { display: grid; grid-template-columns: 260px minmax(0, 1fr); flex: 1; min-height: 120px; border-top: 1px solid var(--color-divider, #343A33); }
  .block-list { overflow: auto; padding: 16px; border-right: 1px solid var(--color-divider, #343A33); }
  h3 { font-size: 14px; margin: 0 0 12px; }
  .block-list h3 { color: var(--color-secondary, #8A9485); font-size: 13px; font-weight: 400; }
  .block-list button { display: block; width: 100%; text-align: left; background: transparent; border-color: transparent; margin-bottom: 4px; overflow-wrap: anywhere; }
  .block-list button[aria-current] { background: var(--color-selection-fill, #2B3521); border-color: var(--color-control-border, #79846F); }
  .block-list button span { display: block; font-size: 13px; color: var(--color-muted, #A0A89D); margin-top: 4px; }
  .block-list button.snapshot { margin-top: 16px; border-color: var(--color-control-border, #79846F); }
  .run-content { min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  .tabs { display: flex; gap: 8px; flex-shrink: 0; overflow-x: auto; padding: 0 16px; border-bottom: 1px solid var(--color-divider, #343A33); }
  .tabs button { background: transparent; border: 0; border-bottom: 2px solid transparent; border-radius: 0; padding: 12px 8px; }
  .tabs button[aria-selected=true] { border-bottom-color: var(--color-primary, #D1F56A); }
  .tab-content { padding: 20px 24px; overflow: auto; min-height: 0; }
  .output-heading { display: flex; justify-content: space-between; align-items: start; gap: 16px; }
  h4 { margin: 16px 0 8px; font-size: 14px; }
  pre { white-space: pre-wrap; overflow-wrap: anywhere; font: 13px/1.6 var(--font-mono, FreeMono, monospace); }
  p { margin: 8px 0; overflow-wrap: anywhere; }
  @media (max-width: 1199px), (max-height: 700px) {
    .run-workspace.expanded { height: min(620px, 75dvh); max-height: 75dvh; }
    .drawer-body { grid-template-columns: 190px minmax(0, 1fr); }
    .run-strip { flex-wrap: wrap; gap: 8px; padding-block: 4px; }
    .count { display: none; }
  }
  @media (max-width: 650px) {
    .drawer-body { display: flex; flex-direction: column; overflow: auto; }
    .block-list { max-height: 160px; flex-shrink: 0; border-right: 0; border-bottom: 1px solid var(--color-divider, #343A33); }
    .run-content { min-height: 240px; flex-shrink: 0; }
    .tab-content { padding: 16px; }
    .run-id { max-width: 120px; }
  }
</style>
