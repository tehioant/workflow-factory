<script lang="ts">
 import { invoke } from '@tauri-apps/api/core';
 import { signInCommands } from './signin.ts';
 let { commands=signInCommands(invoke) }: {commands?:ReturnType<typeof signInCommands>} = $props();
 import { onMount } from 'svelte';
 import type { SignInStatus } from './transport.ts';
 import { errorMessage } from './editor.ts';
 let status=$state<SignInStatus|null>(null);let busy=$state(true);let error=$state('');
 let alive=true;
 async function action(command:string){busy=true;error='';try{const value=await (command==='begin_session_sign_in'?commands.begin():command==='read_session_sign_in'?commands.read():command==='retire_session_sign_in'?commands.retire():commands.cancel());if(alive)status=value;}catch(e){if(alive)error=errorMessage(e);}finally{if(alive)busy=false;}}
 onMount(()=>{let timer:ReturnType<typeof setTimeout>;async function poll(){if(!alive)return;if(status?.state==='pending'&&!busy)await action('read_session_sign_in');if(alive)timer=setTimeout(poll,2000);}void action('read_session_sign_in');timer=setTimeout(poll,2000);return()=>{alive=false;clearTimeout(timer);status=null;};});
</script>
<main class="notice">
 <h1>App-owned Codex managed session</h1>
 <p>For trusted private workloads only. This separately authorizes this app; it never imports your ordinary host login. Separate logins do not guarantee independent upstream refresh families.</p>
 <p>Sign in only on the official provider page in your browser. Never send a device code or credentials in chat. No API-key or paid fallback.</p>
 <button disabled={busy||['pending','cleanupRequired'].includes(status?.state??'')} onclick={()=>action('begin_session_sign_in')}>Begin explicit managed sign-in</button>
 <button disabled={busy||!['pending','cleanupRequired'].includes(status?.state??'')} onclick={()=>action('cancel_session_sign_in')}>Cancel setup</button>
 <button disabled={busy||['pending','cleanupRequired'].includes(status?.state??'')} onclick={()=>action('retire_session_sign_in')}>Retire quarantined session (fresh sign-in required)</button>
 {#if status}<p role="status">{status.state}: {status.message}</p>{/if}
 {#if status?.url&&status?.code}<p>Official sign-in URL: {status.url}</p><p>Transient device code: <strong>{status.code}</strong></p>{/if}
 {#if error}<p role="alert">{error}</p>{/if}
</main>
