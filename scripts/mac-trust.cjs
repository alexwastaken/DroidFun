// Read-only verification of built code and extracted distribution containers.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const {spawnSync} = require('node:child_process');
function run(command,args,options={}) {
  const result=spawnSync(command,args,{encoding:'utf8',maxBuffer:8*1024*1024,...options});
  if (result.error || result.status !== 0) throw new Error(`${path.basename(command)} failed: ${(result.stderr || result.stdout || result.error?.message || '').trim()}`);
  return `${result.stdout || ''}${result.stderr || ''}`.trim();
}
function verifyCode(text,{teamId,executable=false,developerId=true}={}) {
  if (!developerId) return;
  if (!text.includes('Authority=Developer ID Application:') || !text.includes(`TeamIdentifier=${teamId}\n`) || /Signature=adhoc/.test(text) || !/^Timestamp=.+/m.test(text)) throw new Error('Code is missing its expected Developer ID signature, team or secure timestamp.');
  if (executable && !/^CodeDirectory=.*\bruntime\b/m.test(text)) throw new Error('An executable is missing hardened runtime.');
}
function verifyApp(app,{mode,teamId},command=run) {
  command('/usr/bin/codesign',['--verify','--deep','--strict',app]);
  const publicBuild=mode==='developer-id';
  const targets=[['app',app,true],['helper',path.join(app,'Contents/MacOS/picotool'),true],['library',path.join(app,'Contents/Frameworks/libusb-1.0.0.dylib'),false]];
  for (const [label,file,executable] of targets) {
    if (!fs.existsSync(file)) throw new Error(`The signed ${label} is missing from its expected bundle location.`);
    command('/usr/bin/codesign',['--verify','--strict',file]);
    verifyCode(command('/usr/bin/codesign',['-dvv',file])+'\n',{teamId,executable,developerId:publicBuild});
  }
  const dependency=command('/usr/bin/otool',['-L',targets[1][1]]);
  if (!dependency.includes('@executable_path/../Frameworks/libusb-1.0.0.dylib') || /\/opt\/homebrew\/|\/usr\/local\//.test(dependency)) throw new Error('The USB helper must use its bundled signed library.');
  // `version` does not enumerate or open USB. It does load the actual dylib,
  // catching library-validation failures that static signature checks miss.
  const helperVersion=command(targets[1][1],['version']);
  if(!/^picotool v2\.0\.0\b/m.test(helperVersion))throw new Error('The bundled USB helper did not report its expected version.');
  if (publicBuild) {
    command('/usr/bin/xcrun',['stapler','validate',app]);
    command('/usr/sbin/spctl',['--assess','--type','execute','--verbose=2',app]);
  }
  return {codeSignatureVerified:true,developerIdSigned:publicBuild,notarized:publicBuild,stapled:publicBuild,gatekeeperAccepted:publicBuild,helperSigned:publicBuild,librarySigned:publicBuild,helperRuntimeVerified:true};
}
function treeDigest(root) {
  const output=[];
  function walk(directory) {
    for(const entry of fs.readdirSync(directory,{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))) {
      const full=path.join(directory,entry.name),name=path.relative(root,full),st=fs.lstatSync(full);
      if(entry.isDirectory()) walk(full);
      else if(entry.isSymbolicLink()) output.push([name,'link',fs.readlinkSync(full)]);
      else if(entry.isFile()) output.push([name,st.mode&0o777,crypto.createHash('sha256').update(fs.readFileSync(full)).digest('hex')]);
      else throw new Error('Unexpected special file in application bundle.');
    }
  }
  walk(root);return JSON.stringify(output);
}
function detachVerificationMount(mount,{command=run,pause=ms=>Atomics.wait(new Int32Array(new SharedArrayBuffer(4)),0,0,ms)}={}) {
  let failure;
  for(let attempt=0;attempt<3;attempt++) {
    try { command('/usr/bin/hdiutil',['detach',mount]);return; }
    catch(error) { failure=error; }
    if(attempt<2)pause(1000);
  }
  throw new Error(`Could not detach the verification disk image after three attempts; its temporary mount remains at ${mount}. ${failure.message}`,{cause:failure});
}
function finishContainer(temp,mounted,verificationError,{detach=detachVerificationMount,remove=directory=>fs.rmSync(directory,{recursive:true,force:true}),warn=message=>console.warn(message)}={}) {
  let cleanupError;
  try {
    if(mounted)detach(path.join(temp,'mounted'));
    // Never recursively remove a directory while its disk image is mounted.
    remove(temp);
  } catch(error) { cleanupError=error; }
  if(verificationError) {
    if(cleanupError){verificationError.cleanupError=cleanupError;warn(`Verification cleanup also failed: ${cleanupError.message}`);}
    throw verificationError;
  }
  if(cleanupError)throw cleanupError;
}
function verifyContainer(container,kind,original,signing) {
  const temp=fs.mkdtempSync(path.join(os.tmpdir(),'droidfun-container-'));
  let mounted=false,verificationError;
  try {
    let extracted;
    if(kind==='dmg') {
      const mount=path.join(temp,'mounted');fs.mkdirSync(mount);
      run('/usr/bin/hdiutil',['verify',container]);
      run('/usr/bin/hdiutil',['attach','-readonly','-nobrowse','-noautoopen','-mountpoint',mount,container]);mounted=true;
      extracted=path.join(mount,path.basename(original));
    } else {
      if(kind==='zip') run('/usr/bin/ditto',['-x','-k',container,temp]);
      else if(kind==='updater') run('/usr/bin/tar',['-xzf',container,'-C',temp]);
      else throw new Error('Unsupported Mac package container.');
      extracted=path.join(temp,path.basename(original));
    }
    if(treeDigest(extracted)!==treeDigest(original)) throw new Error(`${kind} contains a different app from the verified signed bundle.`);
    verifyApp(extracted,signing);
  } catch(error) { verificationError=error; }
  finishContainer(temp,mounted,verificationError);
}
module.exports={run,verifyCode,verifyApp,treeDigest,verifyContainer,detachVerificationMount,finishContainer};
