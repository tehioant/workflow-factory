// Bounded protocol decoding and positive writer-finalization evidence.


export const diagnosticLimits = Object.freeze({ frame: 65536, input: 1048576, events: 4096, selected: 524288, record: 16384, records: 64, final: 65536, secrets: 196608 });
const denied = () => new Error('diagnostics unavailable');
export function boundedNames(fs,path,limit) {
  const directory=fs.opendirSync(path,{bufferSize:1});const names=[];
  try {let entry;while((entry=directory.readSync())!==null){if(names.length>=limit)throw denied();names.push(entry.name);}}
  finally {directory.closeSync();}
  return names;
}

// Pin each directory and open only fixed names relative to its descriptor.
// This adapter is Linux-guest-only; syscall/race qualification remains separate.
export function privateDiagnosticStore(fs, home='/run/wf-codex') {
  const {O_RDONLY,O_WRONLY,O_DIRECTORY,O_NOFOLLOW,O_CREAT,O_EXCL}=fs.constants;
  const uid=process.getuid();
  const check=(stat,directory)=>{
    if(stat.uid!==uid || (stat.mode&0o077)!==0 || !(directory?stat.isDirectory():stat.isFile()&&stat.nlink===1))throw denied();
  };
  const parent=fs.openSync(home,O_RDONLY|O_DIRECTORY|O_NOFOLLOW);
  let fd;
  try {
    check(fs.fstatSync(parent),true);
    const path=`/proc/self/fd/${parent}/diagnostics`;
    try {fs.mkdirSync(path,{mode:0o700});}catch(error){if(error.code!=='EEXIST')throw denied();}
    fd=fs.openSync(path,O_RDONLY|O_DIRECTORY|O_NOFOLLOW);check(fs.fstatSync(fd),true);
  } finally {fs.closeSync(parent);}
  const path=key=>{
    if(!/^(seed|identity|manifest|final|record-(0|[1-9][0-9]?))$/.test(key))throw denied();
    return `/proc/self/fd/${fd}/${key}`;
  };
  const read=key=>{
    const file=fs.openSync(path(key),O_RDONLY|O_NOFOLLOW);
    try {
      const before=fs.fstatSync(file);check(before,false);
      if(before.size>65536+2048)throw denied();
      const bytes=Buffer.alloc(before.size);let offset=0;
      while(offset<bytes.length){const n=fs.readSync(file,bytes,offset,bytes.length-offset,offset);if(!n)throw denied();offset+=n;}
      const after=fs.fstatSync(file);
      if(before.ino!==after.ino||before.dev!==after.dev||before.size!==after.size||before.mtimeMs!==after.mtimeMs||before.ctimeMs!==after.ctimeMs)throw denied();
      return bytes;
    }finally{fs.closeSync(file);}
  };
  return {
    read,
    empty(){if(boundedNames(fs,`/proc/self/fd/${fd}`,0).length)throw denied();},
    write(key,bytes){
      const target=path(key),pending=target+'.pending';let file;
      if(!Buffer.isBuffer(bytes)||bytes.length>65536+2048)throw denied();
      try {
        file=fs.openSync(pending,O_WRONLY|O_NOFOLLOW|O_CREAT|O_EXCL,0o600);check(fs.fstatSync(file),false);
        let offset=0;
        while(offset<bytes.length){const n=fs.writeSync(file,bytes,offset,bytes.length-offset);if(!n)throw denied();offset+=n;}
        fs.fsyncSync(file);fs.closeSync(file);file=undefined;fs.renameSync(pending,target);fs.fsyncSync(fd);
      }finally{if(file!==undefined)fs.closeSync(file);}
    },
    discard(identity){
      if(JSON.stringify(JSON.parse(read('identity').toString('utf8')))!==JSON.stringify(diagnosticIdentity(identity)))throw denied();
      const names=boundedNames(fs,`/proc/self/fd/${fd}`,68);
      for(const name of names) {path(name);read(name);}
      for(const name of names)fs.unlinkSync(path(name));fs.fsyncSync(fd);
    },
    close(){fs.closeSync(fd);}
  };
}
const size = value => Buffer.byteLength(value);
export function diagnosticIdentity(value) {
  if (!value || Object.keys(value).sort().join(',') !== 'blockId,epoch,runId' ||
      !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value.runId) || !/^[A-Za-z0-9_-]{1,128}$/.test(value.blockId) ||
      typeof value.epoch !== 'string' || !/^[A-Za-z0-9_.:+-]{1,128}$/.test(value.epoch)) throw denied();
  return {runId:value.runId, blockId:value.blockId, epoch:value.epoch};
}
function cacheSecrets(bytes) {
  if (!Buffer.isBuffer(bytes) || bytes.length > 65536) throw denied();
  let cache;
  try { cache = JSON.parse(bytes.toString('utf8')); } catch { throw denied(); }
  const tokens = ['access_token','refresh_token','id_token'].map(key => cache?.tokens?.[key]).filter(value => value !== undefined);
  if (!tokens.length || tokens.some(value => typeof value !== 'string' || !value.length || size(value)>65536)) throw denied();
  return tokens;
}
// Match the union on the original text: replacement cannot expose an overlapping token suffix.
export function redactDiagnostics(text, secrets) {
  const ranges=[];
  for (const token of secrets) {
    let from=0, at;
    if(!token.length)throw denied();
    while ((at=text.indexOf(token,from))!==-1) {
      if(ranges.length>=65536)throw denied();
      ranges.push([at,at+token.length]); from=at+1;
    }
  }
  ranges.sort((a,b)=>a[0]-b[0] || b[1]-a[1]);
  const merged=[];
  for (const range of ranges) {
    const last=merged.at(-1);
    if(last && range[0]<=last[1])last[1]=Math.max(last[1],range[1]);else merged.push(range);
  }
  let result='',from=0;
  for(const [start,end] of merged){result+=text.slice(from,start)+'[REDACTED]';from=end;}
  result+=text.slice(from);
  if(size(result)>diagnosticLimits.input)throw denied();
  return result;
}

