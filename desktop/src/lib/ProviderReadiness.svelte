<script lang="ts">
 import { onDestroy } from 'svelte';
 import type { Commands } from './commands.ts';
 import type { DirectProviderId, ProviderDescriptor, ProviderReadiness } from './transport.ts';
 import { errorMessage } from './editor.ts';
 let {commands,locked=false}:{commands:Commands;locked?:boolean}=$props();
 let providers=$state<ProviderDescriptor[]>([]),selected=$state<DirectProviderId|''>('');
 let result=$state<ProviderReadiness|null>(null),busy=$state(false),error=$state('');
 let alive=true,generation=0;
 onDestroy(()=>{alive=false;generation++;});
 async function load(){if(busy||locked)return;const request=++generation;busy=true;error='';try{const value=await commands.providerDescriptors();if(alive&&request===generation)providers=value;}catch(e){if(alive&&request===generation)error=errorMessage(e);}finally{if(alive&&request===generation)busy=false;}}
 async function check(){
  if(busy||locked||!providers.some(p=>p.id===selected))return;
  const provider=selected as DirectProviderId,request=++generation;busy=true;error='';result=null;
  try{const value=await commands.providerReadiness(provider);if(alive&&request===generation&&selected===provider){if(value.provider!==provider)throw Error('Provider readiness response identity changed.');result=value;}}
  catch(e){if(alive&&request===generation&&selected===provider)error=errorMessage(e);}
  finally{if(alive&&request===generation)busy=false;}
 }
</script>
<section aria-label="Direct model readiness">
 <h2>Direct model readiness</h2>
 <p>Prompt → response, no tools, shell, files, workspace or container. API usage is separately billed from chat subscriptions and needs native per-run charge confirmation.</p>
 <p>Binding checks are manual, local and selected-provider-only. They do not validate balance, model access or entitlement and do not send model requests. Supply the matching variable to the native application's launch environment; no key is entered or stored here.</p>
 <button disabled={busy||locked} onclick={load}>Load provider descriptions</button>
 <label for="readiness-provider">Provider to check</label><select id="readiness-provider" disabled={busy||locked} value={selected} onchange={event=>{selected=event.currentTarget.value as DirectProviderId;generation++;result=null;error='';}}><option value="">Choose provider</option>{#each providers as provider (provider.id)}<option value={provider.id}>{provider.label} · {provider.credentialVariable}</option>{/each}</select>
 <button disabled={busy||locked||!providers.some(p=>p.id===selected)} onclick={check}>Check selected provider binding</button>
 {#if result}<p role="status">{result.provider}: {result.credential} · {result.message}</p>{/if}
 {#if error}<p role="alert">{error}</p>{/if}
</section>
