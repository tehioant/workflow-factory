<script lang="ts">
  import PromptReferences from './PromptReferences.svelte';
  import type { Definition, SavedConfiguration, AiBlock } from './transport.ts';
  let { definition, block, configurations, disabled, onpatch, onremove }: { definition: Definition; block: AiBlock; configurations: SavedConfiguration[]; disabled: boolean; onpatch: (patch: Partial<Pick<AiBlock, 'name' | 'prompt' | 'configurationId' | 'position'>>) => void; onremove: () => void } = $props();
  let field = $state<HTMLTextAreaElement>();
  let invalidReferences = $derived.by(() => {
    const tokens = [...block.prompt.matchAll(/\{\{input\.([^}]+)\}\}/g)].map(match => match[1]);
    return tokens.filter(name => !definition.inputs.some(input => input.name === name));
  });
</script>
<div class="property-group"><label for="block-name">Block name</label><input id="block-name" {disabled} value={block.name} oninput={event => onpatch({ name: event.currentTarget.value })} /></div>
<div class="property-group"><label for="block-config">Codex configuration</label><select id="block-config" {disabled} value={block.configurationId ?? ''} onchange={event => onpatch({ configurationId: event.currentTarget.value || null })}><option value="">Not configured</option>{#each configurations as saved}<option value={saved.config.id}>{saved.config.name}</option>{/each}</select>
  <p class="muted">Model and reasoning come from the saved reusable configuration, not credentials.</p>
  {#if !block.configurationId}<p class="warning">Not configured. Choose a saved configuration before running.</p>{:else if !configurations.some(saved => saved.config.id === block.configurationId)}<p role="alert">This configuration is unavailable. Select an existing configuration or clear the reference.</p>{:else}{@const config = configurations.find(saved => saved.config.id === block.configurationId)!}<p class="muted">{config.config.model} · {config.config.reasoning} · revision {config.revision}</p>{/if}
</div>
<div class="property-group"><div class="field-heading"><label for="prompt">Prompt</label><PromptReferences {definition} blockId={block.id} {field} {disabled} onprompt={prompt => onpatch({ prompt })} /></div><textarea bind:this={field} id="prompt" class="prompt-editor" rows="12" {disabled} value={block.prompt} oninput={event => onpatch({ prompt: event.currentTarget.value })}></textarea>
  <p class="muted">References resolve when the run starts. Connected blocks forward only explicitly referenced final responses.</p>
  {#if invalidReferences.length}<p role="alert">Unknown named inputs: {invalidReferences.join(', ')}. Update references or declarations.</p>{/if}
</div>
<details class="property-group"><summary>Advanced properties</summary><label for="position-x">X position</label><input id="position-x" type="number" {disabled} value={block.position.x} oninput={event => { if (Number.isFinite(event.currentTarget.valueAsNumber)) onpatch({ position: { ...block.position, x: event.currentTarget.valueAsNumber } }); }} /><label for="position-y">Y position</label><input id="position-y" type="number" {disabled} value={block.position.y} oninput={event => { if (Number.isFinite(event.currentTarget.valueAsNumber)) onpatch({ position: { ...block.position, y: event.currentTarget.valueAsNumber } }); }} /></details>
<div class="property-group"><button class="danger text-button" {disabled} onclick={onremove}>Remove block…</button></div>
