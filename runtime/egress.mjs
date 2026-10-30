import http from 'node:http';
import net from 'node:net';
import { lookup } from 'node:dns/promises';
import { pathToFileURL } from 'node:url';
const hosts = new Set(['chatgpt.com', 'api.openai.com', 'auth.openai.com']);
function publicIPv4(address) {
 if (net.isIP(address) !== 4) return false;
 const [a,b] = address.split('.').map(Number);
 return !(a === 0 || a === 10 || a === 127 || a >= 224 || (a === 100 && b >= 64 && b <= 127) || (a === 169 && b === 254) || (a === 172 && b >= 16 && b <= 31) || (a === 192 && (b === 0 || b === 168)) || (a === 198 && (b === 18 || b === 19 || b === 51)) || (a === 203 && b === 0));
}
export async function admittedTarget(authority, resolve = lookup) {
 const match = /^([a-z0-9.-]+):443$/.exec(authority ?? '');
 if (!match || !hosts.has(match[1])) throw new Error('Destination denied');
 const results = await resolve(match[1], { family: 4, all: true });
 if (!results.length || results.some(result => !publicIPv4(result.address))) throw new Error('Destination denied');
 return { host: match[1], address: results[0].address, port: 443 };
}
export function createEgressServer(resolve = lookup, connect = net.connect) {
 const server = http.createServer({ maxHeaderSize: 8192 }, (_, response) => { response.writeHead(403); response.end(); });
 server.on('connect', async (request, client, head) => {
  client.on('error', () => {}); client.setTimeout(120000, () => client.destroy());
  try {
   const target = await admittedTarget(request.url, resolve);
   if (client.destroyed) return;
   // Connect the exact vetted IP. Never let a second DNS lookup rebind it.
   const upstream = connect({ host: target.address, port: target.port });
   upstream.setTimeout(120000, () => upstream.destroy());
   upstream.on('error', () => client.destroy()); client.on('close', () => upstream.destroy()); upstream.on('close', () => client.destroy());
   upstream.on('connect', () => { client.write('HTTP/1.1 200 Connection Established\r\n\r\n'); if (head.length) upstream.write(head); upstream.pipe(client); client.pipe(upstream); });
  } catch { client.end('HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n'); }
 });
 server.headersTimeout = 10000; server.requestTimeout = 10000; server.maxConnections = 32;
 return server;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) createEgressServer().listen(3128, '0.0.0.0');
