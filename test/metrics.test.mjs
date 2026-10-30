import {test} from 'node:test';
import assert from 'node:assert/strict';
import {parseProcStat,cpuSample} from '../scripts/metrics.mjs';
test('Linux stat parser handles spaces/parentheses in command names and reports CPU/start/RSS units',()=>{
 const fields=Array(22).fill('0');fields[0]='S';fields[11]='25';fields[12]='5';fields[19]='12345';fields[21]='10';
 assert.deepEqual(parseProcStat('42 (a tricky ) process) '+fields.join(' '),4096),{cpuTicks:30,startedTicks:12345,rssBytes:40960});
 assert.throws(()=>parseProcStat('malformed',4096),/stat/);
});
test('sampled CPU uses actual elapsed monotonic time and one-core percentage rather than machine-wide percentage',()=>{
 assert.deepEqual(cpuSample({cpuTicks:10},{cpuTicks:40},5000,100),{cpuMs:300,percentOneCore:6});
 assert.throws(()=>cpuSample({cpuTicks:10},{cpuTicks:40},0,100),/sample/);
});
