import http from 'node:http';
import net from 'node:net';
import { lookup } from 'node:dns/promises';
import { pathToFileURL } from 'node:url';
const hosts = new Set(['chatgpt.com', 'api.openai.com', 'auth.openai.com']);
// Conservative deny of every IANA IPv4 special-purpose range, plus multicast.
// Source: iana.org/assignments/iana-ipv4-special-registry (2025-10-09 registry).
const special = ['0.0.0.0/8','10.0.0.0/8','100.64.0.0/10','127.0.0.0/8','169.254.0.0/16','172.16.0.0/12','192.0.0.0/24','192.0.2.0/24','192.31.196.0/24','192.52.193.0/24','192.88.99.0/24','192.168.0.0/16','192.175.48.0/24','198.18.0.0/15','198.51.100.0/24','203.0.113.0/24','224.0.0.0/4','240.0.0.0/4'];
const ipv4 = address => address.split('.').reduce((value, octet) => (value * 256 + Number(octet)) >>> 0, 0);
function publicIPv4(address) {
 if (net.isIP(address) !== 4) return false;
 return !special.some(cidr => { const [base, bits] = cidr.split('/'); const mask = (0xffffffff << (32 - Number(bits))) >>> 0; return (ipv4(address) & mask) === (ipv4(base) & mask); });
}
export function clientHelloHostname(bytes) {
 if (bytes.length < 5) return undefined;
 const length = bytes.readUInt16BE(3);
 if (bytes[0] !== 22 || bytes[1] !== 3 || length > 16384) throw new Error('TLS ClientHello required');
 if (bytes.length < length + 5) return undefined;
 const record = bytes.subarray(5, length + 5);
 if (record.length < 42 || record[0] !== 1 || record.readUIntBE(1,3) !== record.length - 4) throw new Error('TLS ClientHello required');
 let offset = 38;
 const advance = count => { if (offset + count > record.length) throw new Error('Invalid TLS ClientHello'); const start = offset; offset += count; return start; };
 const byte = () => record[advance(1)];
 const word = () => record.readUInt16BE(advance(2));
 advance(byte()); advance(word()); advance(byte());
 const extensionsLength = word(); if (offset + extensionsLength !== record.length) throw new Error('Invalid TLS extensions');
 let hostname;
 while (offset < record.length) {
  const type = word(), size = word(), start = advance(size), end = start + size;
  if (type === 0xfe0d || type === 0xffce) throw new Error('Encrypted TLS hostname is not supported');
  if (type !== 0) continue;
  if (hostname !== undefined || size < 5 || record.readUInt16BE(start) !== size - 2 || record[start + 2] !== 0 || record.readUInt16BE(start + 3) !== size - 5) throw new Error('Invalid TLS hostname');
  hostname = record.subarray(start + 5, end).toString('ascii');
  if (!/^[a-z0-9.-]+$/.test(hostname)) throw new Error('Invalid TLS hostname');
 }
 if (!hostname) throw new Error('TLS hostname required');
 return hostname;
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
   client.write('HTTP/1.1 200 Connection Established\r\n\r\n');
   let initial = Buffer.alloc(0);
   const inspectHello = bytes => {
    try {
     initial = Buffer.concat([initial, bytes]); if (initial.length > 32768) throw new Error('TLS hello too large');
     const hostname = clientHelloHostname(initial); if (hostname === undefined) return;
     if (hostname !== target.host) throw new Error('TLS hostname differs from CONNECT destination');
     client.pause(); client.removeListener('data', inspectHello);
     // Connect the exact vetted IP only after SNI agrees; never re-resolve DNS.
     const upstream = connect({ host: target.address, port: target.port });
     upstream.setTimeout(120000, () => upstream.destroy());
     upstream.on('error', () => client.destroy()); client.on('close', () => upstream.destroy()); upstream.on('close', () => client.destroy());
     upstream.on('connect', () => { upstream.write(initial); upstream.pipe(client); client.pipe(upstream); client.resume(); });
    } catch { client.destroy(); }
   };
   client.on('data', inspectHello); if (head.length) inspectHello(head);
  } catch { client.end('HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n'); }
 });
 server.headersTimeout = 10000; server.requestTimeout = 10000; server.maxConnections = 32;
 return server;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) createEgressServer().listen(3128, '0.0.0.0');
