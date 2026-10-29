// Opt-in transport for verified installers when Actions artifact storage is full.
// Never creates or publishes a release, replaces an asset, or uses a saved token.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const {execFileSync} = require('node:child_process');
const {validateUpdaterEvidence} = require('./updater-assets.cjs');
const {requirePublicMac}=require('./mac-release-policy.cjs');
const run = (args) => execFileSync('gh', args, {encoding:'utf8', maxBuffer:8*1024*1024}).trim();
const repo = process.env.GITHUB_REPOSITORY;
if (repo !== 'alexwastaken/DroidFun') throw new Error('Draft uploads are restricted to the DroidFun repository');
if (!process.env.GH_TOKEN) throw new Error('An ephemeral workflow token is required');
const source = process.env.DESKBOT_SOURCE_SHA;
const checkedOut = execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim();
if (!/^[0-9a-f]{40}$/.test(source || '') || source !== checkedOut) throw new Error('An exact checked-out source commit is required');
const version = JSON.parse(fs.readFileSync('package.json','utf8')).version;
const tag = `v${version}`;
if (process.env.DESKBOT_RELEASE_TAG && process.env.DESKBOT_RELEASE_TAG !== tag) throw new Error('Requested release does not match source version');
const release = JSON.parse(run(['release','view',tag,'--repo',repo,'--json','isDraft,tagName,targetCommitish']));
if (release.isDraft !== true || release.tagName !== tag || release.targetCommitish !== source) throw new Error('Existing draft must match the source version and exact commit');
const out = path.resolve(process.env.DESKBOT_ARTIFACT_DIR || 'packages');
const names = fs.readdirSync(out).filter(name => fs.lstatSync(path.join(out,name)).isFile());
const evidenceNames = names.filter(name => name.startsWith(`verification-${version}-`) && name.endsWith('.json'));
if (evidenceNames.length !== 1) throw new Error('Expected one verified native package record for this runner');
const evidence = JSON.parse(fs.readFileSync(path.join(out,evidenceNames[0]),'utf8'));
if (evidence.version !== version || evidence.applicationCommit !== source || !(evidence.tests?.passed > 0) || evidence.tests.failed !== 0) throw new Error('Native evidence does not match passing source tests');
if (process.platform === 'darwin') {
  requirePublicMac(evidence);
} else if (evidence.smoke?.result !== 'passed' || evidence.smoke.installedBinaryMatchesPackage !== true || evidence.smoke.nativeWindowCreated !== true) {
  throw new Error('Installed native package smoke verification is required');
}
if (!Array.isArray(evidence.assets) || !evidence.assets.length) throw new Error('Verified assets are missing');
for (const asset of evidence.assets) {
  if (path.basename(asset.name) !== asset.name || !names.includes(asset.name)) throw new Error('Invalid verified asset path');
  const bytes = fs.readFileSync(path.join(out,asset.name));
  if (bytes.length !== asset.bytes || crypto.createHash('sha256').update(bytes).digest('hex') !== asset.sha256) throw new Error(`Checksum mismatch: ${asset.name}`);
}
if(process.platform === 'darwin' || process.platform === 'win32'){
  const config=JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json','utf8'));
  validateUpdaterEvidence(evidence,out,config.plugins?.updater?.pubkey);
}
// Preserve checksums, native tests, smoke reports and screenshots with the exact
// installers. No --clobber: retries can never silently replace an earlier build.
const files = names.filter(name => /\.(?:exe|dmg|zip|deb|rpm|json|txt|png|sig)$/.test(name) || name.endsWith('.app.tar.gz')).map(name => path.join(out,name));
if (files.length !== names.length) throw new Error('Unexpected file in package evidence directory');
for(const name of names.filter(name => /\.(?:exe|dmg|zip|deb|rpm|sig)$/.test(name) || name.endsWith('.app.tar.gz'))){
  if(!evidence.assets.some(asset => asset.name === name))throw new Error(`Unverified package or signature: ${name}`);
}
run(['release','upload',tag,'--repo',repo,...files]);
console.log(JSON.stringify({uploadedToDraft:tag,source,files:names,releasePublished:false}));