// The store is private credential-tmpfs staging, not a host log or a transcript sink.
export class DiagnosticProjection {
  constructor(store, identity, seed, emit, terminate) {
    this.store=store; this.identity=diagnosticIdentity(identity); cacheSecrets(seed);
    this.emit=emit;this.terminate=terminate;this.seq=0;this.bytes=0;this.frames=0;this.selected=0;
    this.items=new Map();this.lengths=new Map();this.lines=[Buffer.alloc(0),Buffer.alloc(0)];this.discard=[false,false];
    this.incomplete=false;this.completed=false;this.failed=false;this.final=false;this.ended=false;
    store.empty?.();
    store.write('seed',seed);store.write('identity',Buffer.from(JSON.stringify(this.identity)));
  }
  metadata(kind) {
    this.emit({version:1,type:'wf.diagnostic',seq:++this.seq,kind,bytes:this.bytes,text:'WITHHELD',incomplete:this.incomplete});
  }
  fail() {
    if(this.incomplete)return;
    this.incomplete=true;this.lines=[Buffer.alloc(0),Buffer.alloc(0)];
    try{this.metadata('limit');}finally{this.terminate();}
  }
  save(key, value) {
    const bytes=Buffer.from(JSON.stringify(value));
    const previous=this.lengths.get(key)??0;
    const cap=key==='final'?diagnosticLimits.final+128:diagnosticLimits.record;
    if(bytes.length>cap || this.selected-previous+bytes.length>diagnosticLimits.selected) {this.fail();return;}
    this.store.write(key,bytes);this.selected+=bytes.length-previous;this.lengths.set(key,bytes.length);
  }
  accept(line, stream) {
    if(this.incomplete || !line.length)return;
    if(++this.frames>diagnosticLimits.events){this.fail();return;}
    try {
      if(stream===1) {
        const key=`record-${this.items.size}`;
        if(this.items.size>=diagnosticLimits.records){this.fail();return;}
        this.items.set(key,key);this.save(key,{kind:'stderr',text:line.toString('utf8')});this.metadata('stderr');return;
      }
      if(this.completed) {this.fail();return;}
      const event=JSON.parse(line.toString('utf8'));
      if(['item.started','item.updated','item.completed'].includes(event.type)) {
        const item=event.item;
        if(item?.type==='command_execution') {
          if(typeof item.id!=='string'||size(item.id)>128||typeof item.command!=='string'||typeof item.aggregated_output!=='string'||!['in_progress','completed','failed'].includes(item.status)||!(item.exit_code===null||Number.isSafeInteger(item.exit_code))) {this.fail();return;}
          let key=this.items.get('command:'+item.id);
          if(!key) {
            if(this.items.size>=diagnosticLimits.records){this.fail();return;}
            key=`record-${this.items.size}`;this.items.set('command:'+item.id,key);
          }
          this.save(key,{kind:'command',command:item.command,text:item.aggregated_output,status:item.status,exitCode:item.exit_code});
          this.metadata('command');
        } else if(event.type==='item.completed' && item?.type==='agent_message') {
          if(typeof item.text!=='string'||size(item.text)>diagnosticLimits.final){this.fail();return;}
          this.save('final',{text:item.text});this.final=true;
        }
      } else if(event.type==='turn.completed')this.completed=true;
      else if(event.type==='error'||event.type==='turn.failed') {
        this.failed=true;
        const message=event.message??event.error?.message;
        if(typeof message==='string') {
          if(this.items.size>=diagnosticLimits.records){this.fail();return;}
          const key=`record-${this.items.size}`;this.items.set(key,key);this.save(key,{kind:'error',text:message});
        }
        this.metadata('error');
      } else if(!['thread.started','turn.started'].includes(event.type)) {this.fail();}
    } catch {this.fail();}
  }
  push(stream, chunk) {
    if(this.ended)throw denied();
    if(stream!==0&&stream!==1)throw denied();
    if(!Buffer.isBuffer(chunk))throw denied();
    this.bytes=Math.min(diagnosticLimits.input+1,this.bytes+chunk.length);
    if(this.bytes>diagnosticLimits.input){this.fail();return;}
    if(this.incomplete)return;
    const cap=stream===0?diagnosticLimits.frame:8192;
    let from=0;
    for(let i=0;i<=chunk.length;i++) {
      if(i!==chunk.length && chunk[i]!==10)continue;
      const part=chunk.subarray(from,i);from=i+1;
      if(!this.discard[stream]) {
        if(this.lines[stream].length+part.length>cap){this.lines[stream]=Buffer.alloc(0);this.discard[stream]=true;this.fail();return;}
        this.lines[stream]=Buffer.concat([this.lines[stream],part]);
      }
      if(i<chunk.length) {
        if(!this.discard[stream])this.accept(this.lines[stream],stream);
        this.lines[stream]=Buffer.alloc(0);this.discard[stream]=false;
        if(this.incomplete)return;
      }
    }
  }
  finish(exit) {
    if(this.ended)throw denied();
    for(let stream=0;stream<2;stream++)this.accept(this.lines[stream],stream);
    this.lines=[Buffer.alloc(0),Buffer.alloc(0)];this.ended=true;
    this.store.write('manifest',Buffer.from(JSON.stringify({version:1,identity:this.identity,keys:[...this.lengths.keys()],lengths:Object.fromEntries(this.lengths),incomplete:this.incomplete,completed:this.completed&&!this.failed&&this.final&&exit===0,exit})));
    this.metadata('stopped');
  }
}

