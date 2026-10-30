export function parseProcStat(text,pageSize) {
 const end=text.lastIndexOf(') '); const fields=text.slice(end+2).trim().split(/\s+/);
 const user=Number(fields[11]),system=Number(fields[12]),started=Number(fields[19]),rss=Number(fields[21]);
 if(end<0 || ![user,system,started,rss,pageSize].every(Number.isFinite) || Math.min(user,system,started,rss)<0 || pageSize<=0) throw new Error('Invalid Linux process stat');
 return {cpuTicks:user+system,startedTicks:started,rssBytes:rss*pageSize};
}
export function cpuSample(before,after,elapsedMs,ticksPerSecond) {
 if(elapsedMs<=0 || ticksPerSecond<=0 || after.cpuTicks<before.cpuTicks) throw new Error('Invalid CPU sample');
 const cpuMs=(after.cpuTicks-before.cpuTicks)*1000/ticksPerSecond;
 return {cpuMs,percentOneCore:cpuMs/elapsedMs*100};
}
