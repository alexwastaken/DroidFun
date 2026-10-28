import { useEffect, useRef, useState } from "react";
import { monochrome, readImage, type ImageOptions } from "./image-import";
export default function ImageUpload({hasImage,onChange,onName,onBusy}: {hasImage:boolean;onChange:(pixels:string)=>void;onName:(name:string)=>void;onBusy:(busy:boolean)=>void}) {
 const bitmap=useRef<ImageBitmap|null>(null),job=useRef(0),alive=useRef(true);
 const [loaded,setLoaded]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState("");
 const [options,setOptions]=useState<ImageOptions>({fit:"fit",background:"auto",threshold:128,dither:true});
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;job.current++;bitmap.current?.close();};},[]);
 async function choose(file?:File){
  if(!file)return;const id=++job.current;setBusy(true);onBusy(true);setError("");
  try{const image=await readImage(file);if(!alive.current||id!==job.current){image.close();return;}let pixels:string;try{pixels=monochrome(image,options);}catch(e){image.close();throw e;}bitmap.current?.close();bitmap.current=image;setLoaded(true);onChange(pixels);onName(file.name.replace(/\.[^.]+$/,"").slice(0,24)||"My image");}
  catch(e){if(alive.current&&id===job.current)setError(String(e instanceof Error?e.message:e));}
  finally{if(alive.current&&id===job.current){setBusy(false);onBusy(false);}}
 }
 function adjust(patch:Partial<ImageOptions>){const next={...options,...patch};setOptions(next);if(bitmap.current){try{onChange(monochrome(bitmap.current,next));}catch(e){setError(String(e));}}}
 return <section className="image-upload" aria-label="Upload an image">
  <label>{hasImage?"Replace image":"Choose an image"}<input type="file" accept="image/png,image/jpeg,image/webp" disabled={busy} onChange={e=>{void choose(e.target.files?.[0]);e.target.value="";}}/></label>
  <p>PNG, JPG or still WebP · up to 2 MB. Converted to the screen’s black-and-white pixels. Nothing is uploaded to a website.</p>
  {busy&&<p role="status">Preparing your image…</p>}{error&&<p className="source-error" role="alert">{error}</p>}
  {loaded&&<><label>Image background<select value={options.background} onChange={e=>adjust({background:e.target.value as ImageOptions["background"]})}><option value="auto">Automatic</option><option value="dark">Dark</option><option value="light">Light</option></select></label><label>Image fit<select value={options.fit} onChange={e=>adjust({fit:e.target.value as ImageOptions["fit"]})}><option value="fit">Fit whole image</option><option value="fill">Fill screen · crop edges</option></select></label><label>Black-and-white threshold<input type="range" min={1} max={254} value={options.threshold} onChange={e=>adjust({threshold:Number(e.target.value)})}/></label><label className="inline-check"><input type="checkbox" checked={options.dither} onChange={e=>adjust({dither:e.target.checked})}/>Keep shading with small dots</label></>}
  {!loaded&&hasImage&&<p>Your saved image is ready. To change its fit or shading, choose the original file again.</p>}
 </section>;
}
