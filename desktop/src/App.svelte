<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Button from './lib/ui/button.svelte';
  import { createCommands } from './lib/commands.ts';
  import { emptyEditor, saveEditor, openEditor, errorMessage } from './lib/editor.ts';
  import type { SavedDefinition } from './lib/transport.ts';
  const commands = createCommands(invoke);
  let editor = $state(emptyEditor());
  let definitions = $state<SavedDefinition[]>([]);
  let busy = $state(false);
  let listError = $state('');
  let confirmDiscard = $state(false);
  let pendingId = $state<string | null>(null);

  async function refresh() {
    try { definitions = await commands.list(); listError = ''; }
    catch (error) { listError = errorMessage(error); }
  }
  onMount(() => { busy = true; void refresh().finally(() => { busy = false; }); });
  async function navigate(id: string | null, discard = false) {
    if (editor.dirty && !discard) { pendingId = id; confirmDiscard = true; return; }
    confirmDiscard = false;
    busy = true;
    try {
      const clean = discard ? { ...editor, dirty: false } : editor;
      if (id) {
        const opened = await openEditor(clean, id, commands);
        // Failed load must retain the old working copy, including its dirty flag.
        editor = opened.error ? { ...editor, error: opened.error } : opened;
      } else {
        editor = { working: { schemaVersion: 1, id: crypto.randomUUID(), name: 'Untitled workflow' }, revision: null, dirty: true, error: '' };
      }
    } finally { busy = false; }
  }
  async function save() {
    busy = true;
    try { editor = await saveEditor(editor, commands); if (!editor.dirty) await refresh(); }
    finally { busy = false; }
  }
</script>

<svelte:window onbeforeunload={(event) => { if (editor.dirty) event.preventDefault(); }} />
<div class="app-shell">
  <aside aria-label="Saved workflows">
    <p class="eyebrow">WORKFLOW FACTORY</p>
    <h1>Workflows</h1>
    <Button disabled={busy} onclick={() => navigate(null)}>New workflow</Button>
    <Button variant="secondary" disabled={busy} onclick={async () => { busy = true; try { await refresh(); } finally { busy = false; } }}>Refresh list</Button>
    {#if listError}<p role="alert">{listError}</p>{/if}
    <ul>{#each definitions as saved (saved.definition.id)}
      <li><button class="workflow-link" disabled={busy} aria-current={editor.working?.id === saved.definition.id ? 'page' : undefined} onclick={() => navigate(saved.definition.id)}>{saved.definition.name}<small>Revision {saved.revision}</small></button></li>
    {:else}<li class="muted">No saved desktop workflows.</li>{/each}</ul>
    <p class="muted">Private desktop storage · separate from browser data</p>
  </aside>
  <main aria-busy={busy}>
    <header><span class="eyebrow">DESKTOP / DEFINITION</span><span role="status">{busy ? 'Working…' : editor.dirty ? 'Unsaved changes' : editor.revision ? `Saved · revision ${editor.revision}` : 'Ready'}</span></header>
    {#if confirmDiscard}
      <section class="notice" aria-label="Unsaved changes">
        <h2>Keep your edits?</h2><p>Save first, stay here, or explicitly discard this working copy.</p>
        <Button disabled={busy} onclick={() => { confirmDiscard = false; }}>Stay</Button>
        <Button variant="secondary" disabled={busy} onclick={() => navigate(pendingId, true)}>Discard and continue</Button>
      </section>
    {/if}
    {#if editor.working}
      <section class="editor">
        <label for="workflow-name">Workflow name</label>
        <input id="workflow-name" maxlength="200" disabled={busy} value={editor.working.name} oninput={(event) => { if (editor.working) editor = { ...editor, working: { ...editor.working, name: event.currentTarget.value }, dirty: true, error: '' }; }} />
        <p class="muted">Definition foundation. Canvas editing is a separate increment.</p>
        <Button disabled={busy || !editor.dirty} onclick={save}>Save workflow</Button>
        {#if editor.error}<p role="alert">{editor.error}</p>{/if}
        <p class="muted">To resolve a revision conflict, keep/copy your name or explicitly discard and reopen the saved workflow from the list. Saving never forces an overwrite.</p>
      </section>
    {:else}
      <section class="editor"><h2>Your workflows, saved locally.</h2><p>Create a workflow or open one from the sidebar.</p></section>
    {/if}
  </main>
</div>
