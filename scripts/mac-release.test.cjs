const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const os=require('node:os');
const path=require('node:path');
const {macSigning,requirePublicMac}=require('./mac-release-policy.cjs');
const {verifyCode,verifyApp,treeDigest,detachVerificationMount,finishContainer}=require('./mac-trust.cjs');
const {macPackagingArgs}=require('./mac-helper.cjs');
// Placeholder values only; these are not credentials.
const owner={APPLE_SIGNING_IDENTITY:'Developer ID Application: Test Owner (ABCDE12345)',APPLE_TEAM_ID:'ABCDE12345',APPLE_ID:'owner@example.invalid',APPLE_PASSWORD:'placeholder',APPLE_CERTIFICATE:'placeholder',APPLE_CERTIFICATE_PASSWORD:'placeholder'};
const trusted={signingMode:'developer-id',distribution:'public',architectures:['arm64','x86_64'],codeSignatureVerified:true,developerIdSigned:true,notarized:true,stapled:true,gatekeeperAccepted:true,helperSigned:true,librarySigned:true,helperRuntimeVerified:true,verifiedContainers:['dmg','zip','updater']};
test('public mode is the default and refuses missing or ad-hoc identities before building',()=>{
  assert.throws(()=>macSigning([],{}),/Developer ID Application/);
  assert.throws(()=>macSigning([],{...owner,APPLE_SIGNING_IDENTITY:'-'}),/Developer ID Application/);
  assert.throws(()=>macSigning([],{...owner,APPLE_SIGNING_IDENTITY:'Apple Development: Test Owner (ABCDE12345)'}),/Developer ID Application/);
  assert.equal(macSigning([],owner).mode,'developer-id');
  assert.throws(()=>macSigning(['--mac-signing','unknown'],owner),/Use --mac-signing/);
  assert.throws(()=>macSigning(['--mac-signing'],owner),/Use --mac-signing/);
  assert.throws(()=>macSigning(['--mac-signing','developer-id','--mac-signing','internal-adhoc'],owner),/Choose one/);
});
test('public preflight requires matching team, CI certificate, and one complete notary method',()=>{
  assert.throws(()=>macSigning([],{...owner,APPLE_TEAM_ID:'WRONG12345'}),/must match/);
  assert.throws(()=>macSigning([],{...owner,APPLE_CERTIFICATE:''},{ci:true}),/CI signing/);
  assert.throws(()=>macSigning([],{...owner,APPLE_PASSWORD:''}),/notarization credentials/);
  assert.throws(()=>macSigning([],{...owner,APPLE_API_KEY:'key-id'}),/one notarization/);
  const api={...owner,APPLE_ID:'',APPLE_PASSWORD:'',APPLE_API_KEY:'key-id',APPLE_API_ISSUER:'issuer',APPLE_API_KEY_PATH:'/fixture/key.p8'};
  assert.throws(()=>macSigning([],api,{exists:()=>false}),/key file/);
  assert.equal(macSigning([],api,{exists:()=>true}).mode,'developer-id');
  const local={...owner};delete local.APPLE_CERTIFICATE;delete local.APPLE_CERTIFICATE_PASSWORD;
  assert.equal(macSigning([],local,{ci:false}).identity,owner.APPLE_SIGNING_IDENTITY);
});
test('internal builds require explicit mode, remove signing credentials, and cannot make updater assets',()=>{
  const result=macSigning(['--mac-signing','internal-adhoc'],{...owner,TAURI_SIGNING_PRIVATE_KEY:'placeholder',PATH:'/usr/bin'});
  assert.equal(result.identity,'-');assert.equal(result.mode,'internal-adhoc');assert.deepEqual(result.env,{PATH:'/usr/bin'});
  for(const flag of ['--updater','--signed'])assert.throws(()=>macSigning(['--mac-signing','internal-adhoc',flag],owner),/cannot create public/);
});
test('public publishing rejects absent trust checks and internal or historical ad-hoc evidence',()=>{
  assert.doesNotThrow(()=>requirePublicMac(trusted));
  for(const field of ['codeSignatureVerified','developerIdSigned','notarized','stapled','gatekeeperAccepted','helperSigned','librarySigned','helperRuntimeVerified'])assert.throws(()=>requirePublicMac({...trusted,[field]:false}),/Public Mac distribution/);
  for(const field of ['signingMode','distribution','architectures','verifiedContainers'])assert.throws(()=>requirePublicMac({...trusted,[field]:undefined}),/Public Mac distribution/);
  assert.throws(()=>requirePublicMac({...trusted,signingMode:'internal-adhoc'}),/Internal ad-hoc/);
  for(const kind of trusted.verifiedContainers)assert.throws(()=>requirePublicMac({...trusted,verifiedContainers:trusted.verifiedContainers.filter(x=>x!==kind)}),/Public Mac distribution/);
});
test('code inspection requires Developer ID, matching team, timestamp and hardened executables',()=>{
  const details='CodeDirectory=v1 flags=0x10000(runtime)\nAuthority=Developer ID Application: Test Owner (ABCDE12345)\nTeamIdentifier=ABCDE12345\nTimestamp=Oct 29 2026\n';
  assert.doesNotThrow(()=>verifyCode(details,{teamId:'ABCDE12345',executable:true}));
  for(const changed of [details.replace('Developer ID Application','Apple Development'),details.replace('TeamIdentifier=ABCDE12345','TeamIdentifier=WRONG12345'),details.replace('Timestamp=Oct 29 2026\n',''),details+'Signature=adhoc\n'])assert.throws(()=>verifyCode(changed,{teamId:'ABCDE12345',executable:true}),/Developer ID/);
  const dylib=details.replace('(runtime)','()');
  assert.throws(()=>verifyCode(dylib,{teamId:'ABCDE12345',executable:true}),/hardened runtime/);
  assert.doesNotThrow(()=>verifyCode(dylib,{teamId:'ABCDE12345',executable:false}));
});
test('container equality detects changed bytes, modes, missing files and symlink targets',t=>{
  const base=fs.mkdtempSync(path.join(os.tmpdir(),'droidfun-mac-tree-'));t.after(()=>fs.rmSync(base,{recursive:true,force:true}));
  const a=path.join(base,'a'),b=path.join(base,'b');fs.mkdirSync(a);fs.writeFileSync(path.join(a,'executable'),'original',{mode:0o755});fs.symlinkSync('executable',path.join(a,'link'));fs.cpSync(a,b,{recursive:true,verbatimSymlinks:true});
  assert.equal(treeDigest(a),treeDigest(b));
  fs.chmodSync(path.join(b,'executable'),0o644);assert.notEqual(treeDigest(a),treeDigest(b));
  fs.chmodSync(path.join(b,'executable'),0o755);fs.writeFileSync(path.join(b,'executable'),'changed');assert.notEqual(treeDigest(a),treeDigest(b));
  fs.writeFileSync(path.join(b,'executable'),'original');fs.unlinkSync(path.join(b,'link'));fs.symlinkSync('elsewhere',path.join(b,'link'));assert.notEqual(treeDigest(a),treeDigest(b));
});
test('direct Tauri Mac packaging stages the helper while preserving explicit configuration',()=>{
  const calls=[],stage=(out,target)=>{calls.push([out,target]);return {externalBin:['/fixture/picotool'],frameworks:['/fixture/libusb.dylib']};};
  const options={platform:'darwin',arch:'arm64',stage};
  const original=['build','--target','universal-apple-darwin','--config','{"bundle":{"macOS":{"signingIdentity":"explicit"}}}'];
  const args=macPackagingArgs(original,'/fixture/target',{},options);
  assert.deepEqual(calls,[['/fixture/target','universal-apple-darwin']]);
  assert.deepEqual(args.slice(3),original.slice(1));
  const defaults=JSON.parse(args[2]);assert.equal(defaults.bundle.macOS.signingIdentity,'-');assert.equal(defaults.bundle.macOS.hardenedRuntime,false);assert.deepEqual(defaults.bundle.externalBin,['/fixture/picotool']);
  macPackagingArgs(['bundle'],'/fixture/target',{},options);assert.equal(calls.at(-1)[1],'aarch64-apple-darwin');
  macPackagingArgs(['bundle','--target=x86_64-apple-darwin'],'/fixture/target',{},options);assert.equal(calls.at(-1)[1],'x86_64-apple-darwin');
  const before=calls.length;
  for(const input of [['dev'],['build','--help']])assert.deepEqual(macPackagingArgs(input,'/fixture/target',{},options),input);
  assert.deepEqual(macPackagingArgs(original,'/fixture/target',{DROIDFUN_MAC_HELPER_STAGED:'1'},options),original);
  assert.deepEqual(macPackagingArgs(original,'/fixture/target',{},{...options,platform:'linux'}),original);
  assert.equal(calls.length,before);
  const signed=macPackagingArgs(['build'],'/fixture/target',{APPLE_SIGNING_IDENTITY:owner.APPLE_SIGNING_IDENTITY},options);
  assert.equal(JSON.parse(signed[2]).bundle.macOS.hardenedRuntime,true);
});
test('bundle verification executes only helper version and rejects a library-load failure',t=>{
  const app=fs.mkdtempSync(path.join(os.tmpdir(),'droidfun-helper-runtime-'));t.after(()=>fs.rmSync(app,{recursive:true,force:true}));
  const helper=path.join(app,'Contents/MacOS/picotool'),library=path.join(app,'Contents/Frameworks/libusb-1.0.0.dylib');
  for(const file of [helper,library]){fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,'fixture');}
  const calls=[];
  const command=(exe,args)=>{calls.push([exe,args]);if(exe===helper)return 'picotool v2.0.0 (MacOS, AppleClang-17.0.0, Release)';if(exe==='/usr/bin/otool')return '@executable_path/../Frameworks/libusb-1.0.0.dylib';return '';};
  assert.equal(verifyApp(app,{mode:'internal-adhoc'},command).helperRuntimeVerified,true);
  assert.deepEqual(calls.filter(([exe])=>exe===helper),[[helper,['version']]]);
  assert.throws(()=>verifyApp(app,{mode:'internal-adhoc'},(exe,args)=>{if(exe===helper)throw new Error('dyld: library validation failure');return command(exe,args);}),/library validation failure/);
});
test('verification mount cleanup retries transient busy errors without forcing detach',()=>{
  const calls=[],waits=[],mount='/fixture/verification/mounted';
  detachVerificationMount(mount,{command:(exe,args)=>{calls.push([exe,args]);if(calls.length<3)throw new Error('Resource busy');},pause:ms=>waits.push(ms)});
  assert.equal(calls.length,3);assert.deepEqual(waits,[1000,1000]);
  assert.ok(calls.every(([exe,args])=>exe==='/usr/bin/hdiutil'&&JSON.stringify(args)===JSON.stringify(['detach',mount])));
  let failedCalls=0;
  assert.throws(()=>detachVerificationMount(mount,{command:()=>{failedCalls++;throw new Error('Resource busy');},pause:()=>{}}),/after three attempts/);
  assert.equal(failedCalls,3);
});
test('cleanup preserves the original verification failure and never removes a mounted image',()=>{
  const original=new Error('The packaged app failed its trust check.'),cleanup=new Error('Resource busy');
  let removed=false;const warnings=[];
  assert.throws(()=>finishContainer('/fixture',true,original,{detach:()=>{throw cleanup;},remove:()=>{removed=true;},warn:m=>warnings.push(m)}),error=>error===original&&error.cleanupError===cleanup);
  assert.equal(removed,false);assert.equal(warnings.length,1);
  assert.throws(()=>finishContainer('/fixture',true,undefined,{detach:()=>{throw cleanup;},remove:()=>{removed=true;}}),error=>error===cleanup);
  assert.equal(removed,false);
  const order=[];finishContainer('/fixture',true,undefined,{detach:()=>order.push('detach'),remove:()=>order.push('remove')});assert.deepEqual(order,['detach','remove']);
});
