import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { randomBytes } from 'node:crypto';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { DefinitionStore } from './store.ts';
import { handleRequest } from './http.ts';

const port = Number(process.env.PORT ?? 4317);
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('PORT must be between 1 and 65535');
const host = `127.0.0.1:${port}`;
const authority = { host, token: randomBytes(32).toString('hex') };
const base = process.platform === 'darwin' ? join(homedir(), 'Library', 'Application Support') : (process.env.XDG_DATA_HOME || join(homedir(), '.local', 'share'));
const directory = process.env.WORKFLOW_FACTORY_DATA_DIR || join(base, 'workflow-factory');
const store = new DefinitionStore(join(directory, 'definitions'));
const assets: Record<string, [string, string]> = {
 '/': ['public/index.html', 'text/html; charset=utf-8'],
 '/style.css': ['public/style.css', 'text/css; charset=utf-8'],
 '/app.js': ['public/app.js', 'text/javascript; charset=utf-8'],
 '/editor.js': ['public/editor.js', 'text/javascript; charset=utf-8'],
 '/definition.js': ['definition.js', 'text/javascript; charset=utf-8'],
};
const server = createServer(async (incoming, outgoing) => {
 outgoing.setHeader('Cache-Control', 'no-store');
 outgoing.setHeader('X-Content-Type-Options', 'nosniff');
 outgoing.setHeader('Referrer-Policy', 'no-referrer');
 outgoing.setHeader('Cross-Origin-Resource-Policy', 'same-origin');
 outgoing.setHeader('Content-Security-Policy', "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'");
 if (incoming.headers.host !== host || (incoming.headers.origin && incoming.headers.origin !== `http://${host}`) || (incoming.headers['sec-fetch-site'] && !['same-origin', 'none'].includes(String(incoming.headers['sec-fetch-site'])))) {
  outgoing.writeHead(403); outgoing.end('Local access only'); return;
 }
 try {
  const url = new URL(incoming.url || '/', `http://${host}`);
  if (url.pathname.startsWith('/api/')) {
   if (incoming.headers['x-workflow-token'] !== authority.token) { outgoing.writeHead(403); outgoing.end('Session required'); return; }
   const chunks: Buffer[] = []; let size = 0;
   for await (const chunk of incoming) {
    const bytes = Buffer.from(chunk); size += bytes.length;
    if (size > 1_048_576) { outgoing.writeHead(413); outgoing.end('Definition too large'); return; }
    chunks.push(bytes);
   }
   const headers = new Headers();
   for (const [key, value] of Object.entries(incoming.headers)) if (value !== undefined) headers.set(key, Array.isArray(value) ? value.join(', ') : value);
   const method = incoming.method || 'GET';
   const body = ['GET', 'HEAD'].includes(method) ? undefined : Buffer.concat(chunks).toString('utf8');
   const result = await handleRequest(new Request(url, { method, headers, body }), authority, store);
   outgoing.writeHead(result.status, Object.fromEntries(result.headers)); outgoing.end(await result.text()); return;
  }
  const asset = assets[url.pathname];
  if (!asset || incoming.method !== 'GET') { outgoing.writeHead(404); outgoing.end('Not found'); return; }
  let content = await readFile(new URL(asset[0], import.meta.url), 'utf8');
  if (url.pathname === '/') content = content.replace('__SESSION__', authority.token);
  outgoing.writeHead(200, { 'Content-Type': asset[1] }); outgoing.end(content);
 } catch { outgoing.writeHead(500); outgoing.end('Local server error. Check build and data folder.'); }
});
server.requestTimeout = 15_000; server.headersTimeout = 10_000;
server.on('error', error => { console.error(`Could not start local server: ${error.message}`); process.exitCode = 1; });
server.listen(port, '127.0.0.1', () => console.log(`Workflow Factory: http://${host}
Definitions: ${join(directory, 'definitions')}
Press Ctrl+C to stop. Save changes before stopping.`));
for (const signal of ['SIGINT', 'SIGTERM'] as const) process.on(signal, () => server.close());
