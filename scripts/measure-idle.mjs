// Manual Linux idle-only observation. Never imported/run by npm test or CI.
import {readFile,readdir,mkdtemp,chmod,rm,stat} from 'node:fs/promises';
import {spawn,execFileSync} from 'node:child_process';
import {once} from 'node:events';
import {join} from 'node:path';
import {tmpdir,cpus,totalmem,availableParallelism,loadavg,release,arch} from 'node:os';
import {setTimeout as delay} from 'node:timers/promises';
import {parseProcStat,cpuSample} from './metrics.mjs';
if(process.platform!=='linux') throw new Error('This /proc observer is Linux-only. See RESOURCE_REPORT.md for the unmeasured macOS procedure.');
await stat('dist/server.js');
const seconds=Number(process.env.MEASURE_SECONDS??5);
if(!Number.isInteger(seconds)||seconds<2||seconds>60) throw new Error('MEASURE_SECONDS must be an integer from 2 to 60');
const ticks=Number(execFileSync('getconf',['CLK_TCK'],{encoding:'utf8'}).trim());
const pageSize=Number(execFileSync('getconf',['PAGESIZE'],{encoding:'utf8'}).trim());
const sample=async pid=>({...parseProcStat(await readFile(`/proc/${pid}/stat`,'utf8'),pageSize),sampledMs:performance.now()});
async function runtimeProcesses() {
 const found=[];
 for(const pid of (await readdir('/proc')).filter(id=>/^\d+$/.test(id))) {
  try { const name=(await readFile(`/proc/${pid}/comm`,'utf8')).trim(); if(/^(dockerd|containerd|containerd-shim.*|docker-proxy)$/.test(name)) found.push({pid:Number(pid),name,...await sample(pid)}); } catch { /* Process exited or is not readable by this user. */ }
 }
 return found;
}
const socket=process.env.WORKFLOW_FACTORY_DOCKER_SOCKET||'/var/run/docker.sock';
let docker;
try {
 const run=args=>execFileSync('docker',['--host',`unix://${socket}`,...args],{encoding:'utf8',timeout:20000,stdio:['ignore','pipe','pipe']});
 const version=JSON.parse(run(['version','--format','{{json .}}']));
 const ids=run(['ps','-q']).trim().split(/\s+/).filter(Boolean);
 const names=run(['ps','--format','{{.Names}}']).trim().split(/\s+/).filter(Boolean);
 docker={available:true,client:version.Client.Version,engine:version.Server.Version,runningContainers:ids.length,workflowFactoryContainers:names.filter(name=>name.startsWith('wf-')).length};
} catch { docker={available:false,message:'Local Docker metadata unavailable; runtime baseline cannot be claimed clean or complete.'}; }
const directory=await mkdtemp(join(tmpdir(),'workflow-factory-idle-')); await chmod(directory,0o700);
const started=performance.now();
const child=spawn(process.execPath,['dist/server.js'],{shell:false,env:{...process.env,WORKFLOW_FACTORY_DATA_DIR:directory,PORT:process.env.PORT||'4317'},stdio:['ignore','pipe','pipe']});
const exit=once(child,'exit'); let readyResolve,readyReject; const ready=new Promise((resolve,reject)=>{readyResolve=resolve;readyReject=reject;});
let output='';child.stdout.setEncoding('utf8');child.stdout.on('data',text=>{output+=text;if(output.includes('Workflow Factory: http://'))readyResolve();});
child.stderr.resume(); child.on('error',readyReject); child.on('exit',()=>readyReject(new Error('Idle app exited before readiness; check build and PORT availability.')));
let result;
try {
 let timer; try { await Promise.race([ready,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Idle app did not become ready')),10000);})]); } finally {clearTimeout(timer);}
 const startupWallMs=performance.now()-started; const atReady=await sample(child.pid);
 await delay(1000);
 const appBefore=await sample(child.pid),runtimeBefore=await runtimeProcesses();
 await delay(seconds*1000);
 const appAfter=await sample(child.pid);const elapsedMs=appAfter.sampledMs-appBefore.sampledMs;const runtimeAfter=await runtimeProcesses();
 const runtimeSamples=runtimeAfter.map(after=>{const before=runtimeBefore.find(p=>p.pid===after.pid&&p.startedTicks===after.startedTicks);return {...after,...(before?cpuSample(before,after,after.sampledMs-before.sampledMs,ticks):{cpuMs:null,percentOneCore:null})};});
 const sum=(records,key)=>records.reduce((total,p)=>total+(p[key]??0),0);
 const status=await readFile(`/proc/${child.pid}/status`,'utf8');const highWater=Number(/^VmHWM:\s+(\d+)/m.exec(status)?.[1])*1024;
 const meminfo=await readFile('/proc/meminfo','utf8');
 result={observedAt:new Date().toISOString(),scenario:'Fresh empty-data idle app; no browser, API authentication check, workflow, image build or new container. Existing Docker left untouched.',platform:{os:'linux',kernel:release(),arch:arch(),cpuModel:cpus()[0]?.model,logicalCpus:cpus().length,availableParallelism:availableParallelism(),totalMemoryBytes:totalmem(),availableMemoryBytes:Number(/^MemAvailable:\s+(\d+)/m.exec(meminfo)?.[1])*1024,loadAverage:loadavg(),node:process.version},sample:{elapsedMs,ticksPerSecond:ticks,pageSize},app:{startupWallMs,startupCpuMsAtReady:atReady.cpuTicks*1000/ticks,rssBytesAtReady:atReady.rssBytes,rssBytes:appAfter.rssBytes,highWaterRssBytes:highWater,...cpuSample(appBefore,appAfter,elapsedMs,ticks)},docker,runtime:{processes:runtimeSamples,rssBytes:sum(runtimeSamples,'rssBytes'),cpuMs:sum(runtimeSamples,'cpuMs'),percentOneCore:sum(runtimeSamples,'percentOneCore'),note:'Readable dockerd/containerd/shims/proxies only. Shared RSS is not de-duplicated; unrelated container workloads may confound daemon/control-plane cost. Container and browser memory are excluded.'}};
} finally {
 if(child.exitCode===null&&child.signalCode===null) child.kill('SIGTERM');
 let timer;try { await Promise.race([exit,new Promise((_,reject)=>{timer=setTimeout(()=>{child.kill('SIGKILL');reject(new Error('Idle child did not exit gracefully'));},10000);})]); } finally {clearTimeout(timer);await rm(directory,{recursive:true,force:true});}
}
result.app.exitCode=child.exitCode;result.app.exitSignal=child.signalCode;result.appAndRuntimeRssBytes=result.app.rssBytes+result.runtime.rssBytes;
console.log(JSON.stringify(result,null,2));
