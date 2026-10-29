import { useEffect, useRef, useState } from "react";
const models = ["boo", "claw", "meep", "nimbus"];

export default function DroidModel({model}: {model: string}) {
 const selected=models.includes(model)?model:"nimbus";
 const name=selected[0].toUpperCase()+selected.slice(1);
 const host=useRef<HTMLDivElement>(null);
 const controls=useRef<{setMotion:(playing:boolean)=>void;dispose:()=>void}|null>(null);
 const [ready,setReady]=useState(false);
 const [playing,setPlaying]=useState(()=>!matchMedia("(prefers-reduced-motion: reduce)").matches);
 useEffect(()=>{
  let active=true;setReady(false);
  import("./droid-viewer.js").then(({mountDroid})=>{if(active&&host.current)controls.current=mountDroid(host.current,selected+"-bot",value=>{if(active)setReady(value);},()=>{if(active)setPlaying(false);});}).catch(()=>{if(active)setReady(false);});
  const preference=matchMedia("(prefers-reduced-motion: reduce)");
  const changed=(event:MediaQueryListEvent)=>setPlaying(!event.matches);preference.addEventListener("change",changed);
  setPlaying(!preference.matches);
  return ()=>{active=false;controls.current?.dispose();controls.current=null;preference.removeEventListener("change",changed);};
 },[selected]);
 return <figure className="droid-model">
  <div ref={host} className="droid-model-surface" tabIndex={ready?0:-1} role="img" aria-label={ready?`${name} in 3D. Drag or use the arrow keys to rotate. Home faces forward.`:`${name} droid`}>
   <img className="droid-model-poster" src={`/droids/${selected}-bot.png`} alt={`${name} droid`} aria-hidden={ready} />
  </div>
  {ready&&<button type="button" className="droid-model-motion" aria-pressed={playing} onClick={()=>{controls.current?.setMotion(!playing);setPlaying(!playing);}}>{playing?"Pause motion":"Play motion"}</button>}
 </figure>;
}
