<script lang="ts">
  import IsolationSelect from './IsolationSelect.svelte';
  import type { Definition } from './transport.ts';
  let { definition, disabled = false, onchange }: { definition: Definition; disabled?: boolean; onchange: (definition: Definition) => void } = $props();
  let source = $state(''), target = $state(''), error = $state('');
  let errorIndex = $state<number | null>(null), errorAction = $state<'name' | 'remove'>('name');
  function referenced(name: string) { return definition.blocks.some(block => block.prompt.includes(`{{input.${name}}}`)); }
  function inputChange(index: number, patch: Partial<Definition['inputs'][number]>) {
    const original = definition.inputs[index];
    if (patch.name !== undefined && patch.name !== original.name && referenced(original.name)) { errorIndex = index; errorAction = 'name'; error = `Edit prompt references to ${original.name} before renaming it. Templates are not silently rewritten.`; return; }
    error = ''; errorIndex = null; onchange({ ...definition, inputs: definition.inputs.map((input, i) => i === index ? { ...input, ...patch } : input) });
  }
  function removeInput(index: number) {
    if (referenced(definition.inputs[index].name)) { errorIndex = index; errorAction = 'remove'; error = `Edit prompt references to ${definition.inputs[index].name} before removing it.`; return; }
    error = ''; errorIndex = null; onchange({ ...definition, inputs: definition.inputs.filter((_, i) => i !== index) });
  }
</script>
<div class="property-group">
  <label for="workflow-name">Workflow name</label><input id="workflow-name" maxlength="200" {disabled} value={definition.name} oninput={event => onchange({ ...definition, name: event.currentTarget.value })} />
</div>
<section class="property-group"><h3>Named inputs</h3><p class="muted">Declarations only. Supply actual values when preparing a run.</p>
  {#each definition.inputs as input, index}
    {@const invalidName = !/^[A-Za-z_][A-Za-z0-9_]*$/.test(input.name)}
    {@const duplicate = definition.inputs.some((other, i) => i !== index && other.name === input.name)}
    <div class="input-card">
      <label for={`input-name-${index}`}>Input name</label><input id={`input-name-${index}`} aria-invalid={invalidName || duplicate || (errorIndex === index && errorAction === 'name') ? true : undefined} aria-describedby={[invalidName || duplicate ? `input-name-error-${index}` : '', errorIndex === index && errorAction === 'name' ? 'input-action-error' : ''].filter(Boolean).join(' ') || undefined} {disabled} value={input.name} oninput={event => inputChange(index, { name: event.currentTarget.value })} />
      {#if invalidName || duplicate}<p id={`input-name-error-${index}`} role="alert">{invalidName ? 'Use letters, digits and underscores; start with a letter or underscore.' : ''} {duplicate ? 'Input names must be unique.' : ''}</p>{/if}
      <label><input type="checkbox" {disabled} checked={input.required} onchange={event => inputChange(index, { required: event.currentTarget.checked })} /> Required</label>
      <label for={`input-default-${index}`}>Default · empty means none</label><textarea id={`input-default-${index}`} {disabled} value={input.default ?? ''} oninput={event => inputChange(index, { default: event.currentTarget.value || null })}></textarea>
      <button class="danger text-button" aria-describedby={errorIndex === index && errorAction === 'remove' ? 'input-action-error' : undefined} {disabled} onclick={() => removeInput(index)}>Remove input {input.name || index + 1}</button>
    </div>
  {/each}
  <button {disabled} onclick={() => { let number = definition.inputs.length + 1; while (definition.inputs.some(input => input.name === `input_${number}`)) number++; onchange({ ...definition, inputs: [...definition.inputs, { name: `input_${number}`, required: true, default: null }] }); }}>＋ Add text input</button>
  {#if error}<p id="input-action-error" role="alert">{error}</p>{/if}
</section>
<section class="property-group"><h3>Isolation</h3><IsolationSelect value={definition.isolation} {disabled} onchange={isolation => onchange({ ...definition, isolation })} /><p class="muted">Both choices share only this run’s private workspace and captured outputs. Blocks do not have separate filesystems.</p></section>
<section class="property-group"><h3>Connections</h3>
  {#each definition.connections as edge, index}<div class="connection-row"><span>{definition.blocks.find(block => block.id === edge.source)?.name ?? 'Missing block'} → {definition.blocks.find(block => block.id === edge.target)?.name ?? 'Missing block'}</span><button {disabled} aria-label={`Remove connection ${index + 1}`} onclick={() => onchange({ ...definition, connections: definition.connections.filter((_, i) => i !== index) })}>×</button></div>{:else}<p class="muted">No connections yet.</p>{/each}
  <details><summary>Manage connections</summary>
    <label for="source">From block</label><select id="source" {disabled} bind:value={source}><option value="">Choose source</option>{#each definition.blocks as block}<option value={block.id}>{block.name}</option>{/each}</select>
    <label for="target">To block</label><select id="target" {disabled} bind:value={target}><option value="">Choose target</option>{#each definition.blocks as block}<option value={block.id}>{block.name}</option>{/each}</select>
    <button disabled={disabled || !source || !target} onclick={() => onchange({ ...definition, connections: [...definition.connections, { source, target }] })}>Connect blocks</button>
    <p class="muted">Save validates sequential shape and refuses cycles, duplicate and invalid connections.</p>
  </details>
</section>
