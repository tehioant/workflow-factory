// Container-local writer ownership. No host service and no credential logging.
import { spawn } from 'node:child_process';
import { readFile, writeFile, unlink, stat } from 'node:fs/promises';
const home = '/run/wf-codex';
const marker = `${home}/writer.json`;
const stopped = `${home}/writers-stopped`;
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
  const args=mode==='login'?[...common,'login','--device-auth']:[...common,'-m',process.argv[3],'-c',`model_reasoning_effort=${JSON.stringify(process.argv[4])}`,'exec','--json','--skip-git-repo-check','--sandbox','workspace-write','-'];
  const child=spawn('codex',args,{detached:true,stdio:mode==='login'?['pipe','pipe','pipe']:['pipe','inherit','inherit'],env:process.env});
  if(mode==='login'){
    let bytes=0, text='';
    const prompt=async chunk=>{
      bytes+=chunk.length;if(bytes>16384){process.kill(-child.pid,'SIGTERM');return;}
      text+=chunk.toString('utf8').replace(/\x1b\[[0-9;]*m/g,'');
      const url=text.includes('https://auth.openai.com/codex/device')?['https://auth.openai.com/codex/device']:null;
      const code=text.match(/\b[A-Z0-9]{4}-[A-Z0-9]{4}\b/);
      if(url&&code)await writeFile(`${home}/device-prompt.json`,JSON.stringify({url:url[0],code:code[0]}),{mode:0o600});
    };
    child.stdout.on('data',chunk=>{void prompt(chunk).catch(()=>process.kill(-child.pid,'SIGTERM'));});
    child.stderr.on('data',chunk=>{void prompt(chunk).catch(()=>process.kill(-child.pid,'SIGTERM'));});
  }
  await writeFile(marker,JSON.stringify({pid:child.pid}),{mode:0o600});
  process.stdin.pipe(child.stdin);child.stdin.on('error',()=>{});
  let code=1;
  try { code=await new Promise((resolve,reject)=>{child.once('exit',value=>resolve(value??1));child.once('error',reject);}); }
  finally {
    // A Codex exit is not proof that a descendant has stopped writing credentials.
    try { process.kill(-child.pid,'SIGKILL'); } catch(error) {if(error.code!=='ESRCH')throw error;}
    let gone=false;
    for(let n=0;n<100;n++){
      try{process.kill(-child.pid,0);}catch(error){if(error.code==='ESRCH'){gone=true;break;}throw error;}
      await new Promise(resolve=>setTimeout(resolve,20));
    }
    if(!gone)throw Error('writer descendants unconfirmed');
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
  const record=JSON.parse(await readFile(marker,'utf8'));
  if(!Number.isSafeInteger(record.pid)||record.pid<=1)throw Error('unknown writer');
  process.kill(-record.pid,'SIGTERM');
} else if (mode==='export') {
  await stat(stopped);
  try {await stat(marker);throw Error('writer still owned');} catch(error){if(error.code!=='ENOENT')throw error;}
  const size=(await stat(`${home}/auth.json`)).size;
  if(size>65536)throw Error('cache limit');
  const bytes=await readFile(`${home}/auth.json`);
  process.stdout.write(bytes,()=>bytes.fill(0));
} else throw Error('unsupported keeper operation');
