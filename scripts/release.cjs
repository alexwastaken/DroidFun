// Native packaging only. This never dispatches a hosted build or uploads artifacts.
const {spawnSync}=require('node:child_process');
const path=require('node:path');
const fs=require('node:fs');
const os=require('node:os');
const args=process.argv.slice(2), platform=process.platform;
const targets={darwin:['universal-apple-darwin','aarch64-apple-darwin','x86_64-apple-darwin'],win32:['x86_64-pc-windows-msvc','aarch64-pc-windows-msvc'],linux:['x86_64-unknown-linux-gnu']};
const targetIndex=args.indexOf('--target'), target=targetIndex>=0?args[targetIndex+1]:targets[platform]?.[0];
if(!targets[platform]?.includes(target)){console.error('Choose a supported target on its native operating system.');process.exit(1);}
if(args.includes('--signed')){
 const required=platform==='darwin'?['APPLE_SIGNING_IDENTITY']:platform==='win32'?['WINDOWS_CERTIFICATE_THUMBPRINT']:[];
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
// Sign before Tauri creates the disk image so both downloads contain the same
// verified bundle. '-' is an ad-hoc signature, not Developer ID or notarization.
if(platform==='darwin')cliArgs.push('--config',JSON.stringify({bundle:{macOS:{signingIdentity:process.env.APPLE_SIGNING_IDENTITY||'-'}}}));
if(platform==='win32' && args.includes('--signed'))cliArgs.push('--config',JSON.stringify({bundle:{windows:{certificateThumbprint:process.env.WINDOWS_CERTIFICATE_THUMBPRINT,digestAlgorithm:'sha256',timestampUrl:'https://timestamp.digicert.com'}}}));
const result=spawnSync(process.execPath,cliArgs,{cwd:path.join(__dirname,'..'),stdio:'inherit',env:process.env});
if(result.status!==0)process.exit(result.status??1);
if(platform==='linux'){
 const output=process.env.CARGO_TARGET_DIR||path.join(os.homedir(),'.cache','deskbot','target');
 fs.chmodSync(path.join(output,target,'release','deskbot'),0o755);
 const bundle=spawnSync(process.execPath,[path.join(__dirname,'tauri.cjs'),'bundle','--target',target,'--bundles',bundles],{cwd:path.join(__dirname,'..'),stdio:'inherit',env:process.env});
 if(bundle.status!==0)process.exit(bundle.status??1);
}
console.log('Native packaging completed. Signing/notarization and platform acceptance must be checked before distribution.');
