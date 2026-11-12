// Pure bounded protocol decoding and positive writer-finalization evidence.
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
