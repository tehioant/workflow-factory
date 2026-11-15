<script lang="ts">
  import type { RunDetail } from './transport.ts';
  import { runStateLabel } from './run-presentation.ts';
  let { detail, blockId }: { detail: RunDetail; blockId: string } = $props();
  const block = $derived(detail.definition.definition.blocks.find(block => block.id === blockId));
  const capture = $derived(detail.blocks.find(block => block.blockId === blockId));
</script>
<section class="captured-block" aria-label="Captured block snapshot">
  {#if block}
    <p class="eyebrow">Read-only run snapshot</p>
    <h3>{block.name}</h3>
    <p class="identity">{block.id} · revision {detail.definition.revision}</p>
    <p>{capture?.result ? runStateLabel(capture.result.state) : detail.legacy ? 'Execution evidence unavailable' : 'Not started / result unavailable'}</p>
    <h4>Captured configuration</h4>
    {#if capture?.configuration}
      <p>{capture.configuration.config.name} · revision {capture.configuration.revision}</p>
      {#if 'kind' in capture.configuration.config && capture.configuration.config.kind === 'directModel'}
        <p class="secondary">{capture.configuration.config.provider} · {capture.configuration.config.model} · max output tokens {capture.configuration.config.maxOutputTokens}</p>
        <p>Direct model · prompt → response, no tools, shell, files, workspace or container.</p>
      {:else if !('kind' in capture.configuration.config)}
        <p class="secondary">Legacy Codex agent · {capture.configuration.config.model} / {capture.configuration.config.reasoning}</p>
      {:else}<p>Unsupported captured configuration kind. Current settings are not substituted.</p>{/if}
    {:else}
      <p>Per-block configuration unavailable. Current settings are not substituted.</p>
    {/if}
    {#if detail.legacy}
      <p>Legacy capture: per-block settings and executed prompts may be unavailable.</p>
      {#if detail.legacyConfiguration && !('kind' in detail.legacyConfiguration.config)}<p>Legacy run configuration: {detail.legacyConfiguration.config.name} · revision {detail.legacyConfiguration.revision} · {detail.legacyConfiguration.config.model} / {detail.legacyConfiguration.config.reasoning}</p>{/if}
    {/if}
    <h4>Actual retained resolved prompt</h4>
    {#if capture?.result}<pre>{capture.result.resolvedPrompt}</pre>{:else}<p>No retained executed prompt. A template is not proof of execution.</p>{/if}
    <details>
      <summary>Captured template and input preparation</summary>
      <h4>Captured template</h4><pre>{block.prompt}</pre>
      <h4>Input-prepared prompt · not execution evidence</h4><pre>{detail.inputs.prompts[blockId] ?? 'Unavailable'}</pre>
    </details>
  {:else}<p>Selected block is unavailable in this captured definition.</p>{/if}
</section>
<style>
  .captured-block { color: var(--color-text, #EDF1F8); min-width: 0; font-size: 14px; }
  h3 { font-size: 18px; margin: 4px 0 8px; overflow-wrap: anywhere; }
  h4 { font-size: 14px; margin: 24px 0 8px; }
  p { margin: 8px 0; overflow-wrap: anywhere; }
  .eyebrow { color: var(--color-secondary, #97A5BE); font-size: 11px; letter-spacing: .06em; text-transform: uppercase; }
  .identity, .secondary { color: var(--color-muted, #A6B0C3); font-size: 13px; }
  pre { font: 13px/1.6 var(--font-mono, FreeMono, monospace); white-space: pre-wrap; overflow-wrap: anywhere; padding: 12px; background: var(--color-canvas, #141824); border-radius: 6px; }
  details { margin-top: 24px; }
  summary { cursor: pointer; padding: 8px 0; }
  summary:focus-visible { outline: 2px solid var(--color-accent, #8DAEFF); outline-offset: 3px; }
</style>
