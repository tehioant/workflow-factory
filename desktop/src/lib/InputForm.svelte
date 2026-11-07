<script lang="ts">
 import type { Definition, PreparedInputs } from './transport.ts';
 import type { Commands } from './commands.ts';
 import { errorMessage } from './editor.ts';
 let {definition,commands}:{definition:Definition;commands:Commands}=$props();
 let values=$state<Record<string,string>>({}); let prepared=$state<PreparedInputs|null>(null); let error=$state(''); let busy=$state(false);
 $effect(()=>{definition; values={}; prepared=null; error='';});
</script>
<form onsubmit={async(e)=>{e.preventDefault();busy=true;prepared=null;try{prepared=await commands.prepare(definition,$state.snapshot(values));error='';}catch(e){error=errorMessage(e);}finally{busy=false;}}}>
 <h3>Prepare input preview</h3><p class="muted">Literal text only. This does not execute any block.</p>
 {#each definition.inputs as input}
 <label for={`supplied-${input.name}`}>{input.name} {input.required?'(required)':'(optional)'}</label><textarea id={`supplied-${input.name}`} disabled={busy} placeholder={input.default??'No default'} value={values[input.name]??''} oninput={(e)=>{values={...values,[input.name]:e.currentTarget.value};prepared=null;}}></textarea>
 {/each}
 <button disabled={busy} type="submit">Prepare values</button>
 {#if error}<p role="alert">{error}</p>{/if}
 {#if prepared}{#each Object.entries(prepared.prompts) as [id,prompt]}<h4>Prepared prompt · {id}</h4><pre>{prompt}</pre>{/each}{/if}
</form>
