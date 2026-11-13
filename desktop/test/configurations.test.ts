import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createCommands } from '../src/lib/commands.ts';
test('configuration action sends only nonsecret fields and expected revision',async()=>{
 let actual:unknown; const commands=createCommands(async<T>(command:string,args?:Record<string,unknown>)=>{actual={command,args};return {config,revision:3} as T;});
 const config={id:'prepared',name:'Careful',model:'model',reasoning:'high' as const};
 const saved=await commands.saveConfiguration({config,expectedRevision:2});
 assert.deepEqual(actual,{command:'save_configuration',args:{request:{config,expectedRevision:2}}});assert.equal(saved.revision,3);
});