export async function publishDiagnostics(store, request, checkpoint) {
  const identity=diagnosticIdentity(request.identity);
  if(!await checkpoint.stopped())throw denied();
  const current=await checkpoint.cache();cacheSecrets(current);
  if(typeof request.cache!=='string'||size(request.cache)>65536||!Buffer.from(request.cache).equals(current))throw denied();
  if(!Array.isArray(request.secrets)||request.secrets.some(s=>typeof s!=='string'||!s.length)||size(JSON.stringify(request.secrets))>diagnosticLimits.secrets)throw denied();
  const secrets=[...new Set([...request.secrets,...cacheSecrets(store.read('seed')),...cacheSecrets(current)])];
  if(size(JSON.stringify(secrets))>diagnosticLimits.secrets)throw denied();
  let manifest;
  try {manifest=JSON.parse(store.read('manifest').toString('utf8'));}catch {throw denied();}
  if(manifest.version!==1||JSON.stringify(manifest.identity)!==JSON.stringify(identity)||!Array.isArray(manifest.keys)||manifest.keys.length>65||new Set(manifest.keys).size!==manifest.keys.length||typeof manifest.incomplete!=='boolean'||typeof manifest.completed!=='boolean'||!Number.isSafeInteger(manifest.exit))throw denied();
  const records=[];let final=null,total=0;
  for(const key of manifest.keys) {
    if(!/^(final|record-(0|[1-9][0-9]?))$/.test(key))throw denied();
    const bytes=store.read(key);total+=bytes.length;
    if(total>diagnosticLimits.selected||bytes.length!==manifest.lengths[key]||bytes.length>(key==='final'?diagnosticLimits.final+128:diagnosticLimits.record))throw denied();
    let value;
    try {value=JSON.parse(bytes.toString('utf8'));}catch {throw denied();}
    if(typeof value.text!=='string')throw denied();
    if(key==='final')final=redactDiagnostics(value.text,secrets);
    else {
      if(!['command','stderr','error'].includes(value.kind))throw denied();
      const record={kind:value.kind,text:redactDiagnostics(value.text,secrets)};
      if(value.kind==='command') {
        if(typeof value.command!=='string'||!['in_progress','completed','failed'].includes(value.status)||!(value.exitCode===null||Number.isSafeInteger(value.exitCode)))throw denied();
        Object.assign(record,{command:redactDiagnostics(value.command,secrets),status:value.status,exitCode:value.exitCode});
      }
      records.push(record);
    }
  }
  const publication={version:1,identity,records,final:manifest.completed&&!manifest.incomplete?final:null,incomplete:manifest.incomplete,completed:manifest.completed&&!manifest.incomplete,exit:manifest.exit};
  if(size(JSON.stringify(publication))>diagnosticLimits.input)throw denied();
  return publication;
}
export function devicePrompt(raw) {
  if (Buffer.byteLength(raw)>16384) throw Error('prompt limit');
  const text=raw.replace(/\x1b\[[0-9;]*m/g,'');
  const code=text.match(/\b[A-Z0-9]{4}-[A-Z0-9]{4}\b/);
  return text.includes('https://auth.openai.com/codex/device')&&code?{url:'https://auth.openai.com/codex/device',code:code[0]}:null;
}
export async function confirmedStopped(read, marker, sentinel) {
  try {await read(marker); return false;} catch(error) {if(error.code!=='ENOENT')throw error;}
  try {return (await read(sentinel,'utf8'))==='confirmed';} catch(error) {if(error.code!=='ENOENT')throw error;return false;}
}

export async function stopWriters(read, signal, sleep, marker, sentinel) {
  for(let n=0;n<150;n++) {
    if(await confirmedStopped(read,marker,sentinel))return;
    let record;
    try{record=JSON.parse(await read(marker,'utf8'));}catch(error){if(error.code!=='ENOENT')throw error;}
    if(record) {
      if(!Number.isSafeInteger(record.pid)||record.pid<=1)throw Error('unknown writer');
      if(n===0||n===100)try{signal(-record.pid,n===0?'SIGTERM':'SIGKILL');}catch(error){if(error.code!=='ESRCH')throw error;}
    }
    await sleep(20);
  }
  if(!await confirmedStopped(read,marker,sentinel))throw Error('writer finalization unconfirmed');
}
