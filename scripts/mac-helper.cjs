// Stage copied USB code in Tauri's supported externalBin/framework locations.
const fs=require('node:fs');
const path=require('node:path');
const {run}=require('./mac-trust.cjs');
function stageMacHelper(output,target,{original=path.join(__dirname,'../src-tauri/resources/picotool-macos-universal'),command=run}={}) {
  const staging=path.join(output,'mac-helper-staging',target);fs.mkdirSync(staging,{recursive:true});
  const helper=path.join(staging,`picotool-${target}`),library=path.join(staging,'libusb-1.0.0.dylib');
  fs.copyFileSync(path.join(original,'picotool'),helper);fs.copyFileSync(path.join(original,'libusb-1.0.0.dylib'),library);
  fs.chmodSync(helper,0o755);fs.chmodSync(library,0o755);
  command('/usr/bin/install_name_tool',['-change','@executable_path/libusb-1.0.0.dylib','@executable_path/../Frameworks/libusb-1.0.0.dylib',helper]);
  command('/usr/bin/install_name_tool',['-id','@rpath/libusb-1.0.0.dylib',library]);
  if(target==='universal-apple-darwin') {
    // Tauri validates externalBin during each architecture's Rust build before
    // making the universal bundle. All three names can use this universal code.
    // Copy after patching so every path loads the signed Frameworks library.
    for(const arch of ['aarch64-apple-darwin','x86_64-apple-darwin']) {
      const archHelper=path.join(staging,`picotool-${arch}`);
      fs.copyFileSync(helper,archHelper);fs.chmodSync(archHelper,0o755);
    }
  }
  return {externalBin:[path.join(staging,'picotool')],frameworks:[library]};
}
function macPackagingArgs(args,output,env,{platform=process.platform,arch=process.arch,stage=stageMacHelper}={}) {
  if(platform!=='darwin'||!['build','bundle'].includes(args[0])||args.some(arg=>['--help','-h','--version','-V'].includes(arg))||env.DROIDFUN_MAC_HELPER_STAGED==='1')return args;
  const index=args.findIndex(arg=>arg==='--target'||arg==='-t');
  const target=index>=0?args[index+1]:args.find(arg=>arg.startsWith('--target='))?.slice(9)||(arch==='arm64'?'aarch64-apple-darwin':'x86_64-apple-darwin');
  if(!['universal-apple-darwin','aarch64-apple-darwin','x86_64-apple-darwin'].includes(target))throw new Error('Choose a supported native Mac packaging target.');
  const helper=stage(output,target);
  const identity=env.APPLE_SIGNING_IDENTITY||'-';
  const defaults={bundle:{externalBin:helper.externalBin,macOS:{frameworks:helper.frameworks,signingIdentity:identity,hardenedRuntime:identity!=='-'}}};
  // Earlier defaults preserve explicit later --config options, including the
  // release wrapper's strict signing policy. No Apple credentials are created.
  return [args[0],'--config',JSON.stringify(defaults),...args.slice(1)];
}
module.exports={stageMacHelper,macPackagingArgs};
