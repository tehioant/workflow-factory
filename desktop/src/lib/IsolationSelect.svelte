<script lang="ts">
 import type { IsolationMode } from './transport.ts';
 let {value='perRun',disabled=false,onchange}: {value?:IsolationMode;disabled?:boolean;onchange:(mode:IsolationMode)=>void}=$props();
</script>
<label for="workflow-isolation">Agent isolation</label>
<select id="workflow-isolation" {disabled} {value} onchange={(event)=>onchange(event.currentTarget.value==='perBlock'?'perBlock':'perRun')}>
 <option value="perRun">One agent per run</option>
 <option value="perBlock">Fresh agent per block</option>
</select>
