// Fixed guest workspace only. No raw bytes leave before the stopped/cache checkpoint.
import * as fs from 'node:fs';
import { pathToFileURL } from 'node:url';
import { diagnosticIdentity, redactDiagnostics, confirmedStopped, boundedNames } from './keeper-protocol.mjs';
const denied=()=>Error('Artifact capture unavailable');
const {O_RDONLY,O_DIRECTORY,O_NOFOLLOW,O_NONBLOCK}=fs.constants;
function checked(fs,fd,directory) {
  const s=fs.fstatSync(fd,{bigint:true});
  if(s.uid!==BigInt(process.getuid()) || (directory?!s.isDirectory():!s.isFile()||s.nlink!==1n))throw denied();
  return s;
}
function unchanged(a,b) {return ['dev','ino','size','mtimeNs','ctimeNs','mode','nlink','uid'].every(k=>a[k]===b[k]);}
export async function captureArtifacts(request,checkpoint,io=fs) {
  const fs=io;
  const identity=diagnosticIdentity(request.identity);
  if(!await checkpoint.stopped())throw denied();
  const current=await checkpoint.cache();
  if(!Buffer.isBuffer(current)||current.length>65536||typeof request.cache!=='string'||!current.equals(Buffer.from(request.cache)))throw denied();
  const cache=JSON.parse(current.toString('utf8'));
  const tokens=['access_token','refresh_token','id_token'].map(k=>cache?.tokens?.[k]).filter(v=>v!==undefined);
  if(!tokens.length||tokens.some(v=>typeof v!=='string'||!v.length)||!Array.isArray(request.secrets)||request.secrets.some(v=>typeof v!=='string'||!v.length)||Buffer.byteLength(JSON.stringify(request.secrets))>196608)throw denied();
  const secrets=[...new Set([...tokens,...request.secrets])];
  if(Buffer.byteLength(JSON.stringify(secrets))>196608)throw denied();
  const root=fs.openSync('/workspace',O_RDONLY|O_DIRECTORY|O_NOFOLLOW);
  let inspected=0,total=0;const files=[];
  function walk(parent,segments) {
    const before=checked(fs,parent,true);
    const names=boundedNames(fs,`/proc/self/fd/${parent}`,256-inspected);
    inspected+=names.length;
    for(const name of names.sort()) {
      if(!name||name==='.'||name==='..'||name.includes('/')||name.includes('\\')||/[\x00-\x1f\x7f]/.test(name)||Buffer.byteLength(name)>128||segments.length>=16)throw denied();
      const path=`/proc/self/fd/${parent}/${name}`;
      const fd=fs.openSync(path,O_RDONLY|O_NOFOLLOW|O_NONBLOCK);
      try {
        const before=fs.fstatSync(fd,{bigint:true});
        if(before.isDirectory())walk(fd,[...segments,name]);
        else {
          checked(fs,fd,false);
          if(before.size>65536n||files.length>=64)throw denied();
          const bytes=Buffer.alloc(Number(before.size));let offset=0;
          while(offset<bytes.length){const n=fs.readSync(fd,bytes,offset,bytes.length-offset,offset);if(!n)throw denied();offset+=n;}
          if(!unchanged(before,checked(fs,fd,false)))throw denied();
          let text;try{text=new TextDecoder('utf-8',{fatal:true}).decode(bytes);}catch{throw denied();}
          if(/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/.test(text))throw denied();
          const filename=redactDiagnostics([...segments,name].join('/'),secrets);
          text=redactDiagnostics(text,secrets);
          total+=Buffer.byteLength(text);
          if(total>524288||Buffer.byteLength(text)>65536)throw denied();
          files.push({name:filename,text});
        }
      }finally{fs.closeSync(fd);}
    }
    if(!unchanged(before,checked(fs,parent,true)))throw denied();
  }
  try{walk(root,[]);}finally{fs.closeSync(root);}
  if(!await checkpoint.stopped()||!current.equals(await checkpoint.cache()))throw denied();
  const result={version:1,identity,files,complete:true};
  if(Buffer.byteLength(JSON.stringify(result))>1048576)throw denied();
  return result;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href) {
  process.on('uncaughtException',()=>{process.stderr.write('Artifact capture unavailable\n');process.exit(1);});
  try {
    let bytes=Buffer.alloc(0);
    for await(const chunk of process.stdin){if(bytes.length+chunk.length>262144)throw denied();bytes=Buffer.concat([bytes,chunk]);}
    const request=JSON.parse(bytes.toString('utf8'));bytes.fill(0);
    const read=fs.promises.readFile;
    const cache=async()=>{
      const parent=fs.openSync('/run/wf-codex',O_RDONLY|O_DIRECTORY|O_NOFOLLOW);let fd;
      try{checked(fs,parent,true);fd=fs.openSync(`/proc/self/fd/${parent}/auth.json`,O_RDONLY|O_NOFOLLOW|O_NONBLOCK);const before=checked(fs,fd,false);if(before.size>65536n||(before.mode&0o077n)!==0n)throw denied();const bytes=Buffer.alloc(Number(before.size));let offset=0;while(offset<bytes.length){const n=fs.readSync(fd,bytes,offset,bytes.length-offset,offset);if(!n)throw denied();offset+=n;}if(bytes.length>65536||!unchanged(before,checked(fs,fd,false)))throw denied();return bytes;}finally{if(fd!==undefined)fs.closeSync(fd);fs.closeSync(parent);}
    };
    process.stdout.write(JSON.stringify(await captureArtifacts(request,{stopped:()=>confirmedStopped(read,'/run/wf-codex/writer.json','/run/wf-codex/writers-stopped'),cache})));
  }catch{process.stderr.write('Artifact capture unavailable\n');process.exitCode=1;}
}
