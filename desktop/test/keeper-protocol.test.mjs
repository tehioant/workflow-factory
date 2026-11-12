import {test} from 'node:test';
import assert from 'node:assert/strict';

import {devicePrompt,confirmedStopped,stopWriters,DiagnosticProjection,publishDiagnostics,diagnosticLimits,redactDiagnostics} from '../../runtime/keeper-protocol.mjs';

test('d10_guest_projection_withholds_until_known_secret_checkpoint',async()=>{
  const files=new Map();
  const store={write:(key,bytes)=>files.set(key,Buffer.from(bytes)),read:key=>Buffer.from(files.get(key)??assert.fail('missing prepared staging'))};
  const cache=token=>Buffer.from(JSON.stringify({tokens:{access_token:token}}));
  const seed='prepared-seed-token',rotated='prepared-current-token',prior='prepared-prior-token';
  const identity={runId:'00000000-0000-0000-0000-000000000001',blockId:'a',epoch:'prepared-epoch'};
  const live=[];let stopped=false,current=null,terminated=0;
  const projection=new DiagnosticProjection(store,identity,cache(seed),value=>live.push(value),()=>terminated++);
  const item={id:'provider-local-id',type:'command_execution',command:`check.py ${seed}`,aggregated_output:'pending',status:'in_progress',exit_code:null};
  const frames=[{type:'thread.started',thread_id:'never-public'},
    {type:'item.completed',item:{type:'reasoning',text:'DROP REASONING'}},
    {type:'item.updated',item},
    {type:'item.completed',item:{...item,aggregated_output:`check.py:42 expected café, got 3 ${seed} ${rotated} ${prior}`,status:'completed',exit_code:1}},
    {type:'item.completed',item:{type:'agent_message',text:`final ${rotated}`}},
    {type:'turn.completed'}];
  const input=Buffer.from(frames.map(value=>JSON.stringify(value)).join('\n'));
  for(const byte of input)projection.push(0,Buffer.from([byte]));
  for(const byte of Buffer.from(`stderr café ${rotated}\n`))projection.push(1,Buffer.from([byte]));
  assert(live.length>0,'real typed metadata arrives before EOF');
  assert(live.every(value=>value.text==='WITHHELD'&&Number.isSafeInteger(value.seq)));
  assert(!JSON.stringify(live).includes(seed));assert(!JSON.stringify(live).includes(rotated));
  projection.finish(0);
  assert(![...files.values()].some(value=>value.includes('DROP REASONING')));
  const request={identity,cache:cache(rotated).toString('utf8'),secrets:[prior]};
  const checkpoint={stopped:async()=>stopped,cache:async()=>current};
  await assert.rejects(publishDiagnostics(store,request,checkpoint));
  stopped=true;await assert.rejects(publishDiagnostics(store,request,checkpoint));
  current=cache(rotated);
  const published=await publishDiagnostics(store,request,checkpoint);
  assert.equal(published.records.length,2,'aggregate updates replace, not duplicate');
  assert.match(published.records[0].text,/check.py:42 expected café, got 3/);
  assert.equal(published.final,'final [REDACTED]');
  for(const token of [seed,rotated,prior])assert(!JSON.stringify(published).includes(token));
  assert.equal(terminated,0);assert.equal(published.incomplete,false);
  await assert.rejects(publishDiagnostics(store,{...request,identity:{...identity,blockId:'other'}},checkpoint));
  await assert.rejects(publishDiagnostics(store,{...request,cache:'mismatch'},checkpoint));
  await assert.rejects(publishDiagnostics(store,{...request,secrets:['x'.repeat(diagnosticLimits.secrets)]},checkpoint));
  assert.equal(redactDiagnostics('ababa',['aba']),'[REDACTED]');
  const limitedFiles=new Map();const limitedLive=[];
  const limitedStore={write:(key,value)=>limitedFiles.set(key,Buffer.from(value)),read:key=>limitedFiles.get(key)};
  const limited=new DiagnosticProjection(limitedStore,identity,cache(seed),value=>limitedLive.push(value),()=>terminated++);
  limited.push(0,Buffer.alloc(diagnosticLimits.frame+1,97));
  limited.push(1,Buffer.alloc(diagnosticLimits.input,98));
  limited.finish(1);
  assert.equal(terminated,1);assert(limitedLive.some(value=>value.kind==='limit'&&value.incomplete));
  assert(limited.lines.every(value=>value.length===0));
  const unavailable=await publishDiagnostics(limitedStore,request,checkpoint);
  assert.equal(unavailable.incomplete,true);assert.equal(unavailable.final,null);
  assert([...limitedFiles.values()].reduce((sum,value)=>sum+value.length,0)<diagnosticLimits.selected);
});
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
