import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createCommands } from '../src/lib/commands.ts';
test('readiness action preserves an unsupported response without enabling execution',async()=>{
 const status={executionEnabled:false,credentialSource:'notInspected',credentialAvailability:'notInspected',status:'unsupportedConcurrentAuth',guidance:'Blocked'};
 const commands=createCommands(async<T>(command:string,args?:Record<string,unknown>)=>{assert.equal(command,'auth_readiness');assert.equal(args,undefined);return status as T;});
 assert.deepEqual(await commands.authReadiness(),status);
});
test('configuration action sends only nonsecret fields and expected revision',async()=>{
 let actual:unknown; const commands=createCommands(async<T>(command:string,args?:Record<string,unknown>)=>{actual={command,args};return {config,revision:3} as T;});
 const config={id:'prepared',name:'Careful',model:'model',reasoning:'high' as const};
 const saved=await commands.saveConfiguration({config,expectedRevision:2});
 assert.deepEqual(actual,{command:'save_configuration',args:{request:{config,expectedRevision:2}}});assert.equal(saved.revision,3);
});
