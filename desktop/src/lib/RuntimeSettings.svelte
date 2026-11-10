<script lang="ts">
  import type { Commands } from './commands.ts';
  import type { Readiness, SetupPlan } from './transport.ts';
  import { errorMessage } from './editor.ts';
  let { commands }: { commands: Commands } = $props();
  let readiness = $state<Readiness | null>(null);
  let plan = $state<SetupPlan | null>(null);
  let busy = $state(false);
  let error = $state('');
  async function check() {
    busy = true; error = '';
    try { readiness = await commands.runtimeReadiness(); }
    catch (e) { readiness = null; error = errorMessage(e); }
    finally { busy = false; }
  }
  async function showPlan() {
    busy = true; error = '';
    try { plan = await commands.runtimeSetupPlan(); }
    catch (e) { plan = null; error = errorMessage(e); }
    finally { busy = false; }
  }
</script>
<section aria-label="Runtime settings">
  <h2>Local runtime settings</h2>
  <p>Not checked automatically. Uses the backend-selected local Unix Docker socket only; no Docker context or credentials are read. Execution remains auth-blocked regardless of runtime readiness.</p>
  <button disabled={busy} onclick={check}>Check local Docker readiness</button>
  <button disabled={busy} onclick={showPlan}>Show setup instructions (does not execute)</button>
  {#if busy}<p role="status">Checking…</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
  {#if readiness}
    <p role="status">{readiness.status}: {readiness.message}</p>
    <p>Isolation evidence: {readiness.isolation} · Image: {readiness.imageId ?? 'not observed'}</p>
  {:else}<p>No current runtime observation.</p>{/if}
  {#if plan}
    <p>{plan.instructions}</p>
    <p>Program: <code>{plan.program}</code></p>
    <p>Arguments (JSON array, not a shell command):</p><pre>{JSON.stringify(plan.argv, null, 2)}</pre>
    <p>Install/start Docker explicitly if needed. This panel never installs, builds, pulls, configures auth or runs the plan.</p>
  {/if}
</section>
