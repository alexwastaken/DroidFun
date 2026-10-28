import * as THREE from 'three';

// Storefront-only hardware, using the mounting datums in the source CAD.
// M3 socket heads: 5.5 mm diameter, 3 mm tall, with a recessed 2.5 mm hex drive.
const finish=new THREE.MeshStandardMaterial({color:'#202124',metalness:.65,roughness:.42});
const headShape=new THREE.Shape();
headShape.absarc(0,0,2.75,0,Math.PI*2,false);
const socket=new THREE.Path(),radius=2.5/Math.sqrt(3);
for(let i=0;i<6;i++){
 const angle=-i*Math.PI/3,x=Math.cos(angle)*radius,y=Math.sin(angle)*radius;
 if(i===0)socket.moveTo(x,y);else socket.lineTo(x,y);
}
socket.closePath();headShape.holes.push(socket);
const rim=new THREE.ExtrudeGeometry(headShape,{depth:1.4,bevelEnabled:false,curveSegments:24});
const base=new THREE.CylinderGeometry(2.75,2.75,1.6,48);
base.rotateX(Math.PI/2);base.translate(0,0,.8);

function screw(model,label,position,direction,length,headHeight=3){
 const group=new THREE.Group();group.name='hardware_socket_screw_'+label;
 const lower=new THREE.Mesh(base,finish),upper=new THREE.Mesh(rim,finish);
 const headScale=headHeight/3;
 lower.scale.z=headScale;upper.scale.z=headScale;
 upper.position.z=1.6*headScale;group.add(lower,upper);
 const shaftGeometry=new THREE.CylinderGeometry(1.5,1.5,length,20);
 shaftGeometry.rotateX(Math.PI/2);shaftGeometry.translate(0,0,-length/2);
 group.add(new THREE.Mesh(shaftGeometry,finish));
 group.position.set(...position);
 group.quaternion.setFromUnitVectors(new THREE.Vector3(0,0,1),new THREE.Vector3(...direction));
 group.traverse(mesh=>{if(mesh.isMesh){mesh.castShadow=true;mesh.receiveShadow=false;}});
 model.add(group);
}

function roundedPort(width,height,radius,clockwise=false){
 const path=clockwise?new THREE.Path():new THREE.Shape();
 const x=width/2-radius,y=height/2-radius;
 path.absarc(x,y,radius,0,Math.PI/2,false);
 path.absarc(-x,y,radius,Math.PI/2,Math.PI,false);
 path.absarc(-x,-y,radius,Math.PI,Math.PI*1.5,false);
 path.absarc(x,-y,radius,Math.PI*1.5,Math.PI*2,false);
 path.closePath();
 if(clockwise){const points=path.getPoints(12).reverse();return new THREE.Path(points);}
 return path;
}

function clawUsbPort(model){
 // The CAD electronics use a solid placeholder for the connector. Replace it
 // in the storefront with a hollow receptacle at the same USB-C datum.
 const original=model.getObjectByName('elec_usb_connector');
 if(original)original.visible=false;
 const group=new THREE.Group();group.name='hardware_usb_c_receptacle';
 group.position.set(0,22.02,16.5695215);group.rotation.x=Math.PI/2;
 const outline=roundedPort(8.94,3.16,1.28);
 outline.holes.push(roundedPort(8.04,2.26,.95,true));
 const shell=new THREE.Mesh(new THREE.ExtrudeGeometry(outline,{depth:2.4,bevelEnabled:false,curveSegments:16}),new THREE.MeshStandardMaterial({color:'#b6bcc3',metalness:.8,roughness:.32}));
 shell.name='hardware_usb_c_metal_rim';group.add(shell);
 const cavity=new THREE.Mesh(new THREE.ShapeGeometry(roundedPort(8.04,2.26,.95)),new THREE.MeshBasicMaterial({color:'#11151b',side:THREE.DoubleSide,toneMapped:false}));
 cavity.position.z=.9;cavity.name='hardware_usb_c_dark_interior';group.add(cavity);
 const tongue=new THREE.Mesh(new THREE.BoxGeometry(6.3,.5,.3),new THREE.MeshStandardMaterial({color:'#303640',roughness:.65}));
 tongue.position.z=.48;tongue.name='hardware_usb_c_center_tongue';group.add(tongue);
 model.add(group);
}

export function addPreviewHardware(model,name){
 if(name==='claw-bot'){
  clawUsbPort(model);
  // Recess the display-only low-profile heads into the rear panel. Their
  // outer faces sit 0.2 mm above its y=21.9 surface, retaining the hex sockets.
  for(const x of [-18.5,18.5])for(const z of [17,41])screw(model,`rear_${x}_${z}`,[x,21.3,z],[0,1,0],10,.8);
 }else if(name==='meep-bot'){
  // Orbit Scout lifts the complete head assembly 12 mm above its local CAD datum.
  for(const [x,z] of [[-13,31],[13,52]])screw(model,`rear_${x}`,[x,11.6,z+12],[0,1,0],20);
  for(const x of [-16,16])screw(model,`base_${x}`,[x,-7,3.2],[0,0,-1],8);
 }
}
