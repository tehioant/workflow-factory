<script lang="ts">
 import { Handle, Position, type NodeProps } from '@xyflow/svelte';
 let {data}: NodeProps = $props();
</script>
<Handle type="target" position={Position.Left} aria-label="Connect to this block" />
<div class="ai-card">
  <div class="ai-card-heading"><span class="node-icon" aria-hidden="true">✣</span><small>AI BLOCK</small>{#if data.ordinal}<small>{String(data.ordinal).padStart(2,'0')}</small>{/if}</div>
  <strong title={String(data.label)}>{String(data.label)}</strong>
  <p class="node-preview" title={String(data.prompt ?? '')}>{String(data.prompt ?? '').trim() || 'Write a focused prompt'}</p>
  <footer><span aria-hidden="true">◈</span> {String(data.configuration ?? 'Not configured')}</footer>
</div>
<Handle type="source" position={Position.Right} aria-label="Connect from this block" />
