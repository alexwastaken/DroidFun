import { useEffect, useRef, useState } from "react";

export default function Oled({ frame, brightness, orientation, description }: { frame: number[]; brightness: number; orientation: number; description: string }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const shell = useRef<HTMLDivElement>(null);
  const [pixelScale,setPixelScale] = useState(2);
  useEffect(() => {
    const parent=shell.current?.parentElement;if(!parent)return;
    const resize=() => {const style=getComputedStyle(parent);const available=parent.clientWidth-parseFloat(style.paddingLeft)-parseFloat(style.paddingRight);setPixelScale(Math.max(1,Math.min(2,Math.floor(available/128))));};
    const observer=new ResizeObserver(resize);observer.observe(parent);resize();return () => observer.disconnect();
  },[]);
  useEffect(() => {
    const context = canvas.current?.getContext("2d");
    if (!context) return;
    const pixels = context.createImageData(128, 64);
    for (let y = 0; y < 64; y++) for (let x = 0; x < 128; x++) {
      const lit = (frame[(y >> 3) * 128 + x] & (1 << (y & 7))) !== 0;
      const i = (y * 128 + x) * 4;
      pixels.data[i] = lit ? 91 : 4;
      pixels.data[i + 1] = lit ? 189 : 17;
      pixels.data[i + 2] = lit ? 252 : 28;
      pixels.data[i + 3] = 255;
    }
    context.putImageData(pixels, 0, 0);
  }, [frame]);
  return <div ref={shell} className="oled" style={{ width: pixelScale*128, opacity: 0.3 + brightness * 0.007 }}><canvas ref={canvas} width={128} height={64} role="img" aria-label={description} style={{transform: `rotate(${orientation}deg)`}} /></div>;
}

