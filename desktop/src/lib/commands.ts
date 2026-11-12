import type { Readiness, SetupPlan, SaveConfiguration, SavedConfiguration, AuthReadiness, Definition, PreparedInputs, SaveDefinition, SavedDefinition } from './transport.ts';
import { Channel } from '@tauri-apps/api/core';
export type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
export function createCommands(invoke: Invoke) {
  return {
    queueStatus:()=>invoke<import('./transport.ts').QueueStatus>('queue_status'),
    queueSettings:()=>invoke<import('./transport.ts').QueueSettings>('queue_settings'),
    saveQueueSettings:(settings:import('./transport.ts').QueueSettings)=>invoke<import('./transport.ts').QueueSettings>('save_queue_settings',{settings}),
    authorizeRun:(id:string)=>invoke<void>('authorize_run',{id}),
    inspectWorkflow:(run:string)=>invoke<string>('inspect_workflow',{run}),
    inspectSubscribe:(definitionId:string,run:string,onmessage:(notice:import('./transport.ts').Notice)=>void)=>{const channel=new Channel<import('./transport.ts').Notice>();channel.onmessage=onmessage;return invoke<import('./transport.ts').InspectionPage>('inspect_subscribe',{definitionId,run,channel});},
    inspectPage:(definitionId:string,run:string,subscription:string,cursor:string|null)=>invoke<import('./transport.ts').InspectionPage>('inspect_page',{definitionId,run,subscription,cursor}),
    inspectClose:(definitionId:string,run:string,subscription:string)=>invoke<void>('inspect_close',{definitionId,run,subscription}),
    inspectArtifacts:(definitionId:string,run:string,subscription:string,cursor:string|null)=>invoke<import('./transport.ts').ArtifactPage>('inspect_artifacts',{definitionId,run,subscription,cursor}),
    inspectArtifact:(definitionId:string,run:string,subscription:string,generation:string,id:string,cursor:string|null)=>invoke<import('./transport.ts').ArtifactText>('inspect_artifact',{definitionId,run,subscription,generation,id,cursor}),
    listRunHistory: (definitionId:string,cursor:string|null=null,limit=20) => invoke<import('./transport.ts').HistoryPage>('list_run_history',{definitionId,cursor,limit}),
    runDetail: (definitionId:string,id:string) => invoke<import('./transport.ts').RunDetail>('run_detail',{definitionId,id}),
    openSessionSignIn: () => invoke<void>('open_session_sign_in'),
    startRun: (request: import('./transport.ts').StartRun) => invoke<import('./transport.ts').RunStatus>('start_run', {request}),
    runStatus: (id: string | null = null) => invoke<import('./transport.ts').RunStatus | null>('run_status', {id}),
    stopRun: (id: string) => invoke<import('./transport.ts').RunStatus>('stop_run', {id}),
    managedSessionStatus: () => invoke<import('./transport.ts').ManagedSessionStatus>('managed_session_status'),
    runtimeReadiness: () => invoke<Readiness>('runtime_readiness'),
    runtimeSetupPlan: () => invoke<SetupPlan>('runtime_setup_plan'),
    authReadiness:()=>invoke<AuthReadiness>('auth_readiness'),
    listConfigurations:()=>invoke<SavedConfiguration[]>('list_configurations'),
    saveConfiguration:(request:SaveConfiguration)=>invoke<SavedConfiguration>('save_configuration',{request}),
    deleteConfiguration:(id:string,expectedRevision:number)=>invoke<void>('delete_configuration',{id,expectedRevision}),
    prepare: (definition:Definition,supplied:Record<string,string>)=>invoke<PreparedInputs>('prepare_inputs',{definition,supplied}),
    list: () => invoke<SavedDefinition[]>('list_definitions'),
    load: (id: string) => invoke<SavedDefinition>('load_definition', { id }),
    save: (request: SaveDefinition) => invoke<SavedDefinition>('save_definition', { request }),
  };
}
export type Commands = ReturnType<typeof createCommands>;
