import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createCommands } from '../src/lib/commands.ts';
import { insertInputReference } from '../src/lib/inputs.ts';
test('input form preparation uses only prepared values in a narrow command',async()=>{
 let actual:unknown; const commands=createCommands(async<T>(command:string,args?:Record<string,unknown>)=>{actual={command,args};return {values:{topic:'text'},prompts:{a:'text'}} as T;});
 const definition={schemaVersion:1,id:'prepared',name:'Draft',blocks:[],connections:[],inputs:[]};
 const prepared=await commands.prepare(definition,{topic:'text'});
 assert.deepEqual(actual,{command:'prepare_inputs',args:{definition,supplied:{topic:'text'}}});assert.deepEqual(prepared.prompts,{a:'text'});
});
test('picker inserts a reference at the selected prompt range, not a value',()=>{assert.equal(insertInputReference('Hello world',6,11,'topic'),'Hello {{input.topic}}');});
