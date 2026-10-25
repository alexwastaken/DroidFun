// Read-only validation shared by packaging, draft uploads, and feed generation.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;
function base64(value, label) {
  if (typeof value !== 'string' || !value || value.length > 16384 || !/^[A-Za-z0-9+/]+={0,2}$/.test(value)) throw new Error(`Invalid ${label} encoding`);
  const bytes = Buffer.from(value, 'base64');
  if (bytes.toString('base64') !== value) throw new Error(`Invalid ${label} encoding`);
  return bytes;
}
function safeName(name) {
  if (typeof name !== 'string' || !name || /[\\/\r\n\t\0]/.test(name) || name === '.' || name === '..') throw new Error('Invalid updater asset name');
  return name;
}
function assetNames(version, platform) {
  if (!SEMVER.test(version)) throw new Error('Invalid updater version');
  if (!['macOS-universal', 'Windows-x64', 'Windows-ARM64'].includes(platform)) throw new Error('Unsupported updater platform');
  const asset = `DroidFun-App-${version}-${platform}${platform === 'macOS-universal' ? '.app.tar.gz' : '-setup.exe'}`;
  return {asset, signature: `${asset}.sig`};
}
function digest(file) {
  const data = fs.readFileSync(file);
  return {name: path.basename(file), bytes: data.length, sha256: crypto.createHash('sha256').update(data).digest('hex')};
}
function verifySignature(data, encodedSignature, encodedPublicKey, version, originalName) {
  if (!SEMVER.test(version)) throw new Error('Invalid updater version');
  if (typeof encodedPublicKey !== 'string' || typeof encodedSignature !== 'string') throw new Error('Updater public key and signature are required');
  const keyLines = base64(encodedPublicKey.trim(), 'updater public key').toString('utf8').trimEnd().split(/\r?\n/);
  const lines = base64(encodedSignature.trim(), 'updater signature').toString('utf8').trimEnd().split(/\r?\n/);
  if (keyLines.length !== 2 || !keyLines[0].startsWith('untrusted comment: ') || lines.length !== 4 || !lines[0].startsWith('untrusted comment: ') || !lines[2].startsWith('trusted comment: ')) throw new Error('Invalid updater signature format');
  const key = base64(keyLines[1], 'public key');
  const packet = base64(lines[1], 'signature packet');
  const global = base64(lines[3], 'trusted comment signature');
  if (key.length !== 42 || !['Ed', 'ED'].includes(key.subarray(0, 2).toString()) || packet.length !== 74 || packet.subarray(0, 2).toString() !== 'ED' || global.length !== 64 || !key.subarray(2, 10).equals(packet.subarray(2, 10))) throw new Error('Updater signature key or algorithm does not match');
  const publicKey = crypto.createPublicKey({key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), key.subarray(10)]), format: 'der', type: 'spki'});
  const signature = packet.subarray(10);
  const comment = lines[2].slice('trusted comment: '.length);
  // Minisign ED: Ed25519 over BLAKE2b-512(data), and a second Ed25519
  // signature over signature || trusted-comment. See rust-minisign-verify.
  if (!crypto.verify(null, crypto.createHash('blake2b512').update(data).digest(), publicKey, signature) || !crypto.verify(null, Buffer.concat([signature, Buffer.from(comment)]), publicKey, global)) throw new Error('Updater signature verification failed');
  const fields = comment.split('\t');
  const values = prefix => fields.filter(f => f.startsWith(prefix)).map(f => f.slice(prefix.length));
  const versions = values('version:');
  const files = values('file:');
  const timestamps = values('timestamp:');
  if (versions.length !== 1 || versions[0] !== version || files.length !== 1 || timestamps.length !== 1 || !/^\d+$/.test(timestamps[0])) throw new Error('Updater signature must authenticate this exact app version');
  safeName(files[0]);
  if (originalName !== undefined && files[0] !== originalName) throw new Error('Signed updater filename does not match the built package');
  return {signature: encodedSignature.trim(), keyId: key.subarray(2, 10).toString('hex'), signedVersion: version, signedFileName: files[0]};
}
function collectUpdater(source, out, version, platform, publicKey) {
  const names = assetNames(version, platform);
  const sig = fs.readFileSync(`${source}.sig`, 'utf8').trim();
  const verified = verifySignature(fs.readFileSync(source), sig, publicKey, version, path.basename(source));
  fs.copyFileSync(source, path.join(out, names.asset));
  fs.copyFileSync(`${source}.sig`, path.join(out, names.signature));
  return {assetName: names.asset, signatureName: names.signature, signatureVerified: true, ...verified};
}
function validateUpdaterEvidence(evidence, directory, publicKey) {
  if (!Array.isArray(evidence.assets)) throw new Error('Updater assets are missing');
  const names = assetNames(evidence.version, evidence.platform);
  const updater = evidence.updater;
  if (!updater || updater.assetName !== names.asset || updater.signatureName !== names.signature || updater.signatureVerified !== true) throw new Error('Signed updater evidence is missing or has unexpected names');
  for (const name of [names.asset, names.signature]) {
    const recorded = evidence.assets.filter(a => a.name === name);
    const actual = digest(path.join(directory, name));
    if (recorded.length !== 1 || recorded[0].bytes !== actual.bytes || recorded[0].sha256 !== actual.sha256) throw new Error(`Updater asset checksum mismatch: ${name}`);
  }
  const verified = verifySignature(fs.readFileSync(path.join(directory, names.asset)), fs.readFileSync(path.join(directory, names.signature), 'utf8').trim(), publicKey, evidence.version, updater.signedFileName);
  for (const field of ['signature', 'keyId', 'signedVersion', 'signedFileName']) if (updater[field] !== verified[field]) throw new Error(`Updater signature evidence mismatch: ${field}`);
  return verified;
}
function updaterEnabled(args, platform, env) {
  if (!args.includes('--updater')) return false;
  if (!['darwin','win32'].includes(platform)) throw new Error('In-app updater packages are supported on macOS and Windows; Linux uses DEB/RPM downloads.');
  if (!env.TAURI_SIGNING_PRIVATE_KEY?.trim()) throw new Error('Updater release requires TAURI_SIGNING_PRIVATE_KEY. No updater release was produced.');
  return true;
}
module.exports = {assetNames, digest, verifySignature, collectUpdater, validateUpdaterEvidence, safeName, SEMVER, updaterEnabled};
