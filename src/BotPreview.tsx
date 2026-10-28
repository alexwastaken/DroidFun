import { useState, type ReactNode } from "react";
import "./bot-preview.css";

type BotModel = "boo" | "claw" | "meep" | "nimbus";
type BotProps = { model: string };

const names: Record<BotModel, string> = {
  boo: "Boo Bot",
  nimbus: "Nimbus Bot",
  claw: "Claw Bot",
  meep: "Meep Bot",
};

function botModel(model: string): BotModel {
  return model === "boo" || model === "claw" || model === "nimbus" ? model : "meep";
}

/** The housing is decorative; the real OLED canvas supplies its own accessible label. */
function Housing({ model, miniature = false }: { model: BotModel; miniature?: boolean }) {
  // Match each enclosure's screen-to-body width, rather than a shared tablet-sized display.
  const screen = {boo:{x:108.5,y:111,width:113},claw:{x:94.5,y:137,width:141},nimbus:{x:91,y:109,width:148},meep:{x:87.5,y:103,width:155}}[model];
  return <svg className="bot-preview__housing" viewBox="0 0 330 280" preserveAspectRatio="none" aria-hidden="true" focusable="false">
    <ellipse className="bot-preview__ground" cx="165" cy="274" rx={model === "boo" ? 125 : 105} ry="5" />
    {model === "boo" && <>
      <path className="bot-preview__body" d="M18 255V111C18 52 83 10 165 10s147 42 147 101v144q-24 28-49 0-24 28-49 0-25 28-49 0-25 28-49 0-25 28-49 0-25 28-49 0Z" />
      <path className="bot-preview__shine" d="M48 94C57 57 107 32 154 30" />
    </>}
    {model === "claw" && <>
      <rect className="bot-preview__joint" x="91" y="67" width="28" height="25" rx="2" />
      <rect className="bot-preview__joint" x="211" y="67" width="28" height="25" rx="2" />
      <rect className="bot-preview__limb" x="2" y="137" width="25" height="43" rx="3" />
      <rect className="bot-preview__limb" x="303" y="137" width="25" height="43" rx="3" />
      <rect className="bot-preview__limb" x="76" y="246" width="47" height="27" rx="3" />
      <rect className="bot-preview__limb" x="207" y="246" width="47" height="27" rx="3" />
      <rect className="bot-preview__body" x="14" y="86" width="302" height="171" rx="6" />
      <rect className="bot-preview__body" x="46" y="7" width="238" height="67" rx="5" />
      <path className="bot-preview__shine" d="M55 15H275M24 94H306" />
      <rect className="bot-preview__eye" x="95" y="32" width="48" height="11" rx="2" />
      <rect className="bot-preview__eye" x="187" y="32" width="48" height="11" rx="2" />
    </>}
    {model === "nimbus" && <>
      <path className="bot-preview__body" d="M26 263Q8 263 8 244V164Q8 117 60 112Q67 21 165 21Q263 21 270 112Q322 117 322 164V244Q322 263 304 263Z" />
      <path className="bot-preview__shine" d="M82 98Q94 43 155 40" />
    </>}
    {model === "meep" && <>
      <rect className="bot-preview__joint" x="160" y="17" width="10" height="34" rx="5" />
      <circle className="bot-preview__body" cx="165" cy="13" r="10" />
      <rect className="bot-preview__limb" x="2" y="135" width="25" height="49" rx="11" />
      <rect className="bot-preview__limb" x="303" y="135" width="25" height="49" rx="11" />
      <rect className="bot-preview__limb" x="76" y="244" width="50" height="29" rx="9" />
      <rect className="bot-preview__limb" x="204" y="244" width="50" height="29" rx="9" />
      <rect className="bot-preview__body" x="14" y="43" width="302" height="211" rx="47" />
      <path className="bot-preview__shine" d="M35 84Q42 57 72 57H111" />
    </>}
    {miniature && <g transform={`translate(${screen.x} ${screen.y}) scale(${screen.width / 252})`}>
      <rect className="bot-preview__mini-screen" x="0" y="0" width="252" height="128" rx="5" />
      {model === "claw" ? <>
        <rect className="bot-preview__mini-pixel" x="26" y="27" width="112" height="12" rx="3" />
        <rect className="bot-preview__mini-meter" x="26" y="58" width="200" height="22" rx="4" />
        <rect className="bot-preview__mini-pixel" x="26" y="58" width="136" height="22" rx="4" />
      </> : <>
        <rect className="bot-preview__mini-pixel" x="66" y="33" width="26" height="48" rx="13" />
        <rect className="bot-preview__mini-pixel" x="160" y="33" width="26" height="48" rx="13" />
      </>}
    </g>}
  </svg>;
}

/** A compact decorative portrait for a picker or a heading with visible bot text. */
export function BotAvatar({ model }: BotProps) {
  const selected = botModel(model);
  return <span className={`bot-avatar bot-avatar--${selected}`} aria-hidden="true">
    <Housing model={selected} miniature />
  </span>;
}

export default function BotPreview({ model, children }: BotProps & { children: ReactNode }) {
  const selected = botModel(model);
  const [enlarged, setEnlarged] = useState(false);
  return <section className={`bot-preview bot-preview--${selected}`} aria-label={`${names[selected]} screen preview`}>
    <div className="bot-preview__figure">
      <Housing model={selected} />
      <div className="bot-preview__screen">{children}</div>
    </div>
    <button type="button" className="bot-preview__enlarge" aria-expanded={enlarged} onClick={() => setEnlarged(!enlarged)}>{enlarged ? "Close enlarged screen" : "Enlarge screen"}</button>
    {enlarged && <div className="bot-preview__enlarged" aria-label="Enlarged screen preview">{children}</div>}
  </section>;
}
