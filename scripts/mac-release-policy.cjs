// Fail closed for public Mac packages. Never print credential values.
const fs = require('node:fs');
const APPLE_ENV = ['APPLE_SIGNING_IDENTITY','APPLE_CERTIFICATE','APPLE_CERTIFICATE_PASSWORD','APPLE_ID','APPLE_PASSWORD','APPLE_TEAM_ID','APPLE_API_KEY','APPLE_API_ISSUER','APPLE_API_KEY_PATH'];
function macSigning(args, environment = process.env, {ci = !!environment.CI, exists = p => fs.statSync(p).isFile()} = {}) {
  const indexes = args.flatMap((arg,i) => arg === '--mac-signing' ? [i] : []);
  if (indexes.length > 1) throw new Error('Choose one --mac-signing mode.');
  const mode = indexes.length ? args[indexes[0]+1] : 'developer-id';
  if (!['developer-id','internal-adhoc'].includes(mode)) throw new Error('Use --mac-signing developer-id or --mac-signing internal-adhoc.');
  const env = {...environment};
  for (const key of APPLE_ENV) if (!env[key]?.trim()) delete env[key];
  if (mode === 'internal-adhoc') {
    if (args.includes('--signed') || args.includes('--updater')) throw new Error('Internal ad-hoc Mac builds cannot create public signed/updater packages.');
    for (const key of [...APPLE_ENV,'TAURI_SIGNING_PRIVATE_KEY','TAURI_SIGNING_PRIVATE_KEY_PASSWORD','TAURI_SIGNING_PRIVATE_KEY_PATH']) delete env[key];
    return {mode, identity:'-', env};
  }
  const identity = env.APPLE_SIGNING_IDENTITY;
  const match = /^Developer ID Application: .+ \(([A-Z0-9]{10})\)$/.exec(identity || '');
  if (!match) throw new Error('Public Mac releases require APPLE_SIGNING_IDENTITY for a Developer ID Application certificate. Use --mac-signing internal-adhoc only for private testing.');
  if (env.APPLE_TEAM_ID !== match[1]) throw new Error('APPLE_TEAM_ID must match the Developer ID Application signing identity.');
  if ((ci || env.APPLE_CERTIFICATE || env.APPLE_CERTIFICATE_PASSWORD) && (!env.APPLE_CERTIFICATE || !env.APPLE_CERTIFICATE_PASSWORD)) throw new Error('CI signing requires APPLE_CERTIFICATE and APPLE_CERTIFICATE_PASSWORD.');
  const appleId = ['APPLE_ID','APPLE_PASSWORD'];
  const api = ['APPLE_API_KEY','APPLE_API_ISSUER','APPLE_API_KEY_PATH'];
  const any = keys => keys.some(k => !!env[k]);
  const all = keys => keys.every(k => !!env[k]);
  if (any(appleId) && any(api)) throw new Error('Choose one notarization credential method: Apple ID or App Store Connect API.');
  if (!all(appleId) && !all(api)) throw new Error('Public Mac releases require notarization credentials: APPLE_ID and APPLE_PASSWORD (app-specific password), or APPLE_API_KEY, APPLE_API_ISSUER and APPLE_API_KEY_PATH.');
  if (all(api)) {
    let present = false;
    try { present = exists(env.APPLE_API_KEY_PATH); } catch {}
    if (!present) throw new Error('APPLE_API_KEY_PATH must point to the notarization key file.');
  }
  return {mode,identity,teamId:match[1],env};
}
function requirePublicMac(evidence) {
  if (evidence.signingMode !== 'developer-id' || evidence.distribution !== 'public' || evidence.codeSignatureVerified !== true || evidence.developerIdSigned !== true || evidence.notarized !== true || evidence.stapled !== true || evidence.gatekeeperAccepted !== true || evidence.helperSigned !== true || evidence.librarySigned !== true || evidence.helperRuntimeVerified !== true || !evidence.architectures?.includes('arm64') || !evidence.architectures?.includes('x86_64') || !['dmg','zip','updater'].every(kind => evidence.verifiedContainers?.includes(kind))) throw new Error('Public Mac distribution requires a verified Developer ID signature, notarization, stapled ticket, helpers and all final containers. Internal ad-hoc packages cannot be published.');
}
module.exports = {macSigning, requirePublicMac};
if (require.main === module) {
  try { const signing = macSigning(process.argv.slice(2)); console.log(`Mac signing preflight passed: ${signing.mode}.`); }
  catch (error) { console.error(error.message); process.exitCode=1; }
}
