// Build tooling is checked out separately so a release can use its original source commit.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
const out = process.env.DESKBOT_ARTIFACT_DIR;
if (!out) throw new Error('DESKBOT_ARTIFACT_DIR must be set');
fs.mkdirSync(out, { recursive: true });
const run = (cmd, args) => {
  const r = spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
  if (r.error || r.status !== 0) throw new Error(`${cmd} failed: ${r.error || r.stderr}`);
  return r.stdout.trim();
};
if (process.argv[2] === 'test') {
  const r = spawnSync('cargo', ['test', '--locked', '--manifest-path', 'src-tauri/Cargo.toml', '--lib', '--target', process.env.DESKBOT_TARGET], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
  fs.writeFileSync(path.join(out, 'native-tests.txt'), (r.stdout || '') + (r.stderr || ''));
  process.stdout.write(r.stdout || '');
  process.stderr.write(r.stderr || '');
  if (r.error || r.status !== 0 || !/test result: ok\. [1-9]\d* passed; 0 failed/.test(r.stdout)) process.exit(r.status || 1);
} else if (process.argv[2] === 'collect') {
  const version = JSON.parse(fs.readFileSync('package.json')).version;
  if (`v${version}` !== process.env.DESKBOT_RELEASE_TAG) throw new Error('Source version does not match release');
  const target = process.env.DESKBOT_TARGET;
  const platform = process.env.DESKBOT_PLATFORM;
  const release = path.join(process.env.CARGO_TARGET_DIR, target, 'release');
  const binary = fs.readFileSync(path.join(release, process.platform === 'win32' ? 'deskbot.exe' : 'deskbot'));
  let architecture;
  if (process.platform === 'win32') {
    const offset = binary.readUInt32LE(0x3c);
    if (binary.toString('ascii', offset, offset + 4) !== 'PE\0\0') throw new Error('Invalid PE executable');
    const machine = binary.readUInt16LE(offset + 4);
    architecture = machine === 0x8664 ? 'x64' : machine === 0xaa64 ? 'ARM64' : 'unknown';
  } else {
    if (!binary.subarray(0, 4).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46])) || binary[4] !== 2 || binary[5] !== 1 || binary.readUInt16LE(18) !== 62) throw new Error('Expected Linux x86_64 ELF executable');
    architecture = 'x86_64';
  }
  if (!platform.endsWith(architecture)) throw new Error('Binary architecture differs from requested package');
  const walk = dir => fs.readdirSync(dir, { withFileTypes: true }).flatMap(e => e.isDirectory() ? walk(path.join(dir, e.name)) : [path.join(dir, e.name)]);
  const extensions = process.platform === 'win32' ? ['.exe'] : ['.deb', '.rpm'];
  const assets = extensions.map(ext => {
    const matches = walk(path.join(release, 'bundle')).filter(f => path.extname(f) === ext);
    if (matches.length !== 1) throw new Error(`Expected one ${ext} package, found ${matches.length}`);
    const name = `Deskbot-${version}-${platform}${ext === '.exe' ? '-setup' : ''}${ext}`;
    fs.copyFileSync(matches[0], path.join(out, name));
    const data = fs.readFileSync(path.join(out, name));
    return { name, bytes: data.length, sha256: crypto.createHash('sha256').update(data).digest('hex') };
  });
  const testLog = fs.readFileSync(path.join(out, 'native-tests.txt'), 'utf8');
  const passed = Number(testLog.match(/test result: ok\. (\d+) passed; 0 failed/)?.[1]);
  if (!(passed > 0)) throw new Error('Native tests did not pass');
  const smoke = JSON.parse(fs.readFileSync(path.join(out, 'native-smoke.json'), 'utf8').replace(/^\uFEFF/, ''));
  if (smoke.result !== 'passed') throw new Error('Installed package launch check did not pass');
  const evidence = { version, platform, target, applicationCommit: run('git', ['rev-parse', 'HEAD']), pipelineCommit: process.env.GITHUB_SHA, runUrl: `https://github.com/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}`, runner: { os: process.env.RUNNER_OS, architecture: process.env.RUNNER_ARCH, image: process.env.ImageVersion }, rust: run('rustc', ['--version']), node: process.version, tests: { passed, failed: 0 }, architecture, smoke, signed: false, assets, remaining: ['Physical robot USB behavior', 'Live account sign-in and usage on this OS', 'Human tray and window interaction', 'Installation/upgrade/uninstall acceptance beyond recorded checks', 'Code signing'] };
  fs.writeFileSync(path.join(out, `verification-${version}-${platform}.json`), JSON.stringify(evidence, null, 2) + '\n');
  fs.renameSync(path.join(out, 'native-tests.txt'), path.join(out, `native-tests-${platform}.txt`));
  fs.renameSync(path.join(out, 'native-smoke.json'), path.join(out, `native-smoke-${platform}.json`));
  fs.writeFileSync(path.join(out, `SHA256SUMS-${version}-${platform}.txt`), assets.map(a => `${a.sha256}  ${a.name}`).join('\n') + '\n');
  console.log(JSON.stringify(evidence, null, 2));
} else throw new Error('Use test or collect');
