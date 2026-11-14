<script lang="ts">
  import { onMount } from 'svelte';
  import type { Commands } from './commands.ts';
  import { errorMessage } from './editor.ts';
  let { commands }: { commands: Commands } = $props();
  let active = $state(2), waiting = $state(16), ready = $state(false), saving = $state(false), error = $state('');
  let alive = true;
  onMount(() => {
    void commands.queueStatus().then(value => { if (alive) { active = value.settings.active; waiting = value.settings.waiting; ready = true; } }).catch(e => { if (alive) error = errorMessage(e); });
    return () => { alive = false; };
  });
  async function save() {
    if (!ready || saving) return;
    saving = true; error = '';
    try { await commands.saveQueueSettings({ active, waiting }); }
    catch (e) { if (alive) error = errorMessage(e); }
    finally { if (alive) saving = false; }
  }
</script>
<section class="notice" aria-label="Queue capacities">
  <h2>Queue capacities</h2>
  <p>Global limits for active and waiting runs. Lower limits never discard existing runs.</p>
  <label for="queue-active">Active capacity (1–8)</label><input id="queue-active" type="number" min="1" max="8" step="1" disabled={!ready || saving} bind:value={active} />
  <label for="queue-waiting">Waiting capacity (0–64)</label><input id="queue-waiting" type="number" min="0" max="64" step="1" disabled={!ready || saving} bind:value={waiting} />
  <button disabled={!ready || saving} onclick={save}>Save capacities</button>
  {#if error}<p role="alert">{error}</p>{/if}
</section>
