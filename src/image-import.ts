// Only the 128 × 64 monochrome bitmap is saved. The original file stays local.
export type ImageOptions = { fit: "fit" | "fill"; threshold: number; dither: boolean };
const MAX_PIXELS = 4_000_000;
export function imageDimensions(bytes: Uint8Array): {width: number; height: number; type: string} {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const ascii = (start: number, count: number) => String.fromCharCode(...bytes.slice(start, start + count));
  let width = 0, height = 0, type = "";
  if (bytes.length >= 24 && bytes[0] === 137 && ascii(1,7) === "PNG\r\n\x1a\n" && ascii(12,4) === "IHDR") {
    width=v.getUint32(16);height=v.getUint32(20);type="image/png";
  } else if (bytes.length >= 4 && bytes[0] === 255 && bytes[1] === 216) {
    let i=2;
    while(i+3<bytes.length){
      if(bytes[i++]!==255)break;
      while(bytes[i]===255)i++;
      const marker=bytes[i++];if(marker===0xda||marker===0xd9)break;
      if(marker===0x01||(marker>=0xd0&&marker<=0xd7))continue;
      if(i+2>bytes.length)break;
      const length=v.getUint16(i);if(length<2||i+length>bytes.length)break;
      if([0xc0,0xc1,0xc2].includes(marker)&&length>=8){height=v.getUint16(i+3);width=v.getUint16(i+5);type="image/jpeg";break;}
      i+=length;
    }
  } else if(bytes.length>=30&&ascii(0,4)==="RIFF"&&ascii(8,4)==="WEBP"){
    type="image/webp";
    const chunk=ascii(12,4);
    if(chunk==="VP8X"){
      if(bytes[20]&2)throw Error("Choose a still PNG, JPG or WebP image; animated images are not supported.");
      width=1+bytes[24]+(bytes[25]<<8)+(bytes[26]<<16);height=1+bytes[27]+(bytes[28]<<8)+(bytes[29]<<16);
    }else if(chunk==="VP8L"&&bytes[20]===0x2f){const bits=v.getUint32(21,true);width=(bits&0x3fff)+1;height=((bits>>>14)&0x3fff)+1;}
    else if(chunk==="VP8 "&&bytes[23]===0x9d&&bytes[24]===1&&bytes[25]===0x2a){width=v.getUint16(26,true)&0x3fff;height=v.getUint16(28,true)&0x3fff;}
  }
  if(!width||!height)throw Error("Choose a valid PNG, JPG or still WebP image.");
  if(width>4096||height>4096||width*height>MAX_PIXELS)throw Error("Choose a smaller image: at most 4096 pixels per side and 4 megapixels.");
  return {width,height,type};
}
export async function readImage(file: File): Promise<ImageBitmap> {
  if(file.size>2*1024*1024)throw Error("Choose an image smaller than 2 MB.");
  const buffer=await file.arrayBuffer(),info=imageDimensions(new Uint8Array(buffer));
  let bitmap: ImageBitmap;
  try{bitmap=await createImageBitmap(new Blob([buffer],{type:info.type}));}catch{throw Error("This image could not be opened. Try saving it as PNG or JPG.");}
  if(bitmap.width>4096||bitmap.height>4096||bitmap.width*bitmap.height>MAX_PIXELS){bitmap.close();throw Error("The decoded image is too large. Choose a smaller image.");}
  return bitmap;
}
export function monochrome(bitmap: ImageBitmap, options: ImageOptions): string {
  const canvas=document.createElement("canvas");canvas.width=128;canvas.height=64;
  const ctx=canvas.getContext("2d",{willReadFrequently:true});if(!ctx)throw Error("Image preview is unavailable.");
  ctx.fillStyle="#000";ctx.fillRect(0,0,128,64);
  const scale=(options.fit==="fill"?Math.max:Math.min)(128/bitmap.width,64/bitmap.height);
  const w=bitmap.width*scale,h=bitmap.height*scale;ctx.imageSmoothingEnabled=true;ctx.imageSmoothingQuality="high";ctx.drawImage(bitmap,(128-w)/2,(64-h)/2,w,h);
  const rgba=ctx.getImageData(0,0,128,64).data,gray=new Float32Array(8192),bytes=new Uint8Array(1024);
  for(let p=0;p<gray.length;p++)gray[p]=.2126*rgba[p*4]+.7152*rgba[p*4+1]+.0722*rgba[p*4+2];
  for(let y=0;y<64;y++)for(let x=0;x<128;x++){
    const p=y*128+x,lit=gray[p]>=options.threshold;if(lit)bytes[(y>>3)*128+x]|=1<<(y&7);
    if(options.dither){const error=gray[p]-(lit?255:0);if(x<127)gray[p+1]+=error*7/16;if(y<63){if(x>0)gray[p+127]+=error*3/16;gray[p+128]+=error*5/16;if(x<127)gray[p+129]+=error/16;}}
  }
  return Array.from(bytes,b=>b.toString(16).padStart(2,"0")).join("");
}
