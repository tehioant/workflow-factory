<script lang="ts">
  import type { Commands } from './commands.ts';
  import type { Definition, SavedDefinition } from './transport.ts';
  import { errorMessage } from './editor.ts';
  import { focusScope } from './editor-presentation.ts';
  let { commands, definition, revision, dirty, locked, pending, admissionError, onsave, onstart, onretry, onclose, onsettings }: { commands: Commands; definition: Definition; revision: number | null; dirty: boolean; locked: boolean; pending: boolean; admissionError: string; onsave: () => Promise<SavedDefinition | null>; onstart: (saved: SavedDefinition, supplied: Record<string, string>) => Promise<void>; onretry: () => Promise<void>; onclose: () => void; onsettings: () => void } = $props();
  let saved = $state<SavedDefinition | null>(null), values = $state<Record<string, string>>({});
  let working = $state(false), error = $state('');
  let alive = true;
  import { onDestroy } from 'svelte';
  onDestroy(() => { alive = false; });
  async function choose(save: boolean) {
    if (working || locked || pending) return;
    working = true; error = '';
    try {
      const result = save ? await onsave() : await commands.load(definition.id);
      if (!alive) return;
      if (!result) { error = 'Save failed. Your draft is retained; no run was requested.'; return; }
      if (result.definition.id !== definition.id) throw new Error('The saved definition does not match this workflow.');
      saved = result;
    } catch (e) { if (alive) error = errorMessage(e); }
    finally { if (alive) working = false; }
  }
  async function start() {
    if (!saved || working || locked || pending) return;
    working = true; error = '';
    try {
      const captured = $state.snapshot(saved), supplied = $state.snapshot(values);
      await commands.prepare(captured.definition, supplied);
      if (alive) await onstart(captured, supplied);
    } catch (e) { if (alive) error = errorMessage(e); }
    finally { if (alive) working = false; }
  }
</script>
<div class="modal-backdrop">
  <div class="workspace-dialog" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="prepare-title" use:focusScope={{ enabled: true, close: () => { if (!working && !locked) onclose(); } }}>
    <header class="panel-heading"><div><p class="eyebrow">RUN PREPARATION</p><h2 id="prepare-title">Run {definition.name}</h2></div><button aria-label="Close run preparation" disabled={working || locked} onclick={onclose}>×</button></header>
    <div class="dialog-content">
      {#if pending}
        <p role="status">Admission outcome may be unknown. Only the original immutable request can be retried. Its revision and supplied values cannot be changed.</p>
        <button class="primary" disabled={locked || working} onclick={onretry}>Retry same request</button>
      {:else if !saved}
        <p>{dirty ? 'This draft has unsaved changes. Choose the revision to request; opening this dialog starts nothing.' : `Saved revision ${revision ?? 'unavailable'}. Choose it to supply run inputs.`}</p>
        {#if dirty}<button disabled={locked || working} onclick={() => choose(true)}>Save and run · prepare inputs</button>{/if}
        <button class="primary" disabled={locked || working || revision === null} onclick={() => choose(false)}>{dirty ? 'Run last saved revision' : 'Prepare saved revision'}</button>
        <button disabled={locked || working} onclick={onclose}>Stay</button>
      {:else}
        <p class="capture-label">{saved.definition.name} · revision {saved.revision}</p>
        <p class="muted">Inputs below belong to this saved revision, not unsaved declarations. Values are literal text.</p>
        {#each saved.definition.inputs as input}
          <label for={`run-input-${input.name}`}>{input.name} · {input.required ? 'Required' : 'Optional'}</label>
          <textarea id={`run-input-${input.name}`} disabled={working || locked} placeholder={input.default ?? 'No default'} value={values[input.name] ?? ''} oninput={event => { values = { ...values, [input.name]: event.currentTarget.value }; }}></textarea>
        {:else}<p>No named inputs are declared.</p>{/each}
        <button class="primary" disabled={working || locked} onclick={start}>{working ? 'Validating / requesting…' : `Start revision ${saved.revision}`}</button>
        <button disabled={working || locked} onclick={() => { saved = null; }}>Change revision choice</button>
      {/if}
      {#if error}<p role="alert">{error}</p>{/if}
      {#if admissionError}<p role="alert">{admissionError}</p>{/if}
      <p class="muted">Execution still requires native runtime, supported managed authentication and containment approval. A saved configuration is not a credential.</p>
      <button disabled={working || locked} onclick={onsettings}>Open Settings / readiness</button>
    </div>
  </div>
</div>
