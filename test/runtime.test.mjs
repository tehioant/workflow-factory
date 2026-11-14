import { test } from 'node:test';
import assert from 'node:assert/strict';
import { admittedTarget, clientHelloHostname, createEgressServer } from '../runtime/egress.mjs';
import { EventEmitter } from 'node:events';

test('egress accepts only exact AI TLS hosts and vetted public IPv4 destinations', async () => {
 const lookup = async () => [{ address: '104.18.32.7', family: 4 }];
 assert.deepEqual(await admittedTarget('chatgpt.com:443', lookup), { host: 'chatgpt.com', address: '104.18.32.7', port: 443 });
 await assert.rejects(admittedTarget('chatgpt.com.attacker.test:443', lookup));
 await assert.rejects(admittedTarget('chatgpt.com:80', lookup));
 await assert.rejects(admittedTarget('chatgpt.com:443', async () => [{ address: '127.0.0.1', family: 4 }]));
 await assert.rejects(admittedTarget('chatgpt.com:443', async () => [{ address: '169.254.169.254', family: 4 }]));
 await assert.rejects(admittedTarget('chatgpt.com:443', async () => [{ address: '192.88.99.1', family: 4 }]));
});


function hello(host) {
 const name = Buffer.from(host); const entry = Buffer.concat([Buffer.from([0, name.length >> 8, name.length & 255]), name]);
 const list = Buffer.concat([Buffer.from([entry.length >> 8, entry.length & 255]), entry]);
 const ext = Buffer.concat([Buffer.from([0, 0, list.length >> 8, list.length & 255]), list]);
 const body = Buffer.concat([Buffer.from([3,3]), Buffer.alloc(32), Buffer.from([0,0,2,0x13,1,1,0,ext.length >> 8,ext.length & 255]), ext]);
 const handshake = Buffer.concat([Buffer.from([1, 0, body.length >> 8, body.length & 255]), body]);
 return Buffer.concat([Buffer.from([22,3,1,handshake.length >> 8,handshake.length & 255]), handshake]);
}
test('egress ClientHello gate binds TLS SNI and refuses non-TLS or absent hostnames', () => {
 assert.equal(clientHelloHostname(hello('chatgpt.com')), 'chatgpt.com');
 assert.equal(clientHelloHostname(hello('other.test')), 'other.test');
 assert.equal(clientHelloHostname(hello('chatgpt.com').subarray(0, 8)), undefined);
 assert.throws(() => clientHelloHostname(Buffer.from('GET / HTTP/1.1\r\n')), /TLS/);
});


test('CONNECT handler opens no upstream socket when TLS SNI differs from the admitted host', async () => {
 let connections = 0;
 const server = createEgressServer(async () => [{ address: '104.18.32.7', family: 4 }], () => { connections++; throw new Error('Upstream must not be opened'); });
 const client = new EventEmitter(); Object.assign(client, { setTimeout() {}, write() {}, end() {}, destroy() { this.destroyed = true; } });
 server.emit('connect', { url: 'chatgpt.com:443' }, client, hello('other.test'));
 await new Promise(resolve => setImmediate(resolve));
 assert.equal(connections, 0); assert.equal(client.destroyed, true);
});
