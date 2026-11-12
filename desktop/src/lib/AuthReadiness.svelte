<script lang="ts">
 import type { Commands } from './commands.ts';
 import type { AuthReadiness } from './transport.ts';
 let {commands}:{commands:Commands}=$props();
 let readiness=$state<AuthReadiness|null>(null);
 let busy=$state(false);let failed=$state(false);
 async function check(){if(busy)return;busy=true;failed=false;readiness=null;try{readiness=await commands.authReadiness();}catch{failed=true;}finally{busy=false;}}
</script>
<section aria-label="Execution readiness">
 <h2>Execution unavailable</h2><button disabled>Run workflow (blocked)</button>
 <p>Manual inspection only: existing ~/.codex/auth.json. No custom home, keyring, config or environment credential discovery. Never paste or convert credentials for this app.</p>
 <button disabled={busy} onclick={check}>Check Codex credential availability</button>
 <p aria-live="polite">Credential availability: {busy?'checking':readiness?.credentialAvailability??'notInspected'}</p>
 <p>{readiness?.guidance??'Container authentication/staging and managed-cache refresh/write-back are not implemented here. The documented app-owned serialized cache route requires trusted-private scope and secure native storage/staging in #29. Availability is not validity or authentication.'}</p>
 {#if readiness}<p>Credential source: {readiness.credentialSource} · Container route: {readiness.status}</p>{/if}
 {#if failed}<p role="alert">Credential inspection unavailable. No execution enabled; retry manually.</p>{/if}
</section>
