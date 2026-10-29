// Source-level release guards for the public customer's native/UI boundary.
// This does not replace runtime IPC tests or inspect packaged binaries for secrets.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const read = relative => fs.readFileSync(path.join(root, relative), 'utf8');
const json = relative => JSON.parse(read(relative));
const sorted = values => [...values].sort();
const privateCommand = /(?:^|_)(?:workshop|owner|admin|registry|database|sql)(?:_|$)/i;

function directives(value) {
  assert.equal(typeof value, 'string', 'CSP must remain explicit and reviewable');
  const result = new Map();
  for (const part of value.split(';').map(part => part.trim()).filter(Boolean)) {
    const [name, ...sources] = part.split(/\s+/);
    assert.ok(!result.has(name), `Duplicate CSP directive: ${name}`);
    result.set(name, sources);
  }
  return result;
}

test('every customer command has an explicit matching permission, without owner or Workshop commands', () => {
  const handlers = [...read('src-tauri/src/lib.rs').matchAll(/tauri::generate_handler!\s*\[([\s\S]*?)\]/g)];
  assert.equal(handlers.length, 1, 'Review additional native command handlers before publishing');
  const commands = handlers[0][1].split(',').map(value => value.trim()).filter(Boolean).map(value => {
    assert.match(value, /^(?:[a-z_][a-z0-9_]*::)*[a-z_][a-z0-9_]*$/, 'Unexpected command registration syntax');
    assert.ok(!privateCommand.test(value.replaceAll('::', '_')), `Private command registered: ${value}`);
    return value.split('::').at(-1);
  });
  assert.ok(commands.length > 0);
  assert.equal(new Set(commands).size, commands.length, 'Command names must be unambiguous');
  const permissions = json('src-tauri/permissions/customer.json');
  assert.equal(permissions.permission?.length, 1);
  assert.ok(!permissions.default && !permissions.set, 'No implicit permission sets');
  const permission = permissions.permission[0];
  assert.equal(permission.identifier, 'customer-controls');
  assert.deepEqual(Object.keys(permission.commands), ['allow']);
  assert.ok(Array.isArray(permission.commands.allow));
  assert.equal(new Set(permission.commands.allow).size, permission.commands.allow.length);
  for (const command of permission.commands.allow) {
    assert.equal(typeof command, 'string');
    assert.ok(!privateCommand.test(command), `Private command granted: ${command}`);
  }
  assert.deepEqual(sorted(permission.commands.allow), sorted(commands),
    'Native commands and the explicit customer permission must be reviewed together');
});

test('only the local main window receives the minimal customer capability', () => {
  const config = json('src-tauri/tauri.conf.json');
  assert.deepEqual(config.app.security.capabilities, ['default']);
  const capability = json('src-tauri/capabilities/default.json');
  assert.equal(capability.identifier, 'default');
  assert.equal(capability.local, true);
  assert.ok(!Object.hasOwn(capability, 'remote'), 'Remote origins must not receive native commands');
  assert.deepEqual(capability.windows, ['main']);
  assert.ok(!Object.hasOwn(capability, 'webviews'), 'Do not add independent webview grants');
  assert.deepEqual(sorted(capability.permissions), sorted([
    'core:event:allow-listen', 'core:event:allow-unlisten', 'customer-controls',
  ]), 'No broad core, shell, filesystem, SQL or plugin grants in the customer webview');
});

test('production CSP isolates the installed UI from development servers and remote code', () => {
  const security = json('src-tauri/tauri.conf.json').app.security;
  const csp = directives(security.csp);
  assert.deepEqual(csp.get('default-src'), ["'self'"]);
  assert.deepEqual(csp.get('script-src') ?? csp.get('default-src'), ["'self'"]);
  for (const name of ['script-src-elem', 'script-src-attr']) {
    if (csp.has(name)) assert.ok(csp.get(name).every(source => ["'self'", "'none'"].includes(source)));
  }
  for (const name of ['frame-src', 'object-src', 'base-uri', 'form-action']) {
    assert.deepEqual(csp.get(name), ["'none'"], `${name} must stay disabled`);
  }
  const connections = csp.get('connect-src');
  assert.ok(connections?.length);
  for (const source of connections) {
    assert.ok(["'self'", 'ipc:', 'http://ipc.localhost', 'https://ipc.localhost'].includes(source),
      `Production UI connection outside local IPC: ${source}`);
  }
  assert.doesNotMatch(security.csp, /(?:https?|wss?):\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1\])(?::|\/|\s|;|$)/i);
  const dev = directives(security.devCsp);
  assert.ok(dev.get('connect-src')?.includes('http://127.0.0.1:1420'));
  assert.ok(dev.get('connect-src')?.includes('ws://127.0.0.1:1420'));
  assert.notEqual(security.csp, security.devCsp, 'Development access must not ship as production CSP');
});

test('platform packaging configs do not override the reviewed customer security boundary', () => {
  const tauri = path.join(root, 'src-tauri');
  for (const filename of fs.readdirSync(tauri).filter(name => /^tauri\..+\.conf\.json$/.test(name))) {
    const config = json(`src-tauri/${filename}`);
    assert.ok(!config.app?.security, `${filename} needs an explicit security review`);
  }
});

test('customer model lookup stays a bounded public hint with no owner or Workshop API', () => {
  const source = read('src-tauri/src/droid_registry.rs');
  const endpoint = source.match(/const ENDPOINT:\s*&str\s*=\s*"([^"]+)"\s*;/)?.[1];
  assert.equal(endpoint, 'https://droidfun.io/api/droids/identify');
  assert.deepEqual([...source.matchAll(/https?:\/\/[^"'\s]+/g)].map(match => match[0]), [endpoint]);
  assert.doesNotMatch(source, /\/api\/(?:admin|workshop)(?:\/|\b)|bearer_auth|basic_auth|Authorization|SESSION_SECRET|ADMIN_SETUP_TOKEN/);
  assert.match(source, /\.post\(ENDPOINT\)/);
  assert.match(source, /\.https_only\(true\)/);
  assert.match(source, /\.redirect\(reqwest::redirect::Policy::none\(\)\)/);
  assert.match(source, /#\[serde\(deny_unknown_fields\)\]/);
  assert.match(source, /bytes\.len\(\)\s*\+\s*chunk\.len\(\)\s*>\s*256/);
});
