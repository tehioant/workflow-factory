export function insertInputReference(prompt:string,start:number,end:number,name:string):string {
 return prompt.slice(0,start)+`{{input.${name}}}`+prompt.slice(end);
}
