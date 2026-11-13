<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import ShutdownDialog from './lib/ShutdownDialog.svelte';
  import Canvas from './lib/Canvas.svelte';
  import DefinitionProperties from './lib/DefinitionProperties.svelte';
  import BlockInspector from './lib/BlockInspector.svelte';
  import RunPreparation from './lib/RunPreparation.svelte';
  import ConfigurationManager from './lib/ConfigurationManager.svelte';
  import RuntimeSettings from './lib/RuntimeSettings.svelte';
  import AuthReadinessPanel from './lib/AuthReadiness.svelte';
  import QueuePanel from './lib/QueuePanel.svelte';
  import RunPanel from './lib/RunPanel.svelte';
  import RunHistory from './lib/RunHistory.svelte';
  import { createCommands } from './lib/commands.ts';
  import { emptyEditor, editGraph, saveEditor, openEditor, errorMessage } from './lib/editor.ts';
  import { focusScope } from './lib/editor-presentation.ts';
  import type { Definition, SavedDefinition, SavedConfiguration, StartRun, RunStatus, QueueEntry } from './lib/transport.ts';
  const commands = createCommands(invoke);
  let editor = $state(emptyEditor()), configurations = $state<SavedConfiguration[]>([]), definitions = $state<SavedDefinition[]>([]);
  let busy = $state(false), listError = $state(''), configError = $state('');
  let destination = $state<'workflows' | 'runs' | 'configurations' | 'settings'>('workflows');
  let search = $state(''), selected = $state(''), inspector = $state(true), browser = $state(true), width = $state(1920);
  let block = $derived(editor.working?.blocks.find(block => block.id === selected));
  let filtered = $derived(definitions.filter(saved => saved.definition.name.toLocaleLowerCase().includes(search.toLocaleLowerCase())));
  let shutdownVisible = $state(false), shutdownFrozen = $state(false), shutdownSubscriptionFailed = $state(false);
  let configurationManager = $state<ConfigurationManager>();
  let configDraft = $derived(configurationManager?.shutdownState() ?? { dirty: false, busy: false, identity: '' });
  let shutdownIdentity = $derived(JSON.stringify([editor, configDraft.identity, busy]));
  let confirmDiscard = $state(false), pendingId = $state<string | null>(null);
  let removing = $state(false), removeReferences = $state(false);
  let dependentReferences = $derived(editor.working?.blocks.filter(item => item.id !== selected && item.prompt.includes(`{{output.${selected}.finalResponse}}`)) ?? []);
  let dependentConnections = $derived(editor.working?.connections.filter(edge => edge.source === selected || edge.target === selected).length ?? 0);
  let preparation = $state(false), run = $state<RunStatus | null>(null), runReady = $state(false), runError = $state('');
  let pendingStart = $state<StartRun | null>(null), activeInspectionWorkflow = $state<string | null>(null), runOpen = $state(false);
  let runSelection = 0;
  const locked = () => shutdownFrozen || busy;
  async function refresh() { try { definitions = await commands.list(); listError = ''; } catch (error) { listError = errorMessage(error); } }
  async function refreshConfigurations() { configurations = await commands.listConfigurations(); }
  onMount(() => {
    let alive = true, stop: (() => void) | undefined;
    void listen('shutdown-requested', () => { if (alive) shutdownVisible = true; }).then(value => { if (alive) stop = value; else value(); }).catch(() => { if (alive) shutdownSubscriptionFailed = true; });
    busy = true;
    void (async () => { await refresh(); try { await refreshConfigurations(); configError = ''; } catch (error) { configError = errorMessage(error); } })().finally(() => { if (alive) busy = false; });
    void commands.runStatus().then(value => { if (alive) { run = value; runReady = true; } }).catch(error => { if (alive) runError = errorMessage(error); });
    return () => { alive = false; stop?.(); };
  });
  async function save() {
    if (locked()) return;
    busy = true;
    try { editor = await saveEditor($state.snapshot(editor), commands); if (!editor.dirty) await refresh(); }
    finally { busy = false; }
  }
  async function saveShutdownDrafts() { if (busy || configDraft.busy) return false; if (editor.dirty) await save(); if (editor.dirty) return false; return await configurationManager?.saveDraft() ?? true; }
  async function navigate(id: string | null, discard = false) {
    if (locked()) return;
    if (editor.dirty && !discard) { pendingId = id; confirmDiscard = true; return; }
    confirmDiscard = false; busy = true;
    try {
      if (id) { const opened = await openEditor($state.snapshot(discard ? { ...editor, dirty: false } : editor), id, commands); editor = opened.error ? { ...editor, error: opened.error } : opened; if (opened.error) return; }
      else editor = { working: { schemaVersion: 1, id: crypto.randomUUID(), name: 'Untitled workflow', isolation: 'perRun', blocks: [], connections: [], inputs: [] }, revision: null, dirty: true, error: '' };
      selected = ''; inspector = true; destination = 'workflows'; if (width < 1600) browser = false;
    } finally { busy = false; }
  }
  function changeDefinition(definition: Definition) { if (!locked()) editor = { ...editor, working: definition, dirty: true, error: '' }; }
  function selectBlock(id: string) { if (!busy) { selected = id; inspector = true; if (width < 1600) browser = false; } }
  function addBlock() { if (locked() || !editor.working) return; const id = crypto.randomUUID(); editor = editGraph(editor, { type: 'add', id }); editor = editGraph(editor, { type: 'update', id, patch: { position: { x: 80 + (editor.working!.blocks.length - 1) * 360, y: 160 } } }); selectBlock(id); }
  function removeBlock() {
    if (locked() || (dependentReferences.length && !removeReferences)) return;
    if (editor.working && removeReferences) changeDefinition({ ...editor.working, blocks: editor.working.blocks.map(item => ({ ...item, prompt: item.prompt.split(`{{output.${selected}.finalResponse}}`).join('') })) });
    editor = editGraph(editor, { type: 'remove', id: selected }); selected = ''; removing = false; removeReferences = false;
  }
  async function begin(saved?: SavedDefinition, supplied?: Record<string, string>) {
    if (locked() || !runReady || (pendingStart && saved) || (!pendingStart && !saved)) return;
    busy = true; runError = '';
    try {
      if (saved && !pendingStart) {
        if (!saved.definition.blocks.length) throw new Error('Add and save an AI block before requesting a run.');
        const available = await commands.listConfigurations();
        const revisions = Object.fromEntries(saved.definition.blocks.map(block => { const config = available.find(value => value.config.id === block.configurationId); if (!config) throw new Error('Select and save an available configuration for every block. Open Configurations to resolve it.'); return [config.config.id, config.revision]; }));
        pendingStart = { requestId: crypto.randomUUID(), definitionId: saved.definition.id, expectedRevision: saved.revision, expectedConfigurationRevision: revisions[saved.definition.blocks[0].configurationId!], expectedConfigurationRevisions: revisions, supplied: structuredClone(supplied ?? {}) };
      }
      if (!pendingStart) return;
      run = await commands.startRun($state.snapshot(pendingStart)); activeInspectionWorkflow = pendingStart.definitionId; pendingStart = null; preparation = false; runOpen = true;
    } catch (error) { runError = errorMessage(error); }
    finally { busy = false; }
  }
  async function selectQueued(entry: QueueEntry) { const generation = ++runSelection; try { const value = await commands.runStatus(entry.id); if (generation === runSelection && value?.id === entry.id) { run = value; activeInspectionWorkflow = entry.definitionId; runError = ''; runOpen = true; destination = 'workflows'; } } catch (error) { if (generation === runSelection) runError = errorMessage(error); } }
  function showBrowser() { browser = !browser; if (browser && width < 1600) inspector = false; }
  function showProperties() { selected = ''; inspector = true; if (width < 1600) browser = false; }
