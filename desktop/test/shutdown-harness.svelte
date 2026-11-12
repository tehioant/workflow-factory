<script lang="ts">
 import ShutdownDialog from '../src/lib/ShutdownDialog.svelte';
 import type {Commands} from '../src/lib/commands.ts';
 let {commands,dirty,busy,save,onstay,onfreeze}:{commands:Commands;dirty:boolean;busy:boolean;save:()=>Promise<boolean>;onstay:()=>void;onfreeze:()=>void}=$props();
 let identity=$state('draft-1');
 export function changeIdentity(value:string){identity=value;}
</script>
<ShutdownDialog {commands} {dirty} {busy} {save} {onstay} {onfreeze} {identity}/>
