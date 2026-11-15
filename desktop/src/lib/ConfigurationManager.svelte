<script lang="ts">
 import { onMount } from 'svelte';
 import type { Configuration, ProviderDescriptor, SavedConfiguration } from './transport.ts';
 import type { Commands } from './commands.ts';
 import { errorMessage } from './editor.ts';
 let {configurations,commands,onchanged,locked=false}:{configurations:SavedConfiguration[];commands:Commands;onchanged:()=>Promise<void>;locked?:boolean}=$props();
 let draft=$state<Configuration|null>(null), revision=$state<number|null>(null);
 let dirty=$state(false),busy=$state(false),error=$state(''),confirmDelete=$state(false);
 let providers=$state<ProviderDescriptor[]>([]),descriptorError=$state('');
 onMount(()=>{let alive=true;void commands.providerDescriptors().then(value=>{if(alive)providers=value;}).catch(()=>{if(alive)descriptorError='Provider descriptions unavailable. Direct creation is disabled; retry by reopening the application.';});return()=>{alive=false;};});
 function choose(saved:SavedConfiguration){
  if(('kind' in saved.config || 'schemaVersion' in saved.config) && (!('kind' in saved.config) || saved.config.kind!=='directModel'|| !('schemaVersion' in saved.config) || saved.config.schemaVersion!==2)){error='Unsupported configuration kind/version. No settings were changed.';return;}
  draft=structuredClone($state.snapshot(saved.config));revision=saved.revision;dirty=false;error='';confirmDelete=false;
 }
 function createDirect(){
  if(locked||busy||dirty)return;
  const provider=providers.find(p=>p.id==='openaiResponses');if(!provider){error='OpenAI Responses descriptor unavailable. No alternate provider was selected.';return;}
  draft={schemaVersion:2,kind:'directModel',id:crypto.randomUUID(),name:'New configuration',provider:provider.id,model:'',maxOutputTokens:1024,credential:{kind:'environment',variable:provider.credentialVariable}};revision=null;dirty=true;error='';
 }
 const validation=$derived.by(()=>{
  const config=draft;
  if(!config)return '';
  if(!config.name.trim()||!config.model)return 'Enter a configuration name and an explicit model identifier.';
  if('kind' in config){
   const provider=providers.find(p=>p.id===config.provider);
   if(!provider||config.credential.kind!=='environment'||config.credential.variable!==provider.credentialVariable)return 'The selected provider binding is unavailable or incompatible.';
   if(!Number.isInteger(config.maxOutputTokens)||config.maxOutputTokens<1||config.maxOutputTokens>65536)return 'Maximum output tokens must be an integer from 1 to 65536.';
  }
  return '';
 });
 export function shutdownState(){return {dirty,busy,identity:JSON.stringify([draft,revision,dirty,busy])};}
 export async function saveDraft(){
  if(busy)return false;if(!dirty)return true;if(!draft)return false;
  if(validation){error=validation;return false;}
  const captured=JSON.stringify([draft,revision]);busy=true;
  try{const saved=await commands.saveConfiguration({config:structuredClone($state.snapshot(draft)),expectedRevision:revision});if(captured!==JSON.stringify([draft,revision]))return false;choose(saved);await onchanged();return true;}catch(e){error=errorMessage(e);return false;}finally{busy=false;}
 }
</script>
<svelte:window onbeforeunload={(event)=>{if(dirty)event.preventDefault();}}/>
<section aria-label="Reusable model configurations" inert={locked}>
 <h2>Model configurations</h2>
 <p class="muted">Direct model: prompt → response, no tools, shell, files, workspace or container. API usage is separately billed from chat subscriptions. Saving does not require a key or check model availability. Never paste credentials.</p>
 {#each configurations as saved}<button disabled={locked||busy||dirty} onclick={()=>choose(saved)}>{saved.config.name} · revision {saved.revision}</button>{/each}
 <button disabled={locked||busy||dirty||!providers.some(p=>p.id==='openaiResponses')} onclick={createDirect}>New direct model configuration</button>
 <button disabled={locked||busy||dirty} onclick={()=>{draft={id:crypto.randomUUID(),name:'New configuration',model:'',reasoning:'medium'};revision=null;dirty=true;error='';}}>New legacy Codex configuration</button>
 <button disabled={locked||busy||dirty} onclick={async()=>{busy=true;try{await onchanged();error='';}catch(e){error=errorMessage(e);}finally{busy=false;}}}>Refresh configurations</button>
 {#if descriptorError}<p role="alert">{descriptorError}</p>{/if}
 {#if draft}
 <label for="config-name">Configuration name</label><input id="config-name" disabled={locked||busy} value={draft.name} oninput={(e)=>{if(draft)draft={...draft,name:e.currentTarget.value};dirty=true;}}/>
 {#if 'kind' in draft}
 <label for="config-provider">Direct model provider</label><select id="config-provider" disabled={locked||busy} value={draft.provider} onchange={event=>{const provider=providers.find(p=>p.id===event.currentTarget.value);if(provider&&draft&&'kind' in draft){draft={...draft,provider:provider.id,model:'',maxOutputTokens:1024,credential:{kind:'environment',variable:provider.credentialVariable}};dirty=true;}}}>{#each providers as provider (provider.id)}<option value={provider.id}>{provider.label}</option>{/each}</select>
 <p>Environment binding: <code>{draft.credential.variable}</code>. Supply it to the native application's launch environment; this is not a key manager.</p>
 <label for="config-tokens">Maximum output tokens (1–65536)</label><input id="config-tokens" type="number" min="1" max="65536" step="1" disabled={locked||busy} value={draft.maxOutputTokens} oninput={event=>{if(draft&&'kind' in draft)draft={...draft,maxOutputTokens:event.currentTarget.valueAsNumber};dirty=true;}}/>
 {:else}
 <p>Legacy Codex agent behavior · managed session and container runtime, not a direct model provider.</p>
 <label for="config-reasoning">Reasoning effort</label><select id="config-reasoning" disabled={locked||busy} bind:value={draft.reasoning} onchange={()=>dirty=true}><option value="minimal">Minimal</option><option value="low">Low</option><option value="medium">Medium</option><option value="high">High</option><option value="xhigh">Extra high</option></select>
 {/if}
 <label for="config-model">Model identifier</label><input id="config-model" disabled={locked||busy} value={draft.model} oninput={(e)=>{if(draft)draft={...draft,model:e.currentTarget.value};dirty=true;}}/>
 {#if validation}<p role="status">{validation}</p>{/if}
 <button disabled={locked||busy||!dirty||!!validation} onclick={()=>saveDraft()}>Save configuration</button>
 <button disabled={locked||busy} onclick={()=>{draft=null;revision=null;dirty=false;error='';confirmDelete=false;}}>Discard configuration edits / close</button>
 <button disabled={locked||busy||dirty||revision===null} onclick={()=>confirmDelete=true}>Delete configuration…</button>
 {#if confirmDelete}<p>Delete this saved configuration? References in saved workflows prevent deletion.</p><button disabled={locked||busy} onclick={async()=>{if(!draft||revision===null||locked)return;busy=true;try{await commands.deleteConfiguration(draft.id,revision);draft=null;revision=null;confirmDelete=false;await onchanged();}catch(e){error=errorMessage(e);}finally{busy=false;}}}>Confirm delete configuration</button><button disabled={locked||busy} onclick={()=>confirmDelete=false}>Keep configuration</button>{/if}
 {#if dirty}<p role="status">Unsaved configuration edits. Save or discard before choosing another configuration.</p>{/if}
 {/if}
 {#if error}<p role="alert">{error}</p>{/if}
</section>
