import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createCommands } from '../src/lib/commands.ts';
test('manual runtime actions invoke only parameterless narrow host commands', async () => {
  const calls: unknown[] = [];
  const readiness = {status:'missing',message:'Install explicitly',imageId:null,isolation:'notChecked',executionReady:false};
  const plan = {program:'/usr/bin/docker',argv:['--host','unix:///var/run/docker.sock','build'],instructions:'Review first'};
  const commands = createCommands(async <T>(command: string, args?: Record<string, unknown>) => {
    calls.push({command,args});
    return (command === 'runtime_readiness' ? readiness : plan) as T;
  });
  assert.deepEqual(calls, []);
  assert.deepEqual(await commands.runtimeReadiness(), readiness);
  assert.deepEqual(await commands.runtimeSetupPlan(), plan);
  assert.deepEqual(calls, [{command:'runtime_readiness',args:undefined},{command:'runtime_setup_plan',args:undefined}]);
});
