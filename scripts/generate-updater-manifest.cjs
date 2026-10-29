// Generate the static feed only after exact, verified assets are published.
// This script does not publish a release, upload files, commit, or push.
const fs = require('node:fs');
const path = require('node:path');
const {execFileSync} = require('node:child_process');
const {assetNames, validateUpdaterEvidence, SEMVER} = require('./updater-assets.cjs');
const {requirePublicMac}=require('./mac-release-policy.cjs');
const REPO = 'alexwastaken/DroidFun';
const PLATFORMS = {
  'macOS-universal': ['darwin-aarch64', 'darwin-x86_64'],
  'Windows-x64': ['windows-x86_64'],
  'Windows-ARM64': ['windows-aarch64'],
};
function generate({version, source, release, evidence, directory, publicKey, notes = ''}) {
  if (!SEMVER.test(version) || !/^[0-9a-f]{40}$/.test(source)) throw new Error('Exact release version and source commit required');
  if (release.draft !== false || release.tag_name !== `v${version}` || release.target_commitish !== source || !release.published_at || !Number.isFinite(Date.parse(release.published_at)) || release.html_url !== `https://github.com/${REPO}/releases/tag/v${version}`) throw new Error('Release must already be published for the exact source/version');
  if (!Array.isArray(release.assets) || !Array.isArray(evidence)) throw new Error('Release asset evidence is missing');
  const platforms = {};
  for (const [platform, targets] of Object.entries(PLATFORMS)) {
    const records = evidence.filter(e => e.platform === platform);
    if (records.length !== 1) throw new Error(`Expected one ${platform} evidence record`);
    const item = records[0];
    if (item.version !== version || item.applicationCommit !== source || !(item.tests?.passed > 0) || item.tests.failed !== 0) throw new Error(`Source/test evidence mismatch: ${platform}`);
    if (platform === 'macOS-universal') {
      requirePublicMac(item);
    } else if (item.smoke?.result !== 'passed' || item.smoke.installedBinaryMatchesPackage !== true || item.smoke.nativeWindowCreated !== true) throw new Error(`Native install verification is required: ${platform}`);
    const verified = validateUpdaterEvidence(item, directory, publicKey);
    const names = assetNames(version, platform);
    let url;
    for (const name of [names.asset, names.signature]) {
      const published = release.assets.filter(a => a.name === name);
      const checked = item.assets.find(a => a.name === name);
      const expectedUrl = `https://github.com/${REPO}/releases/download/v${version}/${name}`;
      if (published.length !== 1 || published[0].state !== 'uploaded' || published[0].size !== checked.bytes || published[0].digest !== `sha256:${checked.sha256}` || published[0].browser_download_url !== expectedUrl) throw new Error(`Published asset does not match verified bytes: ${name}`);
      if (name === names.asset) url = expectedUrl;
    }
    for (const target of targets) platforms[target] = {url, signature: verified.signature};
  }
  return {version, notes, pub_date: release.published_at, platforms};
}
if (require.main === module) {
  const args = process.argv.slice(2);
  const allowed = ['--version', '--source', '--packages', '--output', '--notes'];
  if (args.length % 2 || args.some((v,i) => i % 2 === 0 && !allowed.includes(v))) throw new Error('Use --version VERSION --source COMMIT --packages DIRECTORY --output FILE [--notes FILE]');
  const options = {};
  for (let i=0;i<args.length;i+=2) { if (options[args[i]]) throw new Error('Duplicate argument'); options[args[i]] = args[i+1]; }
  for (const name of allowed.slice(0,4)) if (!options[name]) throw new Error(`Missing ${name}`);
  const version = options['--version'];
  if (!SEMVER.test(version)) throw new Error('Invalid version');
  const directory = path.resolve(options['--packages']);
  const evidence = fs.readdirSync(directory).filter(n => n.startsWith(`verification-${version}-`) && n.endsWith('.json')).map(n => JSON.parse(fs.readFileSync(path.join(directory,n),'utf8')));
  const release = JSON.parse(execFileSync('gh',['api',`repos/${REPO}/releases/tags/v${version}`],{encoding:'utf8',maxBuffer:8*1024*1024}));
  const config = JSON.parse(fs.readFileSync(path.join(__dirname,'../src-tauri/tauri.conf.json'),'utf8'));
  const manifest = generate({version,source:options['--source'],release,evidence,directory,publicKey:config.plugins?.updater?.pubkey,notes:options['--notes']?fs.readFileSync(options['--notes'],'utf8'):''});
  const output = path.resolve(options['--output']);
  fs.mkdirSync(path.dirname(output),{recursive:true});
  const temp = `${output}.${process.pid}.tmp`;
  try { fs.writeFileSync(temp,JSON.stringify(manifest,null,2)+'\n',{flag:'wx'}); fs.renameSync(temp,output); }
  finally { if(fs.existsSync(temp))fs.unlinkSync(temp); }
  console.log(`Validated ${Object.keys(manifest.platforms).length} updater targets for ${version}; wrote ${output}. No publication performed.`);
}
module.exports = {generate, PLATFORMS};
