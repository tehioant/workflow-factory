<script lang="ts">
  import type { Commands } from './commands.ts';
  import type { ManagedSessionStatus } from './transport.ts';
  let { commands }: { commands: Commands } = $props();
  let status = $state<ManagedSessionStatus | null>(null);
  let busy = $state(false);
  let failed = $state(false);
  async function check() {
    if (busy) return;
    busy = true; failed = false; status = null;
    try { status = await commands.managedSessionStatus(); }
    catch { failed = true; }
    finally { busy = false; }
  }
</script>
<section aria-label="Managed authentication">
  <h2>Managed session availability</h2>
  <p>Checks app-owned session metadata only, not host credential files. This check does not sign in, validate credentials or authorize execution.</p>
  <p>Use “Open native managed sign-in” in Settings to add or recover an app-owned session.</p>
  <button disabled={busy} onclick={check}>Check managed sessions</button>
  <p aria-live="polite">{#if busy}Managed sessions: checking…{:else if status}Available: {status.available} · Withheld: {status.withheld}{:else}Managed sessions: not checked{/if}</p>
  {#if status}<p>{status.guidance}</p>{/if}
  <p>Session availability does not authorize execution. Run preparation and native authentication/security checks still apply.</p>
  {#if failed}<p role="alert">Managed session status unavailable. No execution authorized; retry manually.</p>{/if}
</section>
