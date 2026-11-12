<script lang="ts">
  import RunHistory from './lib/RunHistory.svelte';
  import { onMount } from 'svelte';
  import IsolationSelect from './lib/IsolationSelect.svelte';
  import RunPanel from './lib/RunPanel.svelte';
  import OutputReferencePicker from './lib/OutputReferencePicker.svelte';
  import type { RunStatus, StartRun } from './lib/transport.ts';
  import AuthReadinessPanel from './lib/AuthReadiness.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Button from './lib/ui/button.svelte';
  import Canvas from './lib/Canvas.svelte';
  import InputForm from './lib/InputForm.svelte';
  import RuntimeSettings from './lib/RuntimeSettings.svelte';
  import ConfigurationManager from './lib/ConfigurationManager.svelte';
  import { insertInputReference } from './lib/inputs.ts';
  import { editGraph } from './lib/editor.ts';
  import { createCommands } from './lib/commands.ts';
  import { emptyEditor, saveEditor, openEditor, errorMessage } from './lib/editor.ts';
  import type { SavedConfiguration, SavedDefinition } from './lib/transport.ts';
  const commands = createCommands(invoke);
  let editor = $state(emptyEditor());
  let configurations = $state<SavedConfiguration[]>([]);
  let configError = $state('');
  async function refreshConfigurations(){configurations=await commands.listConfigurations();}
  let definitions = $state<SavedDefinition[]>([]);
  let busy = $state(false);
  let selected = $state('');
  let source = $state('');
  let inputChoice = $state('');
  let promptField = $state<HTMLTextAreaElement>();
  let target = $state('');
  let block = $derived(editor.working?.blocks.find(b=>b.id===selected));
  let listError = $state('');
  let confirmDiscard = $state(false);
  let pendingId = $state<string | null>(null);
  let supplied=$state<Record<string,string>>({});
  let run=$state<RunStatus|null>(null);
  let runReady=$state(false);
  let runError=$state('');
  let dirtyStart=$state(false);
  let pendingStart=$state<StartRun|null>(null);
  onMount(()=>{let alive=true;void commands.runStatus().then(value=>{if(alive){run=value;runReady=true;}}).catch(error=>{if(alive)runError=errorMessage(error);});return()=>{alive=false;};});
  async function begin(mode:'save'|'saved'|'retry'){
    if(busy||!runReady||(mode!=='retry'&&(!editor.working||pendingStart!==null)))return;
    const inputs=$state.snapshot(supplied);
    busy=true;runError='';
    try{
      if(mode!=='retry'){
        if(mode==='save'){
          editor=await saveEditor($state.snapshot(editor),commands);
          if(editor.dirty){runError=editor.error;return;}
          await refresh();
        }
        if(!editor.working)return;
        const saved=await commands.load(editor.working.id);
        if(!saved.definition.blocks.length)throw new Error('Add and save an AI block.');
        const config=configurations.find(c=>c.config.id===saved.definition.blocks[0].configurationId);
        if(!config)throw new Error('Select and save an available Codex configuration.');
        const revisions=Object.fromEntries(saved.definition.blocks.map(block=>{const captured=configurations.find(c=>c.config.id===block.configurationId);if(!captured)throw new Error('Select and save an available configuration for every block.');return [captured.config.id,captured.revision];}));
        pendingStart={requestId:crypto.randomUUID(),definitionId:saved.definition.id,expectedRevision:saved.revision,expectedConfigurationRevision:config.revision,expectedConfigurationRevisions:revisions,supplied:inputs};
      }
      if(!pendingStart)return;
      run=await commands.startRun($state.snapshot(pendingStart));pendingStart=null;dirtyStart=false;
    }catch(error){runError=errorMessage(error);}
    finally{busy=false;}
  }

  async function refresh() {
    try { definitions = await commands.list(); listError = ''; }
    catch (error) { listError = errorMessage(error); }
  }
  onMount(() => { busy = true; void (async()=>{await refresh();try{await refreshConfigurations();configError='';}catch(error){configError=errorMessage(error);}})().finally(() => { busy = false; }); });
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
        editor = { working: { schemaVersion: 1, id: crypto.randomUUID(), name: 'Untitled workflow', isolation: 'perRun', blocks: [], connections: [], inputs: [] }, revision: null, dirty: true, error: '' };
      }
    } finally { busy = false; }
  }
  async function save() {
    busy = true;
    try { editor = await saveEditor($state.snapshot(editor), commands); if (!editor.dirty) await refresh(); }
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
    <section aria-label="Block palette"><h2>Block palette</h2>
      <Button disabled={busy||!editor.working} onclick={()=>{const id=crypto.randomUUID();editor=editGraph(editor,{type:'add',id});selected=id;}}>Add AI block</Button>
      <p class="muted">AI blocks only. Connect them in workflow properties; edit each prompt in the inspector.</p>
    </section>
    <ConfigurationManager {configurations} {commands} onchanged={refreshConfigurations}/>
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
        <h2>Workflow properties</h2>
        <IsolationSelect value={editor.working.isolation} disabled={busy} onchange={(isolation)=>{if(editor.working)editor={...editor,working:{...editor.working,isolation},dirty:true,error:''};}}/>
        <h3>Named text inputs</h3>
        {#each editor.working.inputs as input,i}
         <label for={`input-name-${i}`}>Input name</label><input id={`input-name-${i}`} disabled={busy} value={input.name} oninput={(e)=>{if(editor.working)editor={...editor,working:{...editor.working,inputs:editor.working.inputs.map((v,n)=>n===i?{...v,name:e.currentTarget.value}:v)},dirty:true};}} />
         <label><input type="checkbox" disabled={busy} checked={input.required} onchange={(e)=>{if(editor.working)editor={...editor,working:{...editor.working,inputs:editor.working.inputs.map((v,n)=>n===i?{...v,required:e.currentTarget.checked}:v)},dirty:true};}} /> Required</label>
         <label for={`input-default-${i}`}>Default (empty means no default)</label><textarea id={`input-default-${i}`} disabled={busy} value={input.default??''} oninput={(e)=>{if(editor.working)editor={...editor,working:{...editor.working,inputs:editor.working.inputs.map((v,n)=>n===i?{...v,default:e.currentTarget.value||null}:v)},dirty:true};}}></textarea>
         <button disabled={busy} onclick={()=>{if(editor.working)editor={...editor,working:{...editor.working,inputs:editor.working.inputs.filter((_,n)=>n!==i)},dirty:true};}}>Remove input {i+1}</button>
        {/each}
        <Button disabled={busy} onclick={()=>{if(editor.working)editor={...editor,working:{...editor.working,inputs:[...editor.working.inputs,{name:`input_${editor.working.inputs.length+1}`,required:true,default:null}]},dirty:true};}}>Add text input</Button>
        <InputForm definition={editor.working} {commands} onvalues={(values)=>{supplied=values;}}/>


        <label for="block-select">Select block</label><select id="block-select" disabled={busy} bind:value={selected}><option value="">Choose a block</option>{#each editor.working.blocks as b}<option value={b.id}>{b.name}</option>{/each}</select>
        <label for="source">Connect from</label><select id="source" disabled={busy} bind:value={source}><option value="">Source block</option>{#each editor.working.blocks as b}<option value={b.id}>{b.name}</option>{/each}</select>
        <label for="target">Connect to</label><select id="target" disabled={busy} bind:value={target}><option value="">Target block</option>{#each editor.working.blocks as b}<option value={b.id}>{b.name}</option>{/each}</select>
        <Button disabled={busy||!source||!target} onclick={()=>{editor=editGraph(editor,{type:'connect',source,target});}}>Connect blocks</Button>
        {#each editor.working.connections as c,i}<p>{editor.working.blocks.find(b=>b.id===c.source)?.name} → {editor.working.blocks.find(b=>b.id===c.target)?.name} <button disabled={busy} onclick={()=>{if(editor.working) editor={...editor,working:{...editor.working,connections:editor.working.connections.filter((_,index)=>index!==i)},dirty:true};}}>Remove connection {i+1}</button></p>{/each}
        <Button disabled={busy || !editor.dirty} onclick={save}>Save workflow</Button>
        {#if editor.error}<p role="alert">{editor.error}</p>{/if}
        <p class="muted">To resolve a revision conflict, keep/copy your name or explicitly discard and reopen the saved workflow from the list. Saving never forces an overwrite.</p>
      </section>
      <Canvas definition={editor.working} disabled={busy} onchange={(d)=>{if(!busy) editor={...editor,working:d,dirty:true,error:''};}} onselect={(id)=>{if(!busy) selected=id;}}/>
      <section class="inspector" aria-label="Block inspector">
       <h2>Block inspector</h2>
       {#if block}
        <label for="block-name">Block name</label><input id="block-name" disabled={busy} value={block.name} oninput={(e)=>editor=editGraph(editor,{type:'update',id:selected,patch:{name:e.currentTarget.value}})} />
        <label for="prompt">Own prompt</label><textarea bind:this={promptField} id="prompt" rows="12" disabled={busy} value={block.prompt} oninput={(e)=>editor=editGraph(editor,{type:'update',id:selected,patch:{prompt:e.currentTarget.value}})}></textarea>
        <OutputReferencePicker definition={editor.working} blockId={selected} field={promptField} disabled={busy} onprompt={(prompt)=>{editor=editGraph(editor,{type:'update',id:selected,patch:{prompt}});}}/>
        <label for="block-config">Codex configuration</label><select id="block-config" disabled={busy} value={block.configurationId??''} onchange={(e)=>editor=editGraph(editor,{type:'update',id:selected,patch:{configurationId:e.currentTarget.value||null}})}><option value="">Not configured</option>{#each configurations as saved}<option value={saved.config.id}>{saved.config.name} · {saved.config.model} / {saved.config.reasoning}</option>{/each}</select>
        {#if block.configurationId&&!configurations.some(c=>c.config.id===block?.configurationId)}<p role="alert">Missing configuration. Select an existing one or clear this reference.</p>{/if}
        <label for="input-reference">Insert named input</label><select id="input-reference" disabled={busy} bind:value={inputChoice}><option value="">Choose input</option>{#each editor.working.inputs as input}<option value={input.name}>{input.name}</option>{/each}</select>
        <Button disabled={busy||!inputChoice} onclick={()=>{if(block){editor=editGraph(editor,{type:'update',id:selected,patch:{prompt:insertInputReference(block.prompt,promptField?.selectionStart??block.prompt.length,promptField?.selectionEnd??block.prompt.length,inputChoice)}});promptField?.focus();}}}>Insert reference</Button>
        <label for="position-x">X position</label><input id="position-x" type="number" disabled={busy} value={block.position.x} oninput={(e)=>{if(block)editor=editGraph(editor,{type:'update',id:selected,patch:{position:{...block.position,x:e.currentTarget.valueAsNumber}}});}} />
        <label for="position-y">Y position</label><input id="position-y" type="number" disabled={busy} value={block.position.y} oninput={(e)=>{if(block)editor=editGraph(editor,{type:'update',id:selected,patch:{position:{...block.position,y:e.currentTarget.valueAsNumber}}});}} />
        <Button disabled={busy} onclick={()=>{editor=editGraph(editor,{type:'remove',id:selected});selected='';}}>Remove AI block</Button>
       {:else}<p>Select a block on the canvas or in properties.</p>{/if}
      </section>
    {:else}
      <section class="editor"><h2>Your workflows, saved locally.</h2><p>Create a workflow or open one from the sidebar.</p></section>
    {/if}
    <section class="notice" aria-label="Start run">
      <button disabled={busy||!runReady||!editor.working||pendingStart!==null} onclick={()=>{if(editor.dirty)dirtyStart=true;else void begin('saved');}}>Start saved workflow</button>
      <button disabled={busy} onclick={()=>{void commands.openSessionSignIn().catch(error=>{runError=errorMessage(error);});}}>Open native managed sign-in</button>
      <p>Trusted private workloads only. Native secure staging, managed login and containment must be qualified before execution.</p>
      {#if dirtyStart}
      <h2>Unsaved changes</h2><p>Save and start, explicitly use the last saved revision without discarding edits, or stay.</p>
      <button disabled={busy||pendingStart!==null} onclick={()=>begin('save')}>Save and start</button>
      <button disabled={busy||pendingStart!==null||!editor.revision} onclick={()=>begin('saved')}>Start last saved revision</button>
      <button disabled={busy||pendingStart!==null} onclick={()=>{dirtyStart=false;}}>Stay</button>
      {/if}
      {#if pendingStart}<p>Admission outcome may be unknown. Retry the same immutable request, not a new start.</p><button disabled={busy} onclick={()=>begin('retry')}>Retry same request</button>{/if}
      {#if runError}<p role="alert">{runError}</p>{/if}
    </section>
    {#key run?.id}<RunPanel {commands} {run} onstatus={(value)=>{run=value;}}/>{/key}
    {#if editor.working}{#key editor.working.id}<RunHistory {commands} definitionId={editor.working.id}/>{/key}{/if}
    <RuntimeSettings {commands}/>
    <AuthReadinessPanel {commands}/>
    {#if configError}<p role="alert">{configError}</p>{/if}
  </main>
</div>
