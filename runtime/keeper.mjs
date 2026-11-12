// Container-local writer ownership. No host service and no credential logging.
import { spawn } from 'node:child_process';
import { devicePrompt, confirmedStopped, stopWriters, DiagnosticProjection, privateDiagnosticStore, publishDiagnostics, diagnosticIdentity } from './keeper-protocol.mjs';
import * as fs from 'node:fs';
import { readFile, writeFile, unlink, stat } from 'node:fs/promises';
// Never reflect credential-adjacent parser errors or exception messages to transport.
process.on('uncaughtException',()=>{process.stderr.write('keeper unavailable\n');process.exit(1);});
const home = '/run/wf-codex';
const marker = `${home}/writer.json`;
const stopped = `${home}/writers-stopped`;
function privateCache() {
  const parent=fs.openSync(home,fs.constants.O_RDONLY|fs.constants.O_DIRECTORY|fs.constants.O_NOFOLLOW);
  let fd;
  try {
    fd=fs.openSync(`/proc/self/fd/${parent}/auth.json`,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW);
    const before=fs.fstatSync(fd);
    if(!before.isFile()||before.nlink!==1||before.uid!==process.getuid()||(before.mode&0o077)!==0||before.size>65536)throw Error('cache unavailable');
    const bytes=Buffer.alloc(before.size);let offset=0;
    while(offset<bytes.length){const n=fs.readSync(fd,bytes,offset,bytes.length-offset,offset);if(!n)throw Error('cache unavailable');offset+=n;}
    const after=fs.fstatSync(fd);
    if(before.size!==after.size||before.mtimeMs!==after.mtimeMs||before.ctimeMs!==after.ctimeMs)throw Error('cache unavailable');
    return bytes;
  }finally{if(fd!==undefined)fs.closeSync(fd);fs.closeSync(parent);}
}
const mode = process.argv[2];
if (!mode) {
  process.on('SIGTERM', () => process.exit(0));
  setInterval(() => {}, 3600000);
} else if (mode === 'stage') {
  let bytes=Buffer.alloc(0);
  for await (const chunk of process.stdin) { if(bytes.length+chunk.length>65536)throw Error('cache limit');bytes=Buffer.concat([bytes,chunk]); }
  await writeFile(`${home}/auth.json`, bytes, {mode:0o600,flag:'wx'});
  bytes.fill(0);
} else if (mode === 'run' || mode === 'login') {
  await unlink(stopped).catch(error=>{if(error.code!=='ENOENT')throw error;});
  // Lock must precede spawning; another exec cannot create a concurrent writer.
  await writeFile(marker, '{}', {mode:0o600,flag:'wx'});
  const common=['-c','cli_auth_credentials_store="file"','-c','forced_login_method="chatgpt"'];
  const args=mode==='login'?[...common,'login','--device-auth']:[...common,'-m',process.argv[3],'-c',`model_reasoning_effort=${JSON.stringify(process.argv[4])}`,'-c','approval_policy="never"','exec','--json','--ephemeral','--ignore-user-config','--ignore-rules','--skip-git-repo-check','--sandbox','danger-full-access','-'];
  let diagnostics,diagnosticStore;
  let child;
  if(mode==='run') {
    const identity=diagnosticIdentity({runId:process.argv[5],blockId:process.argv[6],epoch:process.argv[7]});
    diagnosticStore=privateDiagnosticStore(fs);
    diagnostics=new DiagnosticProjection(diagnosticStore,identity,privateCache(),value=>{
      if(!process.stdout.write(JSON.stringify(value)+'\n'))throw Error('diagnostic transport unavailable');
    },()=>{if(child?.pid)try{process.kill(-child.pid,'SIGTERM');}catch(error){if(error.code!=='ESRCH')throw error;}});
  }
  child=spawn('codex',args,{detached:true,stdio:['pipe','pipe','pipe'],env:process.env});
  const completion=new Promise(resolve=>{child.once('exit',value=>resolve(value??1));child.once('error',()=>resolve(1));});
  if(mode==='login'){
    let bytes=0, text='';
    const prompt=async chunk=>{
      bytes+=chunk.length;if(bytes>16384){process.kill(-child.pid,'SIGTERM');return;}
      text+=chunk.toString('utf8');
      const decoded=devicePrompt(text);
      if(decoded)await writeFile(`${home}/device-prompt.json`,JSON.stringify(decoded),{mode:0o600});
    };
    child.stdout.on('data',chunk=>{void prompt(chunk).catch(()=>process.kill(-child.pid,'SIGTERM'));});
    child.stderr.on('data',chunk=>{void prompt(chunk).catch(()=>process.kill(-child.pid,'SIGTERM'));});
  } else {
    // Synchronous bounded projection prevents promise queues retaining raw chunks.
    for(const [stream,source] of [child.stdout,child.stderr].entries()) {
      source.on('data',chunk=>{try{diagnostics.push(stream,chunk);}catch{diagnostics.fail();}});
    }
  }
  if(!Number.isSafeInteger(child.pid)||child.pid<=1) {
    if(mode==='login')await writeFile(`${home}/login-result.json`,JSON.stringify({exit:1}),{mode:0o600});
    throw Error('writer spawn failed; ownership held');
  }
  await writeFile(marker,JSON.stringify({pid:child.pid}),{mode:0o600});
  process.stdin.pipe(child.stdin);child.stdin.on('error',()=>{});
  let code=1;
  try {
    code=await completion;
    // Exit is not pipe EOF. Keep draining both pipes after terminating descendants.
  }
  finally {
    // A Codex exit is not proof that a descendant has stopped writing credentials.
    try { process.kill(-child.pid,'SIGKILL'); } catch(error) {if(error.code!=='ESRCH')throw error;}
    let gone=false;
    for(let n=0;n<100;n++){
      try{process.kill(-child.pid,0);}catch(error){if(error.code==='ESRCH'){gone=true;break;}throw error;}
      await new Promise(resolve=>setTimeout(resolve,20));
    }
    if(!gone)throw Error('writer descendants unconfirmed');
    if(mode==='run') {
      await Promise.all([child.stdout,child.stderr].map(source=>source.readableEnded?Promise.resolve():new Promise(resolve=>source.once('end',resolve))));
      try {diagnostics.finish(code);}catch { /* Publication remains explicitly unavailable. */ }
      try {diagnosticStore.close();}catch { /* Writer finalization is independent. */ }
    }
    await unlink(marker);await writeFile(stopped,'confirmed',{mode:0o600});
  }
  if(mode==='login')await writeFile(`${home}/login-result.json`,JSON.stringify({exit:code}),{mode:0o600});
  process.exitCode=code;
} else if(mode==='login-status') {
  let result=null,prompt=null;
  try{result=JSON.parse(await readFile(`${home}/login-result.json`,'utf8'));}catch(error){if(error.code!=='ENOENT')throw error;}
  if(!result)try{prompt=JSON.parse(await readFile(`${home}/device-prompt.json`,'utf8'));}catch(error){if(error.code!=='ENOENT')throw error;}
  process.stdout.write(JSON.stringify({exit:result?.exit??null,url:prompt?.url??null,code:prompt?.code??null}));
} else if (mode==='stop-writers') {
  await stopWriters(readFile,process.kill,ms=>new Promise(resolve=>setTimeout(resolve,ms)),marker,stopped);
} else if(mode==='publish-diagnostics'||mode==='discard-diagnostics') {
  let bytes=Buffer.alloc(0);
  for await(const chunk of process.stdin){if(bytes.length+chunk.length>262144)throw Error('diagnostic request limit');bytes=Buffer.concat([bytes,chunk]);}
  const request=JSON.parse(bytes.toString('utf8'));bytes.fill(0);
  const store=privateDiagnosticStore(fs);
  try {
    if(!await confirmedStopped(readFile,marker,stopped))throw Error('writers not positively stopped');
    if(mode==='publish-diagnostics') {
      const publication=await publishDiagnostics(store,request,{stopped:()=>confirmedStopped(readFile,marker,stopped),cache:async()=>privateCache()});
      process.stdout.write(JSON.stringify(publication));
    } else {
      // Host sends this only after durable sanitized commit; no provider text returned.
      await publishDiagnostics(store,request,{stopped:()=>confirmedStopped(readFile,marker,stopped),cache:async()=>privateCache()});
      store.discard(request.identity);
    }
  }finally{store.close();}
} else if (mode==='export') {
  if(!await confirmedStopped(readFile,marker,stopped))throw Error('writers not positively stopped');
  const size=(await stat(`${home}/auth.json`)).size;
  if(size>65536)throw Error('cache limit');
  const bytes=await readFile(`${home}/auth.json`);
  process.stdout.write(bytes,()=>bytes.fill(0));
} else throw Error('unsupported keeper operation');
