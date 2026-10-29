// Native packaging only. This never dispatches a hosted build or uploads artifacts.
const {spawnSync}=require('node:child_process');
const path=require('node:path');
const fs=require('node:fs');
const os=require('node:os');
const {updaterEnabled}=require('./updater-assets.cjs');
const {macSigning}=require('./mac-release-policy.cjs');
const {run,verifyApp,verifyContainer}=require('./mac-trust.cjs');
const {stageMacHelper}=require('./mac-helper.cjs');
const args=process.argv.slice(2), platform=process.platform;
const targets={darwin:['universal-apple-darwin','aarch64-apple-darwin','x86_64-apple-darwin'],win32:['x86_64-pc-windows-msvc','aarch64-pc-windows-msvc'],linux:['x86_64-unknown-linux-gnu']};
const targetIndex=args.indexOf('--target'), target=targetIndex>=0?args[targetIndex+1]:targets[platform]?.[0];
if(!targets[platform]?.includes(target)){console.error('Choose a supported target on its native operating system.');process.exit(1);}
let updater,signing;
try {
 if(platform==='darwin')signing=macSigning(args);
 updater=updaterEnabled(args,platform,process.env);
} catch(error) { console.error(error.message); process.exit(1); }
if(updater && !JSON.parse(fs.readFileSync(path.join(__dirname,'../src-tauri/tauri.conf.json'),'utf8')).plugins?.updater?.pubkey){console.error('Updater release requires its public verification key in tauri.conf.json.');process.exit(1);}
if(args.includes('--signed')){
 const required=platform==='win32'?['WINDOWS_CERTIFICATE_THUMBPRINT']:[];
 if(required.some(name=>!process.env[name])){console.error('Signing configuration is missing. No signed release was produced.');process.exit(1);}
}
// RPM preserves source mode bits. A developer umask of 0002 must not ship
// group-writable root policy/helper files or make the repair trust check fail.
if(platform==='linux'){
 process.umask(0o022);
 for(const file of ['70-droidfun.rules','setup-robot-access','io.droidfun.usb-access.policy'])fs.chmodSync(path.join(__dirname,'../src-tauri/linux',file),0o644);
 for(const file of ['postinst','postrm'])fs.chmodSync(path.join(__dirname,'../src-tauri/linux',file),0o755);
 for(const file of fs.readdirSync(path.join(__dirname,'../src-tauri/icons'))){const full=path.join(__dirname,'../src-tauri/icons',file);if(fs.statSync(full).isFile())fs.chmodSync(full,0o644);}
}
const bundles=platform==='darwin'?'app,dmg':platform==='win32'?'nsis':'deb,rpm';
const cliArgs=[path.join(__dirname,'tauri.cjs'),'build','--target',target,...(platform==='linux'?['--no-bundle']:['--bundles',bundles])];
// Opt in explicitly in release CI. Ordinary local packaging must not require
// a production signing key even when the app has an updater public key.
// Mac updater signing happens only after the signed/stapled app survives an
// extracted-archive check. Windows retains Tauri's native installer signing.
const bundleConfig={createUpdaterArtifacts:platform==='darwin'?false:updater};
const output=path.resolve(process.env.CARGO_TARGET_DIR||path.join(os.homedir(),'.cache','deskbot','target'));
if(platform==='darwin'){
 // Change only staged copies; Tauri signs externalBin and frameworks inside out.
 const helper=stageMacHelper(output,target);
 bundleConfig.externalBin=helper.externalBin;
 bundleConfig.macOS={signingIdentity:signing.identity,hardenedRuntime:signing.mode==='developer-id',frameworks:helper.frameworks};
}
if(platform==='win32' && args.includes('--signed'))bundleConfig.windows={certificateThumbprint:process.env.WINDOWS_CERTIFICATE_THUMBPRINT,digestAlgorithm:'sha256',timestampUrl:'https://timestamp.digicert.com'};
cliArgs.push('--config',JSON.stringify({bundle:bundleConfig}));
// Empty GitHub secrets are not credentials; do not let Tauri choose an empty
// Apple ID credential set or import a certificate during an internal build.
const buildEnv={...(signing?.env||process.env)};
if(platform==='darwin'){
 buildEnv.DROIDFUN_MAC_HELPER_STAGED='1';
 for(const key of ['TAURI_SIGNING_PRIVATE_KEY','TAURI_SIGNING_PRIVATE_KEY_PASSWORD','TAURI_SIGNING_PRIVATE_KEY_PATH'])delete buildEnv[key];
}
const result=spawnSync(process.execPath,cliArgs,{cwd:path.join(__dirname,'..'),stdio:'inherit',env:buildEnv});
if(result.status!==0)process.exit(result.status??1);
if(platform==='darwin'){
 const app=path.join(output,target,'release/bundle/macos/DroidFun App.app');
 verifyApp(app,signing);
 if(updater){
  const archive=`${app}.tar.gz`;
  // Same top-level .app layout consumed by the Tauri updater. Preserve files,
  // modes and symlinks; validate the stapled app after unpacking before signing.
  run('/usr/bin/tar',['-czf',archive,'-C',path.dirname(app),path.basename(app)],{env:{...process.env,COPYFILE_DISABLE:'1'}});
  verifyContainer(archive,'updater',app,signing);
  const signed=spawnSync(process.execPath,[path.join(__dirname,'../node_modules/@tauri-apps/cli/tauri.js'),'signer','sign','--app-version',JSON.parse(fs.readFileSync(path.join(__dirname,'../package.json'),'utf8')).version,archive],{cwd:path.join(__dirname,'..'),stdio:'inherit',env:process.env});
  if(signed.status!==0)process.exit(signed.status??1);
 }
}
if(platform==='linux'){
 const output=process.env.CARGO_TARGET_DIR||path.join(os.homedir(),'.cache','deskbot','target');
 fs.chmodSync(path.join(output,target,'release','deskbot'),0o755);
 const bundle=spawnSync(process.execPath,[path.join(__dirname,'tauri.cjs'),'bundle','--target',target,'--bundles',bundles,'--config',JSON.stringify({bundle:{createUpdaterArtifacts:false}})],{cwd:path.join(__dirname,'..'),stdio:'inherit',env:process.env});
 if(bundle.status!==0)process.exit(bundle.status??1);
}
console.log(platform==='darwin'?`Mac packaging completed (${signing.mode}). Final container verification is required before distribution.`:'Native packaging completed. Signing and platform acceptance must be checked before distribution.');
