// Run on the owner's Mac after collecting the two private CI packages.
// Signs locally; the private updater key never enters CI, source or packages.
const fs=require('node:fs');
const path=require('node:path');
const os=require('node:os');
const crypto=require('node:crypto');
const {execFileSync}=require('node:child_process');
const {verifySignature}=require('./updater-assets.cjs');
const root=path.resolve(__dirname,'..');
const input=path.resolve(process.argv[2]||'.');
const site=process.argv[3]?path.resolve(process.argv[3]):path.resolve(root,'../../../site');
const config=JSON.parse(fs.readFileSync(path.join(root,'src-tauri/tauri.showcase.conf.json'),'utf8'));
const version=config.version;
if(!/^\d+\.\d+\.\d+$/.test(version)||config.identifier!=='com.droidfun.workshop.private')throw new Error('Invalid Workshop release configuration');
const key=process.env.WORKSHOP_UPDATER_SIGNING_KEY_PATH||path.join(os.homedir(),'.config/droidfun-workshop-signing/updater.key');
if(!fs.existsSync(key))throw new Error('Workshop signing key is unavailable');
const output=path.join(input,'prepared');fs.mkdirSync(output,{recursive:true});
const run=(file,args,extra={})=>execFileSync(file,args,{cwd:root,stdio:'pipe',...extra});
const mac=path.join(input,'mac',`DroidFun-Workshop-${version}-macOS.zip`);
const windows=path.join(input,'windows',`DroidFun-Workshop-${version}-Windows.exe`);
if(!fs.existsSync(mac)||!fs.existsSync(windows))throw new Error('Both version-matching CI packages are required');
const extracted=path.join(output,'unpacked');fs.mkdirSync(extracted,{recursive:true});
run('/usr/bin/ditto',['-x','-k',mac,extracted]);
const app=path.join(extracted,'DroidFun Workshop.app');
run('/usr/bin/codesign',['--verify','--deep','--strict',app]);
const plist=path.join(app,'Contents/Info.plist');
const readPlist=field=>run('/usr/libexec/PlistBuddy',['-c','Print :'+field,plist]).toString().trim();
if(readPlist('CFBundleIdentifier')!==config.identifier||readPlist('CFBundleShortVersionString')!==version)throw new Error('Mac package identity/version mismatch');
const architectures=run('/usr/bin/lipo',['-archs',path.join(app,'Contents/MacOS/droidfun-workshop')]).toString().trim().split(/\s+/);
if(!['arm64','x86_64'].every(a=>architectures.includes(a)))throw new Error('Mac package must support Intel and Apple Silicon');
run(path.join(app,'Contents/MacOS/picotool'),['version']);
const archive=path.join(output,`DroidFun-Workshop-${version}-macOS.app.tar.gz`);
run('/usr/bin/tar',['-czf',archive,'-C',extracted,path.basename(app)],{env:{...process.env,COPYFILE_DISABLE:'1'}});
const signedWindows=path.join(output,path.basename(windows));fs.copyFileSync(windows,signedWindows);
const verification=[];
for(const file of [archive,signedWindows]){
 run(process.execPath,[path.join(root,'node_modules/@tauri-apps/cli/tauri.js'),'signer','sign','--app-version',version,'--private-key-path',key,'--password','',file]);
 const sig=fs.readFileSync(file+'.sig','utf8').trim();
 verification.push({file:path.basename(file),...verifySignature(fs.readFileSync(file),sig,config.plugins.updater.pubkey,version,path.basename(file))});
}
const assets=path.join(site,'public/download-assets');
const packageInfo=(id,platform,file)=>{
 const bytes=fs.readFileSync(file),sha256=crypto.createHash('sha256').update(bytes).digest('hex'),extension=path.extname(file);
 const key=path.basename(file,extension)+'-'+sha256.slice(0,12)+extension;fs.copyFileSync(file,path.join(assets,key));
 return {id,platform,version,filename:path.basename(file),key,size:bytes.length,sha256};
};
const packages=[packageInfo('mac','Mac',mac),packageInfo('windows','Windows',windows)];
const updaterMac=packageInfo('mac','Mac',archive);
const updaterPackages=[{...updaterMac,targets:['darwin-aarch64','darwin-x86_64'],signature:verification[0].signature},{...packages[1],targets:['windows-x86_64'],signature:verification[1].signature}];
const downloads=path.join(site,'src/studio-downloads.js');const source=fs.readFileSync(downloads,'utf8');
fs.writeFileSync(downloads,'// Packages are private Worker assets, never catalog entries or public links.\nexport const studioPackages = '+JSON.stringify(packages,null,2)+';\n'+source.slice(source.indexOf('export async function studioDownloads')));
const updates=path.join(site,'src/workshop-updates.js');let feed=fs.readFileSync(updates,'utf8');
const release={version,pub_date:new Date().toISOString(),notes:'Smoother scrolling text and screen previews, with refresh timing matched to your droid. Live text keeps scrolling while backend status is checked.'};
feed=feed.replace(/export const workshopUpdateRelease=[\s\S]*?;\n/,'export const workshopUpdateRelease='+JSON.stringify(release)+';\n').replace(/export const workshopUpdatePackages=[\s\S]*?;\n/,'export const workshopUpdatePackages='+JSON.stringify(updaterPackages,null,2)+';\n');
fs.writeFileSync(updates,feed);
const evidence={version,packages,updaterPackages,signatures:verification,macArchitectures:architectures};
fs.writeFileSync(path.join(output,'release-evidence.json'),JSON.stringify(evidence,null,2)+'\n');
console.log(`Prepared and signature-verified Workshop ${version} for Mac and Windows. Deploy the site to publish.`);
