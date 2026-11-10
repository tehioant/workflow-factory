import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { nodeCases, rustCases } from './test-inventory.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const desktop = join(root, 'desktop');
if (!existsSync(join(desktop, 'dist/index.html'))) throw new Error('Build bundled frontend assets first: npm --prefix desktop run build');
const checkOnly = process.argv.includes('--inventory-only');
const outputArg = process.argv.indexOf('--output');
const output = outputArg < 0 ? null : resolve(process.argv[outputArg + 1]);
function command(program, args, cwd = root, capture = false) {
  const result = spawnSync(program, args, { cwd, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, stdio: capture ? 'pipe' : 'inherit' });
  if (result.error || result.status !== 0) {
    if (capture) { process.stdout.write(result.stdout ?? ''); process.stderr.write(result.stderr ?? ''); }
    throw result.error ?? new Error(`${program} ${args.join(' ')} exited ${result.status}`);
  }
  return result.stdout ?? '';
}
function files(directory, pattern) {
  return readdirSync(directory, { recursive: true }).filter(path => pattern.test(path)).sort().map(path => join(directory, path));
}
const browserFiles = files(join(root, 'test'), /\.test\.mjs$/);
const frontendFiles = files(join(desktop, 'test'), /\.test\.(?:ts|mjs)$/);
function inventory(paths) {
  return paths.flatMap(path => nodeCases(readFileSync(path, 'utf8'), relative(root, path)));
}
const browser = inventory(browserFiles);
const frontend = inventory(frontendFiles);
// Compile and ask each actual harness for its inventory; no tests/fixtures run here.
const messages = command('cargo', ['test', '--workspace', '--all-targets', '--locked', '--no-run', '--message-format=json'], desktop, true)
  .split('\n').filter(Boolean).map(line => JSON.parse(line));
const artifacts = messages.filter(message => message.reason === 'compiler-artifact' && message.profile.test && message.executable);
const targets = new Map(artifacts.map(message => [message.executable, message]));
const rust = [...targets.values()].flatMap(message => rustCases(command(message.executable, ['--list'], desktop, true), `${message.target.name}/${message.target.kind.join('+')}`));
const ids = [...browser, ...frontend, ...rust];
if (!browser.length || !frontend.length || !rust.length || new Set(ids).size !== ids.length) throw new Error('Empty suite or duplicate test IDs');
if (ids.length > 200) throw new Error(`Repository has ${ids.length} test cases, exceeding the 200-case cap`);
const receipt = { cwd: root, head: command('git', ['rev-parse', 'HEAD'], root, true).trim(), counts: { browser: browser.length, frontend: frontend.length, rust: rust.length, total: ids.length }, ids, executed: false };
console.log(`Combined inventory: browser ${browser.length} + frontend ${frontend.length} + Rust ${rust.length} = ${ids.length}/200`);
if (!checkOnly) {
  function runNode(paths, expected, extra = []) {
    const text = command(process.execPath, [...extra, '--test', '--test-reporter=tap', ...paths], root, true);
    process.stdout.write(text);
    const actual = Number(text.match(/^# tests (\d+)$/m)?.[1]);
    if (actual !== expected || !/^# skipped 0$/m.test(text) || !/^# todo 0$/m.test(text)) throw new Error('Executed Node cases differ from inventory or include incomplete cases');
    return actual;
  }
  const actualBrowser = runNode(browserFiles, browser.length);
  const actualFrontend = runNode(frontendFiles, frontend.length, ['--experimental-strip-types']);
  const text = command('cargo', ['test', '--workspace', '--all-targets', '--locked'], desktop, true);
  process.stdout.write(text);
  const summaries = [...text.matchAll(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;/g)];
  const actualRust = summaries.reduce((sum, match) => sum + Number(match[1]), 0);
  if (actualRust !== rust.length || summaries.some(match => Number(match[2]) || Number(match[3]))) throw new Error('Executed Rust cases differ from inventory or include incomplete cases');
  receipt.executed = { browser: actualBrowser, frontend: actualFrontend, rust: actualRust, total: actualBrowser + actualFrontend + actualRust };
}
if (output) writeFileSync(output, JSON.stringify(receipt, null, 2) + '\n');
