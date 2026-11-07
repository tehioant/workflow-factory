<script lang="ts">
 import type { Definition, PreparedInputs } from './transport.ts';
 import type { Commands } from './commands.ts';
 import { errorMessage } from './editor.ts';
 let {definition,commands}:{definition:Definition;commands:Commands}=$props();
 let values=$state<Record<string,string>>({}); let prepared=$state<PreparedInputs|null>(null); let error=$state(''); let busy=$state(false);
 let generation = 0;
 const requestKey = () => JSON.stringify({ definition, values });
 $effect(()=>{JSON.stringify(definition); generation++; values={}; prepared=null; error=''; busy=false;});
 async function prepare(event: SubmitEvent) {
   event.preventDefault();
   const request = ++generation;
   const key = requestKey();
   const current = () => request === generation && key === requestKey();
   busy=true; prepared=null; error='';
   try {
     const result = await commands.prepare($state.snapshot(definition), $state.snapshot(values));
     if(current()) prepared=result;
   } catch(e) { if(current()) error=errorMessage(e); }
   finally { if(current()) busy=false; }
 }
</script>
<form onsubmit={prepare}>
 <h3>Prepare input preview</h3><p class="muted">Literal text only. This does not execute any block.</p>
 {#each definition.inputs as input}
 <label for={`supplied-${input.name}`}>{input.name} {input.required?'(required)':'(optional)'}</label><textarea id={`supplied-${input.name}`} disabled={busy} placeholder={input.default??'No default'} value={values[input.name]??''} oninput={(e)=>{values={...values,[input.name]:e.currentTarget.value};generation++;prepared=null;error='';busy=false;}}></textarea>
 {/each}
 <button disabled={busy} type="submit">Prepare values</button>
 {#if error}<p role="alert">{error}</p>{/if}
 {#if prepared}{#each Object.entries(prepared.prompts) as [id,prompt]}<h4>Prepared prompt · {id}</h4><pre>{prompt}</pre>{/each}{/if}
</form>
