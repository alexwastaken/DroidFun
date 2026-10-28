import { useEffect, useRef, type ReactNode } from "react";
import { BotAvatar } from "./BotPreview";

export const botModels = [
  { id: "nimbus", name: "Nimbus Bot", summary: "A little cloud for your desk.", detail: "Weather, faces, clocks, custom text and everyday updates.", face: "nimbus" },
  { id: "boo", name: "Boo Bot", summary: "Your friendly little desk ghost.", detail: "Faces, weather, clocks, text and more.", face: "boo" },
  { id: "claw", name: "Claw Bot", summary: "Your AI usage, at a glance.", detail: "Dedicated Claude and Codex allowance screens.", face: "claw" },
  { id: "meep", name: "Meep Bot", summary: "Make your desk a little more you.", detail: "Custom text, faces, clocks, weather, stocks and more.", face: "meep" },
];
const generalOptions = [["text", "Custom text"], ["face", "Face"], ["clock", "Clock"], ["pomodoro", "Focus timer"], ["meeting", "Meeting countdown"], ["cpu", "Computer CPU"], ["memory", "Computer memory"], ["weather", "Weather"], ["stock", "Stocks"]];
const usageOptions = [["codex", "Codex allowance"], ["claude", "Claude allowance"]];
export const botName = (model: string) => botModels.find(bot => bot.id === model)?.name ?? "";
export const widgetOptions = (model: string) => model === "claw" ? usageOptions : generalOptions;
export const widgetAllowed = (model: string, kind: string) => widgetOptions(model).some(option => option[0] === kind);

export default function BotPicker({ current, busy, error, onSelect, onCancel, footer }: { current: string; busy: boolean; error: string; onSelect: (model: string) => Promise<void>; onCancel?: () => void; footer?: ReactNode }) {
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => { title.current?.focus(); }, []);
  return <main className="app-shell bot-picker">
    <div className="app-brand"><img src="/brand/droid-fun-wordmark.png" alt="Droid Fun" /><span>App</span></div>
    <div className="intro"><div className="eyebrow">CHOOSE A MODEL</div><h1 ref={title} tabIndex={-1}>Meet the droids.</h1><p>Choose a model to explore its screens. When you plug in a factory-prepared droid, we recognize it automatically.</p></div>
    <div className="bot-choices" aria-label="Choose your bot">{botModels.map(bot => <button key={bot.id} className="bot-choice" data-bot={bot.id} disabled={busy} aria-pressed={current === bot.id} onClick={() => onSelect(bot.id)}>
      <BotAvatar model={bot.id}/>
      <span className="bot-choice-copy"><strong>{bot.name}</strong><span>{bot.summary}</span><small>{bot.detail}</small></span><span aria-hidden="true">→</span>
    </button>)}</div>
    <p className="bot-picker-note">Each droid has its own screens and settings. You can change your selection any time.</p>
    {busy && <p role="status">Getting your bot ready…</p>}{error && <p className="source-error" role="alert">{error}</p>}
    {onCancel && <button className="settings-link" disabled={busy} onClick={onCancel}>{botName(current) ? `Back to ${botName(current)}` : "Back to connection"}</button>}
    <footer>{footer}</footer>
  </main>;
}
