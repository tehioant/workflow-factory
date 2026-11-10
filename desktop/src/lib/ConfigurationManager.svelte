<script lang="ts">
 import type { CodexConfiguration, SavedConfiguration } from './transport.ts';
 import type { Commands } from './commands.ts';
 import { errorMessage } from './editor.ts';
 let {configurations,commands,onchanged}:{configurations:SavedConfiguration[];commands:Commands;onchanged:()=>Promise<void>}=$props();
 let draft=$state<CodexConfiguration|null>(null);let revision=$state<number|null>(null);let dirty=$state(false);let busy=$state(false);let error=$state('');let confirmDelete=$state(false);
 function choose(saved:SavedConfiguration){draft=structuredClone($state.snapshot(saved.config));revision=saved.revision;dirty=false;error='';confirmDelete=false;}
</script>
<svelte:window onbeforeunload={(event)=>{if(dirty)event.preventDefault();}}/>
<section aria-label="Reusable Codex configurations">
 <h2>Codex configurations</h2><p class="muted">Non-secret model/reasoning settings only. Model availability is not checked; never paste credentials.</p>
 {#each configurations as saved}<button disabled={busy||dirty} onclick={()=>choose(saved)}>{saved.config.name} · revision {saved.revision}</button>{/each}
 <button disabled={busy||dirty} onclick={()=>{draft={id:crypto.randomUUID(),name:'New configuration',model:'',reasoning:'medium'};revision=null;dirty=true;}}>New configuration</button>
 <button disabled={busy||dirty} onclick={async()=>{busy=true;try{await onchanged();error='';}catch(e){error=errorMessage(e);}finally{busy=false;}}}>Refresh configurations</button>
 {#if draft}
 <label for="config-name">Configuration name</label><input id="config-name" disabled={busy} value={draft.name} oninput={(e)=>{if(draft)draft={...draft,name:e.currentTarget.value};dirty=true;}}/>
 <label for="config-model">Model identifier</label><input id="config-model" disabled={busy} value={draft.model} oninput={(e)=>{if(draft)draft={...draft,model:e.currentTarget.value};dirty=true;}}/>
 <label for="config-reasoning">Reasoning effort</label><select id="config-reasoning" disabled={busy} bind:value={draft.reasoning} onchange={()=>dirty=true}><option value="minimal">Minimal</option><option value="low">Low</option><option value="medium">Medium</option><option value="high">High</option><option value="xhigh">Extra high</option></select>
 <button disabled={busy||!dirty} onclick={async()=>{if(!draft)return;busy=true;try{const saved=await commands.saveConfiguration({config:structuredClone($state.snapshot(draft)),expectedRevision:revision});choose(saved);await onchanged();}catch(e){error=errorMessage(e);}finally{busy=false;}}}>Save configuration</button>
 <button disabled={busy} onclick={()=>{draft=null;revision=null;dirty=false;error='';confirmDelete=false;}}>Discard configuration edits / close</button>
 <button disabled={busy||dirty||revision===null} onclick={()=>confirmDelete=true}>Delete configuration…</button>
 {#if confirmDelete}<p>Delete this saved configuration? References in saved workflows prevent deletion.</p><button disabled={busy} onclick={async()=>{if(!draft||revision===null)return;busy=true;try{await commands.deleteConfiguration(draft.id,revision);draft=null;revision=null;confirmDelete=false;await onchanged();}catch(e){error=errorMessage(e);}finally{busy=false;}}}>Confirm delete configuration</button><button disabled={busy} onclick={()=>confirmDelete=false}>Keep configuration</button>{/if}
 {#if dirty}<p role="status">Unsaved configuration edits. Save or discard before choosing another configuration.</p>{/if}
 {/if}
 {#if error}<p role="alert">{error}</p>{/if}
</section>