</script>

<svelte:window bind:innerWidth={width} onbeforeunload={event => { if (editor.dirty || configDraft.dirty) event.preventDefault(); }} />
{#if shutdownSubscriptionFailed}<section class="shutdown-warning"><p role="alert">Native shutdown requests are unavailable. Keep the app open or use safe shutdown.</p><button onclick={() => { shutdownVisible = true; }}>Open safe shutdown</button></section>{/if}
{#if shutdownVisible}<div class:shutdown-modal={!shutdownFrozen}><ShutdownDialog {commands} dirty={editor.dirty || configDraft.dirty} busy={busy || configDraft.busy} identity={shutdownIdentity} save={saveShutdownDrafts} onstay={() => { shutdownVisible = false; }} onfreeze={() => { shutdownFrozen = true; }} /></div>{/if}
<div class="app-shell" class:browser-open={browser} class:inspector-open={inspector && destination === 'workflows'} inert={(shutdownVisible && !shutdownFrozen) || preparation || confirmDiscard || removing}>
  <nav class="navigation-rail" aria-label="Main destinations">
    <div class="factory-mark" aria-hidden="true">F</div>
    {#each [{ id: 'workflows', label: 'Workflows', icon: '⌘' }, { id: 'runs', label: 'Runs', icon: '▷' }, { id: 'configurations', label: 'Configurations', icon: '☷' }, { id: 'settings', label: 'Settings', icon: '⚙' }] as item}
      <button title={item.label} aria-label={item.label} aria-current={destination === item.id ? 'page' : undefined} onclick={() => { destination = item.id as typeof destination; }}>{item.icon}</button>
    {/each}
    <button class="rail-bottom" title="Safe shutdown" aria-label="Safe shutdown" onclick={() => { shutdownVisible = true; }}>⏻</button>
  </nav>
  {#if browser}
    <aside class="workflow-browser" aria-label="Saved workflows" use:focusScope={{ enabled: width < 1200, close: () => { browser = false; } }}>
      <div class="browser-heading"><p class="eyebrow">WORKFLOW FACTORY</p><div class="field-heading"><h1>Workflows</h1><button class="text-button" aria-label="Close workflow browser" onclick={() => { browser = false; }}>×</button></div><label class="sr-only" for="workflow-search">Find a workflow</label><input id="workflow-search" type="search" placeholder="Find a workflow" bind:value={search} /><button class="full-width" disabled={locked()} onclick={() => navigate(null)}>＋ New workflow</button></div>
      <div class="workflow-list"><p class="eyebrow">YOUR WORKFLOWS</p>{#if listError}<p role="alert">{listError}</p>{/if}<button class="text-button" disabled={busy} onclick={refresh}>Refresh workflows</button>
        {#if busy && !definitions.length}<p role="status">Loading workflows…</p>{:else}{#each filtered as saved (saved.definition.id)}<button class="workflow-link" disabled={locked()} aria-current={editor.working?.id === saved.definition.id ? 'page' : undefined} onclick={() => navigate(saved.definition.id)}><span aria-hidden="true">⌘</span><span>{saved.definition.name}<small>{saved.definition.blocks.length} AI blocks · revision {saved.revision}</small></span></button>{:else}<p class="muted">{definitions.length ? 'No matching workflows.' : listError ? 'Workflow list unavailable.' : 'No saved desktop workflows.'}</p>{/each}{/if}
        <section class="palette"><p class="eyebrow">BUILD</p><button class="palette-block" disabled={locked() || !editor.working} onclick={addBlock}><span class="node-icon" aria-hidden="true">✣</span><span>AI block<small>Add to canvas</small></span><span aria-hidden="true">＋</span></button><p class="muted">One prompt. One focused step.</p></section>
      </div><footer class="local-context"><span aria-hidden="true">•</span> Local workspace<small>Definitions saved on this device</small></footer>
    </aside>
  {/if}
  <main class="workspace" aria-busy={busy}>
    <header class="workspace-header"><div class="header-title"><button class="browser-trigger" aria-label="Toggle workflow browser" aria-expanded={browser} onclick={showBrowser}>☰</button><div><p class="eyebrow">WORKFLOWS / {destination === 'workflows' ? 'EDITOR' : destination.toUpperCase()}</p><button class="workflow-title" disabled={!editor.working} onclick={showProperties}>{editor.working?.name ?? 'Your local workflows'} <span aria-hidden="true">⌄</span></button></div><span class="save-state" role="status">{busy ? 'Working…' : editor.error ? 'Save / validation failed' : editor.dirty ? editor.revision ? 'Unsaved changes' : 'New · unsaved' : editor.revision ? `Saved · revision ${editor.revision}` : 'Ready'}</span></div>
      <div class="header-actions"><button disabled={locked() || !editor.working || !editor.dirty} onclick={save}>Save</button><button class="primary" disabled={locked() || !runReady || !editor.working} aria-describedby="run-availability" onclick={() => { preparation = true; }}>▷ Run workflow</button></div>
    </header>
    <span id="run-availability" class="sr-only">{!runReady ? 'Native run status is not available. Check Settings; authoring remains available.' : 'Opens input and saved revision preparation; execution is subject to native readiness.'}</span>
    {#if editor.error}<div class="context-error" role="alert">{editor.error}<p>Draft retained. For revision conflicts, copy edits or explicitly discard and reopen; saving never forces overwrite.</p></div>{/if}
    <div class="destination editor-destination" hidden={destination !== 'workflows'}>
      <div class="editor-body">
        <section class="canvas-region" aria-label="Definition editor">
          <div class="canvas-toolbar"><span class="eyebrow">WORKFLOW CANVAS</span><div class="field-heading"><div><button disabled={locked() || !editor.working} onclick={addBlock}>＋ Add block</button><button title="Workflow properties" disabled={!editor.working} onclick={showProperties}>Workflow properties</button></div><span class="muted">{editor.working?.blocks.length ?? 0} blocks</span></div></div>
          {#if editor.working}<Canvas definition={editor.working} configurations={configurations} selectedId={selected} disabled={locked()} onchange={changeDefinition} onselect={selectBlock} />{#if !editor.working.blocks.length}<div class="canvas-empty"><h2>Start with one focused step.</h2><p>Add an AI block, write its prompt, then connect your sequence.</p><button disabled={locked()} onclick={addBlock}>＋ Add AI block</button></div>{/if}{:else}<div class="canvas-empty"><h2>Your workflows, saved locally.</h2><p>Open a workflow or create a new definition to begin.</p><button disabled={locked()} onclick={() => navigate(null)}>New workflow</button></div>{/if}
        </section>
        {#if inspector && editor.working}<aside class="context-inspector" aria-label={block ? 'Block inspector' : 'Workflow properties'} use:focusScope={{ enabled: width < 1200, close: () => { inspector = false; } }}><header class="panel-heading"><div><h2>{block ? 'Block inspector' : 'Workflow properties'}</h2><p class="muted">{block ? 'AI block · editable draft' : 'Settings for this definition'}</p></div><button aria-label="Close inspector" onclick={() => { inspector = false; }}>×</button></header><div class="inspector-content">{#if block}{#key block.id}<BlockInspector definition={editor.working} {block} {configurations} disabled={locked()} onpatch={patch => { if (!locked()) editor = editGraph(editor, { type: 'update', id: selected, patch }); }} onremove={() => { removeReferences = false; removing = true; }} />{/key}{:else}<DefinitionProperties definition={editor.working} disabled={locked()} onchange={changeDefinition} />{/if}</div></aside>{/if}
      </div>
      <section class="legacy-run-drawer" aria-label="Run details"><button class="run-strip" aria-expanded={runOpen} onclick={() => { runOpen = !runOpen; }}><strong>◉ Run details</strong><span>{run ? `${run.id} · ${run.state}` : 'No run selected'}</span><span>{runOpen ? '⌄' : '⌃'}</span></button>{#if runOpen}<div class="legacy-run-content">{#key run?.id}<RunPanel {commands} {run} definitionId={activeInspectionWorkflow} onstatus={value => { run = value; }} />{/key}</div>{/if}</section>
    </div>
    <div class="destination secondary-destination" hidden={destination !== 'runs'}><h1>Runs</h1><QueuePanel {commands} onselect={selectQueued} admissionError={runError} /><h2>Workflow history</h2><p class="muted">Select a saved workflow in the browser to inspect its retained history.</p>{#if editor.working}{#key editor.working.id}<RunHistory {commands} definitionId={editor.working.id} />{/key}{/if}</div>
    <div class="destination secondary-destination" hidden={destination !== 'configurations'}><ConfigurationManager bind:this={configurationManager} locked={shutdownFrozen} {configurations} {commands} onchanged={refreshConfigurations} />{#if configError}<p role="alert">{configError}</p>{/if}</div>
    {#if destination === 'settings'}<div class="destination secondary-destination"><h1>Settings & readiness</h1><RuntimeSettings {commands} /><AuthReadinessPanel {commands} /><button disabled={shutdownFrozen || busy} onclick={() => { void commands.openSessionSignIn().catch(error => { runError = errorMessage(error); }); }}>Open native managed sign-in</button>{#if runError}<p role="alert">{runError}</p>{/if}<button onclick={() => { shutdownVisible = true; }}>Safe application shutdown</button></div>{/if}
  </main>
</div>
{#if confirmDiscard}<div class="modal-backdrop"><section class="workspace-dialog" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="unsaved-title" use:focusScope={{ enabled: true, close: () => { if (!busy) confirmDiscard = false; } }}><h2 id="unsaved-title">Keep your edits?</h2><p>Replacing this definition needs an explicit decision. A failed save keeps your draft here.</p><button disabled={locked()} onclick={() => { confirmDiscard = false; }}>Stay</button><button disabled={locked()} onclick={async () => { await save(); if (!editor.dirty) await navigate(pendingId, true); }}>Save and continue</button><button class="danger" disabled={locked()} onclick={() => navigate(pendingId, true)}>Discard and continue</button>{#if editor.error}<p role="alert">{editor.error}</p>{/if}</section></div>{/if}
{#if preparation && editor.working}<RunPreparation {commands} definition={editor.working} revision={editor.revision} dirty={editor.dirty} locked={locked()} pending={pendingStart !== null} admissionError={runError} onsave={async () => { await save(); return !editor.dirty && editor.working && editor.revision !== null ? { definition: $state.snapshot(editor.working), revision: editor.revision } : null; }} onstart={begin} onretry={() => begin()} onclose={() => { if (!pendingStart) preparation = false; }} onsettings={() => { preparation = false; destination = 'settings'; }} />{/if}
{#if removing && block}<div class="modal-backdrop"><section class="workspace-dialog" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="remove-title" use:focusScope={{ enabled: true, close: () => { removing = false; } }}><h2 id="remove-title">Remove {block.name}?</h2><p>This removes this block and {dependentConnections} affected connections. {dependentReferences.length} other prompts reference its output.</p>{#if dependentReferences.length}<p>{dependentReferences.map(item => item.name).join(', ')}</p><label><input type="checkbox" bind:checked={removeReferences} /> Explicitly remove this block’s output reference tokens from those prompts.</label>{/if}<button disabled={locked()} onclick={() => { removing = false; }}>Keep block</button><button class="danger" disabled={locked() || (dependentReferences.length > 0 && !removeReferences)} onclick={removeBlock}>Confirm remove block</button></section></div>{/if}
