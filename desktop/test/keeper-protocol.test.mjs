import {test} from 'node:test';
import assert from 'node:assert/strict';
import {devicePrompt,confirmedStopped,stopWriters} from '../../runtime/keeper-protocol.mjs';
test('keeper protocol preserves bounded official split prompts and requires positive stopped evidence',async()=>{
  const raw='https://auth.openai.com/codex/device\n\x1b[94mABCD-1234\x1b[0m';
  for(let split=0;split<=raw.length;split++) {
    let text='';let decoded;
    for(const chunk of [raw.slice(0,split),raw.slice(split)]){text+=chunk;decoded=devicePrompt(text);}
    assert.deepEqual(decoded,{url:'https://auth.openai.com/codex/device',code:'ABCD-1234'});
  }
  assert.throws(()=>devicePrompt('a'.repeat(16385)));
  const missing=()=>{throw Object.assign(Error('absent'),{code:'ENOENT'});};
  for(const sentinel of [undefined,'unconfirmed','confirmed']) {
    const read=async path=>path==='marker'?missing():sentinel??missing();
    assert.equal(await confirmedStopped(read,'marker','sentinel'),sentinel==='confirmed');
    if(sentinel==='confirmed')await stopWriters(read,()=>assert.fail('already stopped'),()=>assert.fail('already stopped'),'marker','sentinel');
    else await assert.rejects(stopWriters(read,()=>assert.fail('no known writer'),async()=>{},'marker','sentinel'));
  }
  let stopped=false;const signals=[];
  await stopWriters(async path=>path==='marker'?(stopped?missing():'{"pid":42}'):(stopped?'confirmed':missing()),(...args)=>signals.push(args),async()=>{stopped=true;},'marker','sentinel');
  assert.deepEqual(signals,[[-42,'SIGTERM']]);
});
