import {compile} from 'svelte/compiler';
import {readFile} from 'node:fs/promises';
export async function compileComponent(filename:URL, overrides=new Map<string,string>()):Promise<string>{
 const compiled=compile(await readFile(filename,'utf8'),{filename:filename.pathname,generate:'client'}).js.code;
 const replacements=new Map<string,string>();
 for(const match of compiled.matchAll(/(?:from |import )['"]([^'"]+)['"]/g)){
  const s=match[1];let resolved:string;
  if(overrides.has(s))resolved=overrides.get(s)!;
  else if(s.endsWith('.svelte'))resolved='data:text/javascript;base64,'+Buffer.from(await compileComponent(new URL(s,filename),overrides)).toString('base64');
  else resolved=s==='svelte'?new URL('../node_modules/svelte/src/index-client.js',import.meta.url).href:s.startsWith('.')?new URL(s,filename).href:import.meta.resolve(s);
  replacements.set(s,resolved);
 }
 return compiled.replace(/(from |import )(['"])([^'"]+)\2/g,(_a,p,_q,s)=>p+JSON.stringify(replacements.get(s)));
}
