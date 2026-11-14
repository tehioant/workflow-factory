<script lang="ts">
  import PromptReferences from './PromptReferences.svelte';
  import { sequence } from './editor-presentation.ts';
  import type { Definition, SavedConfiguration, AiBlock } from './transport.ts';
  let { definition, block, configurations, disabled, onpatch, onremove }: { definition: Definition; block: AiBlock; configurations: SavedConfiguration[]; disabled: boolean; onpatch: (patch: Partial<Pick<AiBlock, 'name' | 'prompt' | 'configurationId' | 'position'>>) => void; onremove: () => void } = $props();
  let field = $state<HTMLTextAreaElement>();
  let configError = $derived.by(() => {
    if (!block.configurationId) return 'Not configured. Choose a saved configuration before running.';
    if (!configurations.some(saved => saved.config.id === block.configurationId)) return 'This configuration is unavailable. Select an existing configuration or clear the reference.';
    return '';
  });
  let promptError = $derived.by(() => {
    const order = sequence(definition), predecessors = order.slice(0, Math.max(0, order.indexOf(block.id)));
    let rest = block.prompt;
    while (rest.includes('{{')) {
      const tail = rest.slice(rest.indexOf('{{') + 2), end = tail.indexOf('}}');
      if (end < 0) return 'Unclosed prompt reference. Close it with }}.';
      const token = tail.slice(0, end);
      if (token.startsWith('input.')) {
        if (!definition.inputs.some(input => input.name === token.slice(6))) return 'Unknown named input. Update the reference or input declaration.';
      } else if (token.startsWith('output.') && token.endsWith('.finalResponse')) {
        if (!predecessors.includes(token.slice(7, -'.finalResponse'.length))) return 'Output references must select an existing connected predecessor, not this block or a later block.';
      } else return 'Use {{input.name}} or {{output.blockId.finalResponse}}; other prompt reference syntax is unsupported.';
      rest = tail.slice(end + 2);
    }
    return '';
  });
</script>
<div class="property-group"><label for="block-name">Block name</label><input id="block-name" {disabled} value={block.name} oninput={event => onpatch({ name: event.currentTarget.value })} /></div>
<div class="property-group"><label for="block-config">Codex configuration</label><select id="block-config" aria-invalid={configError ? true : undefined} aria-describedby={configError ? 'block-config-error' : undefined} {disabled} value={block.configurationId ?? ''} onchange={event => onpatch({ configurationId: event.currentTarget.value || null })}><option value="">Not configured</option>{#each configurations as saved}<option value={saved.config.id}>{saved.config.name}</option>{/each}</select>
  <p class="muted">Model and reasoning come from the saved reusable configuration, not credentials.</p>
  {#if configError}<p id="block-config-error" role="alert">{configError}</p>{:else}{@const config = configurations.find(saved => saved.config.id === block.configurationId)!}<p class="muted">{config.config.model} · {config.config.reasoning} · revision {config.revision}</p>{/if}
</div>
<div class="property-group"><div class="field-heading"><label for="prompt">Prompt</label><PromptReferences {definition} blockId={block.id} {field} {disabled} onprompt={prompt => onpatch({ prompt })} /></div><textarea bind:this={field} id="prompt" aria-invalid={promptError ? true : undefined} aria-describedby={promptError ? 'prompt-error' : 'prompt-guidance'} class="prompt-editor" rows="12" {disabled} value={block.prompt} oninput={event => onpatch({ prompt: event.currentTarget.value })}></textarea>
  <p id="prompt-guidance" class="muted">References resolve when the run starts. Connected blocks forward only explicitly referenced final responses.</p>
  {#if promptError}<p id="prompt-error" role="alert">{promptError}</p>{/if}
</div>
<details class="property-group"><summary>Advanced properties</summary><label for="position-x">X position</label><input id="position-x" type="number" {disabled} value={block.position.x} oninput={event => { if (Number.isFinite(event.currentTarget.valueAsNumber)) onpatch({ position: { ...block.position, x: event.currentTarget.valueAsNumber } }); }} /><label for="position-y">Y position</label><input id="position-y" type="number" {disabled} value={block.position.y} oninput={event => { if (Number.isFinite(event.currentTarget.valueAsNumber)) onpatch({ position: { ...block.position, y: event.currentTarget.valueAsNumber } }); }} /></details>
<div class="property-group"><button class="danger text-button" {disabled} onclick={onremove}>Remove block…</button></div>
