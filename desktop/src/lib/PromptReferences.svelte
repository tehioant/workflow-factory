<script lang="ts">
  import { tick } from 'svelte';
  import { sequence } from './editor-presentation.ts';
  import type { Definition } from './transport.ts';
  let { definition, blockId, field, disabled = false, onprompt }: { definition: Definition; blockId: string; field: HTMLTextAreaElement | undefined; disabled?: boolean; onprompt: (prompt: string) => void } = $props();
  let open = $state(false), choice = $state('');
  let start = 0, end = 0;
  let trigger: HTMLButtonElement;
  let menu = $state<HTMLDivElement>();
  async function cancel() { open = false; await tick(); trigger?.focus(); }
  function key(event: KeyboardEvent) {
    if (open && event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); void cancel(); }
  }
  let predecessors = $derived.by(() => { const order = sequence(definition); return order.slice(0, Math.max(0, order.indexOf(blockId))).map(id => definition.blocks.find(block => block.id === id)!); });
  async function show() {
    if (open) { await cancel(); return; }
    start = field?.selectionStart ?? 0; end = field?.selectionEnd ?? start;
    open = true; await tick(); menu?.querySelector('select')?.focus();
  }
  async function insert() {
    if (disabled || !field) return;
    const block = definition.blocks.find(block => block.id === blockId);
    const valid = definition.inputs.some(input => choice === `input.${input.name}`) || predecessors.some(block => choice === `output.${block.id}.finalResponse`);
    if (!block || !valid) return;
    const literal = `{{${choice}}}`;
    onprompt(block.prompt.slice(0, start) + literal + block.prompt.slice(end));
    open = false; await tick(); field.focus(); field.setSelectionRange(start + literal.length, start + literal.length);
  }
</script>
<div class="reference-picker">
  <button bind:this={trigger} class="text-button" {disabled} aria-expanded={open} onclick={show}>Insert reference ▾</button>
  {#if open}
    <div bind:this={menu} class="reference-menu" onkeydowncapture={key} role="dialog" tabindex="-1" aria-label="Insert prompt reference">
      <label for="prompt-reference">Input or predecessor response</label>
      <select id="prompt-reference" bind:value={choice} {disabled}>
        <option value="">Choose a reference</option>
        <optgroup label="Named inputs">{#each definition.inputs as input}<option value={`input.${input.name}`}>{input.name}</option>{/each}</optgroup>
        <optgroup label="Predecessor final responses">{#each predecessors as block}<option value={`output.${block.id}.finalResponse`}>{block.name} · final response</option>{/each}</optgroup>
      </select>
      <p class="muted">Only connected predecessors are eligible. References resolve literally when the run starts.</p>
      <button disabled={disabled || !choice} onclick={insert}>Insert at saved caret</button>
      <button onclick={cancel}>Cancel</button>
    </div>
  {/if}
</div>
