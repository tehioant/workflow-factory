import type { Readiness, SetupPlan, SaveConfiguration, SavedConfiguration, AuthReadiness, Definition, PreparedInputs, SaveDefinition, SavedDefinition } from './transport.ts';
export type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
export function createCommands(invoke: Invoke) {
  return {
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
