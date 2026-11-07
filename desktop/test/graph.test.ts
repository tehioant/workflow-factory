import { test } from 'node:test';
import assert from 'node:assert/strict';
import { toFlow, fromFlow } from '../src/lib/graph.ts';
import { editGraph } from '../src/lib/editor.ts';
const definition = {schemaVersion:1,id:'workflow',name:'Graph',blocks:[{id:'a',name:'First',prompt:'Private prompt',position:{x:10,y:20}}],connections:[]};
const state={working:definition,revision:2,dirty:false,error:''};
test('adding an AI block dirties only the working definition',()=>{const next=editGraph(state,{type:'add',id:'b'}); assert.equal(next.dirty,true); assert.equal(next.working!.blocks[1].name,'AI block'); assert.equal(state.working.blocks.length,1);});
test('removing a block also removes its incident connections',()=>{const prepared={...state,working:{...state.working,connections:[{source:'a',target:'b'}]}}; const next=editGraph(prepared,{type:'remove',id:'a'}); assert.deepEqual(next.working!.blocks,[]); assert.deepEqual(next.working!.connections,[]);});
test('keyboard connection action stores a directional dependency',()=>{const next=editGraph(state,{type:'connect',source:'a',target:'b'}); assert.deepEqual(next.working!.connections,[{source:'a',target:'b'}]); assert.equal(next.dirty,true);});
test('graph mapping keeps library state out of a definition', () => {
 const flow=toFlow(definition); flow.nodes[0].position={x:40,y:60}; flow.nodes[0].selected=true;
 assert.deepEqual(fromFlow(definition, flow.nodes, flow.edges).blocks,[{id:'a',name:'First',prompt:'Private prompt',position:{x:40,y:60}}]);
 assert.equal('selected' in fromFlow(definition,flow.nodes,flow.edges).blocks[0],false);
});
