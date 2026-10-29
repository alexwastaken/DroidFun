const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const {assetNames, collectUpdater, digest, verifySignature, validateUpdaterEvidence, updaterEnabled} = require('./updater-assets.cjs');
const {generate} = require('./generate-updater-manifest.cjs');

// Public RFC 8032 section 7.1 test vector. This is deliberately known test data,
// never a generated key, credential, or key suitable for a real release.
const seed = Buffer.from('9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60','hex');
const publicBytes = Buffer.from('d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a','hex');
const fixtureKey = crypto.createPrivateKey({key:Buffer.concat([Buffer.from('302e020100300506032b657004220420','hex'),seed]),format:'der',type:'pkcs8'});
const keyId = Buffer.from('0102030405060708','hex');
const publicKey = Buffer.from(`untrusted comment: public RFC test fixture\n${Buffer.concat([Buffer.from('Ed'),keyId,publicBytes]).toString('base64')}\n`).toString('base64');
function signFixture(data,version,file) {
  const signature=crypto.sign(null,crypto.createHash('blake2b512').update(data).digest(),fixtureKey);
  const comment=`timestamp:1700000000\tfile:${file}\tversion:${version}`;
  const global=crypto.sign(null,Buffer.concat([signature,Buffer.from(comment)]),fixtureKey);
  return Buffer.from(`untrusted comment: public test fixture\n${Buffer.concat([Buffer.from('ED'),keyId,signature]).toString('base64')}\ntrusted comment: ${comment}\n${global.toString('base64')}\n`).toString('base64');
}
function fixture(t) {
  const directory=fs.mkdtempSync(path.join(os.tmpdir(),'droidfun-updater-test-'));
  t.after(()=>fs.rmSync(directory,{recursive:true,force:true}));
  const version='0.4.2',source='a'.repeat(40),evidence=[],assets=[];
  for(const platform of ['macOS-universal','Windows-x64','Windows-ARM64']){
    const original=platform==='macOS-universal'?'DroidFun App.app.tar.gz':`DroidFun App_${version}_${platform}-setup.exe`;
    const file=path.join(directory,original),data=Buffer.from(`test updater bytes for ${platform}`);
    fs.writeFileSync(file,data);fs.writeFileSync(`${file}.sig`,signFixture(data,version,original));
    const updater=collectUpdater(file,directory,version,platform,publicKey);
    const checked=[updater.assetName,updater.signatureName].map(name=>digest(path.join(directory,name)));
    evidence.push({version,platform,applicationCommit:source,tests:{passed:80,failed:0},architectures:['arm64','x86_64'],signingMode:'developer-id',distribution:'public',codeSignatureVerified:true,developerIdSigned:true,notarized:true,stapled:true,gatekeeperAccepted:true,helperSigned:true,librarySigned:true,helperRuntimeVerified:true,verifiedContainers:['dmg','zip','updater'],smoke:{result:'passed',installedBinaryMatchesPackage:true,nativeWindowCreated:true},updater,assets:checked});
    assets.push(...checked.map(a=>({...a,size:a.bytes,state:'uploaded',digest:`sha256:${a.sha256}`,browser_download_url:`https://github.com/alexwastaken/DroidFun/releases/download/v${version}/${a.name}`})));
  }
  const release={draft:false,prerelease:true,tag_name:`v${version}`,target_commitish:source,published_at:'2026-10-25T12:00:00Z',html_url:`https://github.com/alexwastaken/DroidFun/releases/tag/v${version}`,assets};
  return {version,source,evidence,directory,release,publicKey};
}
test('local builds need no signing key; updater release fails without one',()=>{
  for(const platform of ['darwin','win32','linux'])assert.equal(updaterEnabled([],platform,{}),false);
  for(const platform of ['darwin','win32']){
    assert.throws(()=>updaterEnabled(['--updater'],platform,{}),/TAURI_SIGNING_PRIVATE_KEY/);
    assert.equal(updaterEnabled(['--updater'],platform,{TAURI_SIGNING_PRIVATE_KEY:'test placeholder'}),true);
  }
  assert.throws(()=>updaterEnabled(['--updater'],'linux',{}),/DEB\/RPM/);
});
test('signatures authenticate bytes, key, version and original filename',()=>{
  const data=Buffer.from('fixture'),signature=signFixture(data,'0.4.2','app.exe');
  assert.equal(verifySignature(data,signature,publicKey,'0.4.2','app.exe').signedVersion,'0.4.2');
  assert.throws(()=>verifySignature(Buffer.from('changed'),signature,publicKey,'0.4.2'),/verification failed/);
  assert.throws(()=>verifySignature(data,signature,publicKey,'0.4.3'),/exact app version/);
  assert.throws(()=>verifySignature(data,signature,publicKey,'0.4.2','other.exe'),/filename/);
  const tampered=Buffer.from(Buffer.from(signature,'base64').toString().replace('version:0.4.2','version:0.4.3')).toString('base64');
  assert.throws(()=>verifySignature(data,tampered,publicKey,'0.4.3'),/verification failed/);
  assert.throws(()=>verifySignature(data,signature+'!',publicKey,'0.4.2'),/encoding/);
});
test('independent upstream Minisign vector verifies before rejecting absent version',()=>{
  // https://github.com/jedisct1/rust-minisign-verify/blob/master/src/lib.rs
  const pub=Buffer.from('untrusted comment: upstream test\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3\n').toString('base64');
  const signature=Buffer.from('untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1633700835\tfile:test\tprehashed\nwLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==').toString('base64');
  assert.throws(()=>verifySignature(Buffer.from('test'),signature,pub,'0.4.2'),/exact app version/);
});
test('published prerelease yields exact universal Mac and Windows targets',t=>{
  const input=fixture(t),manifest=generate(input);
  assert.deepEqual(Object.keys(manifest.platforms),['darwin-aarch64','darwin-x86_64','windows-x86_64','windows-aarch64']);
  assert.equal(manifest.platforms['darwin-aarch64'].url,manifest.platforms['darwin-x86_64'].url);
  assert.ok(manifest.platforms['darwin-aarch64'].url.endsWith('.app.tar.gz'));
  assert.ok(manifest.platforms['windows-aarch64'].url.endsWith('Windows-ARM64-setup.exe'));
  assert.ok(!JSON.stringify(manifest).includes('/latest/'));
});
test('feed rejects draft, stale source, missing platform and changed published bytes',t=>{
  const input=fixture(t);
  for(const change of [{draft:true},{target_commitish:'b'.repeat(40)},{tag_name:'v0.4.1'},{published_at:null}])assert.throws(()=>generate({...input,release:{...input.release,...change}}),/already be published/);
  assert.throws(()=>generate({...input,evidence:input.evidence.slice(1)}),/Expected one/);
  const release=structuredClone(input.release);release.assets[0].digest='sha256:'+'0'.repeat(64);
  assert.throws(()=>generate({...input,release}),/Published asset/);
  release.assets[0]=input.release.assets[0];release.assets.push(input.release.assets[0]);
  assert.throws(()=>generate({...input,release}),/Published asset/);
});
test('feed refuses an internal or unnotarized Mac release even with a valid updater signature',t=>{
  const input=fixture(t);
  for(const change of [{signingMode:'internal-adhoc'},{notarized:false},{stapled:false},{helperSigned:false},{verifiedContainers:['dmg','zip']}]){
    const evidence=structuredClone(input.evidence);Object.assign(evidence[0],change);
    assert.throws(()=>generate({...input,evidence}),/Public Mac distribution/);
  }
});
test('evidence rejects modified local bytes, signatures and stale metadata',t=>{
  const input=fixture(t),item=input.evidence[0];
  const altered=structuredClone(item);altered.updater.signedVersion='0.4.1';
  assert.throws(()=>validateUpdaterEvidence(altered,input.directory,publicKey),/evidence mismatch/);
  fs.appendFileSync(path.join(input.directory,item.updater.assetName),'changed');
  assert.throws(()=>generate(input),/checksum mismatch/);
  assert.throws(()=>assetNames('0.4.2/evil','Windows-x64'),/version/);
  assert.throws(()=>assetNames('0.4.2','Linux-x86_64'),/Unsupported/);
});
