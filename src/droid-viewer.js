import * as THREE from 'three';
import {GLTFLoader} from 'three/addons/loaders/GLTFLoader.js';
import {toCreasedNormals} from 'three/addons/utils/BufferGeometryUtils.js';
import {addPreviewHardware} from './preview-hardware.js';
// The same geometry, finishes and hardware as droidfun.io, bundled for offline use.
const loader=new GLTFLoader(),models=new Map();
function modelFor(name){
 if(!models.has(name))models.set(name,loader.loadAsync(`/droids/${name}.glb`).then(gltf=>{
  const model=gltf.scene;
  model.traverse(mesh=>{
   if(!mesh.isMesh)return;
   const n=mesh.name.toLowerCase();
   const electronic=n.startsWith('elec_')||n.startsWith('wire_')||n.startsWith('hardware_');
   let color,screen=false;
   if(name==='claw-bot'){
    color=n.startsWith('eye')?'#17191c':n==='screen_pixels'?'#eaf8ff':n==='elec_oled_glass'?'#080a0c':electronic?n.includes('pcb')?'#165c72':'#8b8d90':'#f37120';
    screen=n==='screen_pixels';
   }else{
    screen=n==='screen_face'||n.startsWith('face_');
    color=screen?n==='screen_face'?'#070910':'#f7fffc':electronic?undefined:name==='boo-bot'?'#eee8dd':name==='nimbus-bot'?'#deeff7':'#78b82d';
   }
   if((name==='nimbus-bot'||name==='boo-bot')&&n==='acrylic_cover'){mesh.material=new THREE.MeshPhysicalMaterial({color:'#d5efff',transparent:true,opacity:.14,roughness:.08,depthWrite:false,side:THREE.DoubleSide});mesh.renderOrder=10;return;}
   if(!screen&&!electronic)mesh.geometry=toCreasedNormals(mesh.geometry,.55);
   if(color)mesh.material=screen?new THREE.MeshBasicMaterial({color,toneMapped:false,polygonOffset:true,polygonOffsetFactor:-1,polygonOffsetUnits:-2}):new THREE.MeshStandardMaterial({color,roughness:.5,metalness:0});
   mesh.castShadow=!screen;mesh.receiveShadow=false;
  });
  addPreviewHardware(model,name);
  const bounds=new THREE.Box3().setFromObject(model),center=bounds.getCenter(new THREE.Vector3());
  model.position.sub(center);
  const group=new THREE.Group();group.add(model);
  const size=bounds.getSize(new THREE.Vector3());
  group.scale.setScalar(1/Math.max(size.x,size.y,size.z));
  return group;
 }).catch(error=>{models.delete(name);throw error;}));
 return models.get(name);
}

export function mountDroid(host,name,onReady,onManual){
 let disposed=false,renderer=null,scene=null,camera=null,resize=null,frame=0,last=0,elapsed=0,angle=0,drag=null,visible=false,manual=false;
 const motion=matchMedia('(prefers-reduced-motion: reduce)');let paused=motion.matches;
 const draw=()=>{if(!renderer||!camera)return;camera.position.set(Math.sin(angle)*camera.userData.distance,-Math.cos(angle)*camera.userData.distance,camera.userData.distance*.23);camera.lookAt(0,0,0);renderer.render(scene,camera);};
 const tick=time=>{frame=0;if(disposed||paused||!visible||document.hidden||manual)return;elapsed+=last?Math.min((time-last)/1000,.06):0;last=time;angle=Math.PI/8*Math.sin(elapsed*Math.PI*2/16)*(1-Math.exp(-elapsed/2));draw();frame=requestAnimationFrame(tick);};
 const stop=()=>{cancelAnimationFrame(frame);frame=0;last=0;};
 const wake=()=>{if(!frame&&renderer&&!paused&&visible&&!document.hidden&&!manual)frame=requestAnimationFrame(tick);};
 const visibility=()=>{if(document.hidden)stop();else wake();};
 const reduced=event=>{paused=event.matches;stop();if(paused){angle=0;draw();}wake();};
 const observer=new IntersectionObserver(entries=>{visible=entries[0].isIntersecting;if(!visible)stop();else wake();});observer.observe(host);
 document.addEventListener('visibilitychange',visibility);motion.addEventListener('change',reduced);
 const down=e=>{if(e.button!==0||!renderer)return;manual=true;paused=true;onManual?.();stop();drag={id:e.pointerId,x:e.clientX,angle};host.setPointerCapture(e.pointerId);};
 const move=e=>{if(drag?.id===e.pointerId){angle=drag.angle-(e.clientX-drag.x)/Math.max(host.clientWidth,1)*Math.PI*2;draw();}};
 const up=e=>{if(drag?.id!==e.pointerId)return;drag=null;if(host.hasPointerCapture(e.pointerId))host.releasePointerCapture(e.pointerId);};
 const key=e=>{if(!['ArrowLeft','ArrowRight','Home'].includes(e.key))return;e.preventDefault();manual=true;paused=true;onManual?.();stop();angle=e.key==='Home'?0:angle+(e.key==='ArrowLeft'?1:-1)*Math.PI/12;draw();};
 host.addEventListener('pointerdown',down);host.addEventListener('pointermove',move);host.addEventListener('pointerup',up);host.addEventListener('pointercancel',up);host.addEventListener('keydown',key);
 const setMotion=value=>{paused=!value;manual=false;angle=0;elapsed=0;stop();draw();wake();};
 modelFor(name).then(template=>{
  if(disposed)return;
  try{
   renderer=new THREE.WebGLRenderer({alpha:true,antialias:true,powerPreference:'low-power'});renderer.setPixelRatio(Math.min(devicePixelRatio,1.5));renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.toneMapping=THREE.ACESFilmicToneMapping;renderer.toneMappingExposure=1.1;renderer.domElement.setAttribute('aria-hidden','true');host.append(renderer.domElement);
   scene=new THREE.Scene();scene.add(template.clone(true));camera=new THREE.PerspectiveCamera(30,1,.01,20);camera.up.set(0,0,1);
   scene.add(new THREE.HemisphereLight('#fffaf4','#9b9a92',2.3));const light=new THREE.DirectionalLight('#fff5e5',2.6);light.position.set(-1.5,-2,3);scene.add(light);const fill=new THREE.DirectionalLight('#e4eeff',1.4);fill.position.set(2,1,2);scene.add(fill);
   const size=()=>{const w=host.clientWidth,h=host.clientHeight;if(!w||!h||disposed)return;renderer.setSize(w,h,false);camera.aspect=w/h;camera.userData.distance=2.55/(Math.min(1,camera.aspect)*(['boo-bot','nimbus-bot'].includes(name)?.8:1));camera.updateProjectionMatrix();draw();};
   resize=new ResizeObserver(size);resize.observe(host);size();host.classList.add('has-3d');onReady(true);wake();
  }catch{renderer?.dispose();renderer?.domElement.remove();renderer=null;onReady(false);}
 }).catch(()=>{if(!disposed)onReady(false);});
 return {setMotion,dispose(){disposed=true;stop();observer.disconnect();resize?.disconnect();document.removeEventListener('visibilitychange',visibility);motion.removeEventListener('change',reduced);host.removeEventListener('pointerdown',down);host.removeEventListener('pointermove',move);host.removeEventListener('pointerup',up);host.removeEventListener('pointercancel',up);host.removeEventListener('keydown',key);renderer?.dispose();renderer?.forceContextLoss();renderer?.domElement.remove();host.classList.remove('has-3d');}};
}
