<script lang="ts">
 import { Handle, Position, type NodeProps } from '@xyflow/svelte';
 let {data}: NodeProps = $props();
</script>
<Handle type="target" position={Position.Left} />
<div class="ai-card"><small>AI BLOCK</small><strong>{String(data.label)}</strong></div>
<Handle type="source" position={Position.Right} />
