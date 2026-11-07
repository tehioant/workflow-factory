import type { Definition } from './transport.ts';
import type { Commands } from './commands.ts';
export type Editor = { working: Definition | null; revision: number | null; dirty: boolean; error: string };
export function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error && typeof error.message === 'string') return error.message;
  return 'Desktop command failed. Your edits are retained; retry.';
}
export type GraphAction = {type:'connect';source:string;target:string} | {type:'remove';id:string} | {type:'add';id:string} | {type:'update';id:string;patch:Partial<Pick<Definition['blocks'][number],'name'|'prompt'|'position'|'configurationId'>>};
export function editGraph(state: Editor, action: GraphAction): Editor {
 if (!state.working) return state;
 const d=state.working;
 if(action.type==='connect') return {...state,working:{...d,connections:[...d.connections,{source:action.source,target:action.target}]},dirty:true,error:''};
 if(action.type==='remove') return {...state,working:{...d,blocks:d.blocks.filter(b=>b.id!==action.id),connections:d.connections.filter(c=>c.source!==action.id&&c.target!==action.id)},dirty:true,error:''};
 const blocks=action.type==='add' ? [...d.blocks,{id:action.id,name:'AI block',prompt:'',configurationId:null,position:{x:80+d.blocks.length*220,y:100}}] : d.blocks.map(b=>b.id===action.id?{...b,...action.patch}:b);
 return {...state,working:{...d,blocks},dirty:true,error:''};
}

export const emptyEditor = (): Editor => ({ working: null, revision: null, dirty: false, error: '' });
export async function openEditor(state: Editor, id: string, commands: Commands): Promise<Editor> {
  if (state.dirty) return { ...state, error: 'Save or discard your edits before opening another definition.' };
  try {
    const saved = await commands.load(id);
    return { working: saved.definition, revision: saved.revision, dirty: false, error: '' };
  } catch (error) { return { ...state, error: errorMessage(error) }; }
}
export async function saveEditor(state: Editor, commands: Commands): Promise<Editor> {
  if (!state.working) return state;
  try {
    const saved = await commands.save({ definition: structuredClone(state.working), expectedRevision: state.revision });
    return { working: saved.definition, revision: saved.revision, dirty: false, error: '' };
  } catch (error) {
    return { ...state, error: errorMessage(error) };
  }
}
