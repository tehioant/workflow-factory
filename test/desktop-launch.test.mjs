import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
const root = new URL('../', import.meta.url);
const json = path => JSON.parse(readFileSync(new URL(path, root), 'utf8'));

test('standard desktop launch waits for bundled assets and never starts a dev server', () => {
  const scripts = json('package.json').scripts;
  const desktop = json('desktop/package.json').scripts;
  const config = json('desktop/src-tauri/tauri.conf.json');
  assert.equal(scripts.start, 'npm --prefix desktop start');
  assert.equal(scripts.build, 'npm --prefix desktop run build');
  assert.equal(scripts.check, 'npm --prefix desktop run check');
  assert.equal(scripts.typecheck, 'npm run check');
  assert.equal(scripts.test, 'node desktop/scripts/check-tests.mjs');
  assert.equal(desktop.start, 'tauri dev --no-dev-server');
  assert.deepEqual(config.build.beforeDevCommand, { script: 'npm run build', wait: true });
  assert.equal(config.build.beforeBuildCommand, 'npm run build');
  assert.equal(config.build.frontendDist, '../dist');
  assert.equal(config.build.devUrl, undefined);
  assert.ok(!config.app.security.csp.includes('127.0.0.1'));
  const host = readFileSync(new URL('desktop/src-tauri/src/main.rs', root), 'utf8');
  assert.equal((host.match(/url\.host_str\(\) == Some\("localhost"\) && url\.scheme\(\) == "tauri"/g) ?? []).length, 2);
  assert.equal((host.match(/url\.host_str\(\) == Some\("tauri\.localhost"\) && url\.scheme\(\) == "http"/g) ?? []).length, 2);
  assert.ok(!host.includes('http://127.0.0.1:1430'));
});
