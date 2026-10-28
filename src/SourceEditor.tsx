import { useEffect, useState } from "react";
import Modal from "./Modal";
import ImageUpload from "./ImageUpload";
import Oled from "./Oled";
import { BotAvatar } from "./BotPreview";
import { botName } from "./BotPicker";
import { invoke } from "@tauri-apps/api/core";

export type Location = { name: string; latitude: number; longitude: number; country: string; region: string };
export type SourceConfig = { customText: string; textSize: number; textAlign: string; textValign: string; textBorder: boolean; textMotion: string; scrollSpeed: string; imagePixels: string; displayInvert: boolean; displayCompanion: boolean; displayLayout: string; mode: string; location: Location | null; unit: string; symbol: string; currency: string; limitId: string; window: string; sessionId: string; cliPath: string; claudeSource: string; claudeProfile: string; claudeReportDirectory: string; claudeReportProfile: string; faceExpression: string; blink: boolean; animationSpeed: string; weatherLayout: string; showChange: boolean; clockZone: string; clockFormat: string; clockSeconds: boolean; clockDate: string; focusMinutes: number; breakMinutes: number; longBreakMinutes: number; focusSessions: number; timerRunning: boolean; timerStartedAt: number; timerElapsed: number; meetingStartsAt: number; meetingMinutes: number; monitorInterval: number; showGraph: boolean; memoryMetric: string; memoryUnit: string };
export type Widget = { id: string; kind: string; label: string; enabled: boolean; durationSec: number; headline: string; footer: string; config: SourceConfig };
type Window = { bucket: string; slot: string; label: string; usedPercent: number; minutes: number; resetsAt: number };
type Bridge = { enabled: boolean; command: string; hasExisting: boolean; explanation: string };
type SourceInfo = { claudeBridge: Bridge; claudeAccount: { signedIn: boolean; connecting: boolean; explanation: string } };
const minutesLabel = (n: number) => n % 1440 === 0 ? `${n / 1440} days` : n % 60 === 0 ? `${n / 60} hours` : `${n} minutes`;

export default function SourceEditor({ model, widget, offline, orientation, onClose, onApply, onRemove }: { model: string; widget: Widget; offline: boolean; orientation: number; onClose: () => void; onApply: (label: string, config: SourceConfig, headline: string, footer: string) => Promise<boolean>; onRemove: () => Promise<boolean> }) {
  const local = ["text","image","clock","pomodoro","meeting","cpu","memory"].includes(widget.kind);
  const [zones,setZones] = useState<string[]>([]);
  useEffect(() => {if(widget.kind!=="clock")return;let active=true;invoke<string[]>("time_zones").then(z => {if(active)setZones(z);}).catch(e => {if(active)setError(String(e));});return () => {active=false;};},[widget.kind]);
  const meetingDate = (epoch:number) => {if(!epoch)return "";const d=new Date(epoch*1000);return `${String(d.getFullYear()).padStart(4,"0")}-${String(d.getMonth()+1).padStart(2,"0")}-${String(d.getDate()).padStart(2,"0")}T${String(d.getHours()).padStart(2,"0")}:${String(d.getMinutes()).padStart(2,"0")}`;};
  const [label, setLabel] = useState(widget.label);
  const [config, setConfig] = useState<SourceConfig>({ ...{ customText: "Hello, human!", textMotion:"static",scrollSpeed:"normal",imagePixels:"", textSize: 1, textAlign: "center", textValign: "center", textBorder: false, displayInvert: false, displayCompanion: true, displayLayout: "classic" }, ...widget.config });
  const [headline, setHeadline] = useState(widget.headline);
  const [footer, setFooter] = useState(widget.footer);
  const [query, setQuery] = useState(widget.config.location?.name ?? "");
  const [locations, setLocations] = useState<Location[]>([]);
  const [windows, setWindows] = useState<Window[]>([]);
  const [info, setInfo] = useState<SourceInfo | null>(null);
  const [pending, setPending] = useState("");
  const [imagePending,setImagePending] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [draftFrame, setDraftFrame] = useState<number[] | null>(null);
  const [previewError, setPreviewError] = useState("");
  const [previewPending, setPreviewPending] = useState(true);
  const draftKey = JSON.stringify({ ...widget, label, config, headline, footer });
  useEffect(() => {
    let active = true;
    setPreviewPending(true); setPreviewError("");
    let timer: number;
    const refresh = async () => {
      try { const frame = await invoke<number[]>("preview_draft", { widget: JSON.parse(draftKey) }); if (active) { setDraftFrame(frame); setPreviewPending(false); } }
      catch(e) { if (active) { setPreviewError(String(e)); setPreviewPending(false); } return; }
      if(active && widget.kind === "text" && config.textMotion === "scroll") timer = window.setTimeout(refresh, 100);
    };
    timer = window.setTimeout(refresh, 220);
    return () => { active = false; window.clearTimeout(timer); };
  }, [draftKey]);
  useEffect(() => {
    if(widget.kind !== "claude" || offline)return;
    let active=true;setInfo(null);
    const timer=window.setTimeout(() => invoke<SourceInfo>("source_info",{profile:config.claudeProfile}).then(value => {if(active)setInfo(value);}).catch(e => {if(active)setError(String(e));}),350);
    return () => {active=false;window.clearTimeout(timer);};
  },[widget.kind,config.claudeProfile,offline]);
  async function run(name: string, action: () => Promise<void>) {
    setPending(name); setError(""); setNotice("");
    try { await action(); } catch (e) { setError(String(e)); } finally { setPending(""); }
  }
  const patch = (value: Partial<SourceConfig>) => setConfig(c => ({ ...c, ...value }));
  return <Modal title={`Configure ${botName(model)} screen`} onClose={onClose}><form className="editor source-editor" onSubmit={e => {
    e.preventDefault(); if (previewPending || previewError || pending || imagePending || (widget.kind === "image" && !config.imagePixels)) return; run("Applying", async () => { if (await onApply(label, config, headline, footer)) onClose(); else setError("Settings could not be applied. Check the widget settings, then try again."); });
  }}>
    <div className="bot-context"><BotAvatar model={model}/><span>{botName(model)} · screen settings</span></div><h2>{widget.kind === "stock" ? "Stocks" : widget.kind === "codex" ? "Codex allowance" : widget.kind === "claude" ? "Claude allowance" : widget.label}</h2>
    <section className="draft-preview" aria-label="Your screen preview"><div className="eyebrow">128 × 64 SCREEN PREVIEW</div>{draftFrame ? <Oled frame={draftFrame} brightness={100} orientation={orientation} description={`${label} draft screen`} /> : <div className="preview-placeholder">Preparing your screen…</div>}<p role="status">{previewPending ? "Updating preview…" : previewError ? "Your last valid preview is shown. Adjust the settings below." : offline && !local && widget.kind !== "face" && config.mode === "live" ? "Online data is paused. This preview may show saved data; it is not a live reading." : "This is how your screen will look. Apply to save it."}</p>{previewError && <p className="source-error" role="alert">{previewError}</p>}</section>
    {offline && !local && widget.kind !== "face" && <p className="offline-notice">Online data is paused. Sample screens still work. Resume online data to connect accounts or search for a city.</p>}
    <label>Name<input maxLength={24} required value={label} onChange={e => setLabel(e.target.value)} /></label>
    {widget.kind === "text" && <>
      <label>Your text<textarea aria-label="Your text" value={config.customText} rows={4} maxLength={config.textMotion === "scroll" ? 256 : 128} spellCheck={false} aria-describedby="text-help" onChange={e => patch({ customText: e.target.value })}/></label>
      <p id="text-help">{config.textMotion === "scroll" ? "Up to 256 characters. Your message repeats from right to left; line breaks become spaces." : "Up to 128 characters. New lines are welcome; text wraps to fit the screen."} Use English letters, numbers and basic punctuation. Check the preview before applying.</p>
      <label>Text motion<select value={config.textMotion} onChange={e=>patch({textMotion:e.target.value})}><option value="static">Stationary</option><option value="scroll">Scroll across the screen</option></select></label>
      {config.textMotion === "scroll" && <><label>Scrolling speed<select value={config.scrollSpeed} onChange={e=>patch({scrollSpeed:e.target.value})}><option value="slow">Slow</option><option value="normal">Normal</option><option value="fast">Fast</option></select></label><p>Keep DroidFun App running for scrolling. Playlist time adjusts for one pass, up to 120 seconds. Pin longer or slower messages to keep them playing. Pause stops motion; system reduced-motion settings show a still portion.</p></>}
      <div className="screen-field-pair"><label>Text size<select value={config.textSize} onChange={e => patch({ textSize: Number(e.target.value) })}><option value={1}>Small · more text</option><option value={2}>Large · easier to read</option></select></label><label>Horizontal<select disabled={config.textMotion === "scroll"} value={config.textAlign} onChange={e => patch({ textAlign: e.target.value })}><option value="left">Left</option><option value="center">Center</option><option value="right">Right</option></select></label></div>
      <label>Vertical position<select value={config.textValign} onChange={e => patch({ textValign: e.target.value })}><option value="top">Top</option><option value="center">Center</option><option value="bottom">Bottom</option></select></label>
      <label className="inline-check"><input type="checkbox" checked={config.textBorder} onChange={e => patch({ textBorder: e.target.checked })}/>Add a border</label>
    </>}
    {widget.kind !== "text" && widget.kind !== "image" && widget.kind !== "face" && <div className="screen-appearance"><h3>Screen style</h3><label>Reading alignment<select value={config.displayLayout} onChange={e => patch({ displayLayout: e.target.value })}><option value="classic">Left aligned</option><option value="minimal">Centered reading</option></select></label><p>Information screens use clear labels and readings, without a face or logo.</p></div>}
    {widget.kind === "image" && <ImageUpload onBusy={setImagePending} hasImage={Boolean(config.imagePixels)} onChange={imagePixels=>patch({imagePixels})} onName={name=>{if(label === "Uploaded image")setLabel(name);}}/>}
    <label className="inline-check"><input type="checkbox" checked={config.displayInvert} onChange={e => patch({ displayInvert: e.target.checked })}/>Invert screen colors</label>
    {widget.kind === "face" && <><label>Expression<select value={config.faceExpression} onChange={e => patch({faceExpression:e.target.value})}><option value="happy">Happy</option><option value="neutral">Neutral</option><option value="sleepy">Sleepy</option><option value="surprised">Surprised</option></select></label><label className="inline-check"><input type="checkbox" checked={config.blink} onChange={e => patch({blink:e.target.checked})}/>Blink</label><label>Animation speed<select value={config.animationSpeed} onChange={e => patch({animationSpeed:e.target.value})}><option value="slow">Relaxed</option><option value="normal">Normal</option><option value="quick">Lively</option></select></label><p>Blinks gently while this face is on screen. System reduced-motion preferences turn animation off. On USB wall power, your bot cycles its saved text and face screens. If none are saved, it shows its built-in happy face.</p></>}
    {!local && widget.kind !== "face" && <label>Data source<select value={config.mode} onChange={e => patch({ mode: e.target.value, window: widget.kind === "claude" && e.target.value === "live" ? "five_hour" : config.window })}><option value="demo">Demo / sample values</option><option value="live">Live data</option></select></label>}
    {!local && widget.kind !== "face" && config.mode === "demo" && <><label>Sample big text<input maxLength={24} value={headline} onChange={e => setHeadline(e.target.value)} /></label><label>Sample footer<input maxLength={24} value={footer} onChange={e => setFooter(e.target.value)} /></label><p>Demo values are labeled on the app and OLED.</p></>}
    {widget.kind === "clock" && <>
      <label>Time zone<select value={config.clockZone} onChange={e => patch({clockZone:e.target.value})}><option value="local">This computer’s time zone</option>{zones.map(zone => <option key={zone} value={zone}>{zone.replaceAll("_"," ")}</option>)}</select></label>
      <label>Time format<select value={config.clockFormat} onChange={e => patch({clockFormat:e.target.value})}><option value="12">12-hour · AM/PM</option><option value="24">24-hour</option></select></label>
      <label className="inline-check"><input type="checkbox" checked={config.clockSeconds} onChange={e => patch({clockSeconds:e.target.checked})}/>Show seconds</label>
      <label>Date line<select value={config.clockDate} onChange={e => patch({clockDate:e.target.value})}><option value="weekday">Weekday, month and day</option><option value="date">Year-month-day</option><option value="none">Hide date</option></select></label><p>Uses the computer’s clock. Add another clock for a different time zone. Seconds stay below the large time so the bot has room.</p>
    </>}
    {widget.kind === "pomodoro" && <>
      <label>Focus minutes<input type="number" required min={1} max={120} value={config.focusMinutes} onChange={e => patch({focusMinutes:Number(e.target.value)})}/></label>
      <label>Short break minutes<input type="number" required min={1} max={60} value={config.breakMinutes} onChange={e => patch({breakMinutes:Number(e.target.value)})}/></label>
      <label>Long break minutes<input type="number" required min={1} max={120} value={config.longBreakMinutes} onChange={e => patch({longBreakMinutes:Number(e.target.value)})}/></label>
      <label>Focus sessions before long break<input type="number" required min={2} max={12} value={config.focusSessions} onChange={e => patch({focusSessions:Number(e.target.value)})}/></label>
      <p>Use Start, Pause, Reset and Skip phase beside this widget in the playlist. The timer keeps going while other screens show, the playlist is paused, or DroidFun App is closed. Changing these lengths resets the timer to paused. Breaks switch automatically; reminders are visual.</p>
    </>}
    {widget.kind === "meeting" && <>
      <label>Meeting date and time<input type="datetime-local" required value={meetingDate(config.meetingStartsAt)} onChange={e => patch({meetingStartsAt:Number.isFinite(new Date(e.target.value).getTime()) ? Math.floor(new Date(e.target.value).getTime()/1000):0})}/></label>
      <label>Meeting duration (minutes)<input type="number" required min={1} max={480} value={config.meetingMinutes} onChange={e => patch({meetingMinutes:Number(e.target.value)})}/></label>
      <p>Use Name for the meeting title. The time uses this computer’s time zone ({Intl.DateTimeFormat().resolvedOptions().timeZone}). This is a manually entered meeting; it shows the countdown, then NOW during the meeting and DONE afterward. Add a separate widget for another meeting.</p>
    </>}
    {(widget.kind === "cpu" || widget.kind === "memory") && <>
      <label>Update every<select value={config.monitorInterval} onChange={e => patch({monitorInterval:Number(e.target.value)})}>{[1,2,5,10,30].map(n => <option key={n} value={n}>{n} seconds</option>)}</select></label>
      {widget.kind === "cpu" ? <><label className="inline-check"><input type="checkbox" checked={config.showGraph} onChange={e => patch({showGraph:e.target.checked})}/>Show last-minute history</label><p>Shows CPU load across all cores on this computer. History builds as readings arrive; disabled monitors stop sampling.</p></> : <><label>Memory to show<select value={config.memoryMetric} onChange={e => patch({memoryMetric:e.target.value})}><option value="used">Used memory</option><option value="available">Available memory</option></select></label><label>Units<select value={config.memoryUnit} onChange={e => patch({memoryUnit:e.target.value})}><option value="gib">GiB · matches installed RAM sizes</option><option value="gb">GB · decimal gigabytes</option></select></label><p>Shows this computer’s RAM, with its total and a percentage bar. Disabled monitors stop sampling.</p></>}
    </>}
    {config.mode === "live" && widget.kind === "weather" && <>
      <label>City search<div className="input-action"><input value={query} onChange={e => setQuery(e.target.value)} placeholder="City or postal code" /><button type="button" disabled={offline || !!pending || query.trim().length < 2} onClick={() => run("Searching", async () => { const found = await invoke<Location[]>("search_weather", { name: query }); setLocations(found); if (!found.length) setNotice("No places found. Try a nearby city or a different spelling."); })}>Find city</button></div></label>
      <div className="location-results">{locations.map((l, index) => <button type="button" key={index} onClick={() => { patch({ location: l }); if (label === "Weather" || label === config.location?.name.slice(0, 24)) setLabel(l.name.slice(0, 24)); setLocations([]); }}><strong>{l.name}</strong><small>{[l.region, l.country].filter(Boolean).join(", ")}</small></button>)}</div>
      {config.location && <p className="source-notice">Selected: {config.location.name}, {config.location.country}</p>}
      <label>Temperature<select value={config.unit} onChange={e => patch({ unit: e.target.value })}><option value="fahrenheit">Fahrenheit (°F)</option><option value="celsius">Celsius (°C)</option></select></label>
      <label>Weather layout<select value={config.weatherLayout} onChange={e => patch({weatherLayout:e.target.value})}><option value="current">Current temperature and condition</option><option value="high_low">Current temperature and daily high/low</option></select></label>
      <p>Current conditions and high/low from <a href="https://open-meteo.com/" target="_blank" rel="noreferrer">Open-Meteo</a>. Refreshes every 15 minutes for enabled widgets. Free service is for personal, non-commercial use.</p>
    </>}
    {config.mode === "live" && widget.kind === "stock" && <>
      <label>Stock symbol<input maxLength={20} required value={config.symbol} onChange={e => patch({ symbol: e.target.value.toUpperCase() })} placeholder="For example, MSFT" /></label>
      <label className="inline-check"><input type="checkbox" checked={config.showChange} onChange={e => patch({showChange:e.target.checked})}/>Show price change when available</label><p>The quote date stays visible. A change is shown only when the provider returns a comparable prior close.</p>
      <p>Free public quotes from Yahoo Finance. No account, API key, or paid plan needed. Currency comes from the quote itself. Refreshes every 15 minutes while enabled; the OLED shows the quote date and values may be delayed.</p>
      <p>For another exchange, use its symbol suffix, such as SHOP.TO. Add a separate stock widget for each symbol.</p>
    </>}
    {config.mode === "live" && widget.kind === "codex" && <>
      <p>Connects to the Codex app or CLI already signed in on this computer. Reads account allowance without starting a model turn.</p>
      <button type="button" disabled={offline || !!pending} onClick={() => run("Connecting Codex", async () => { const values = await invoke<Window[]>("codex_windows", { path: config.cliPath }); setWindows(values); setNotice(values.length ? "Codex connected. Choose an allowance window, then Apply to show it on your bot." : "This account did not return allowance windows."); })}>{pending === "Connecting Codex" ? "Connecting…" : "Connect Codex"}</button>
      {windows.length > 0 ? <label>Allowance window<select value={JSON.stringify([config.limitId, config.window])} onChange={e => { const [limitId, window] = JSON.parse(e.target.value); patch({ limitId, window }); }}>{!windows.some(w => w.bucket === config.limitId && w.slot === config.window) && <option value={JSON.stringify([config.limitId, config.window])} disabled hidden>Choose a returned window</option>}{windows.map(w => <option key={`${w.bucket}:${w.slot}`} value={JSON.stringify([w.bucket, w.slot])}>{w.label} · {minutesLabel(w.minutes)} · {Math.floor(Math.max(0, 100 - w.usedPercent))}% left</option>)}</select></label> : <p>Selected bucket: {config.limitId} / {config.window}. Read the account to see its available windows.</p>}
      <details><summary>CLI location</summary><label>Codex executable (optional)<input value={config.cliPath} onChange={e => patch({ cliPath: e.target.value })} placeholder="Leave empty for automatic detection" /></label></details>
      <p>Refreshes at most every five minutes. Missing windows and expired reports become unavailable.</p>
    </>}
    {config.mode === "live" && widget.kind === "claude" && <>
      <label>Allowance window<select value={config.window} onChange={e => patch({ window: e.target.value })}><option value="five_hour">Five-hour allowance</option><option value="seven_day">Seven-day allowance</option></select></label>
      <label>Usage source<select value={config.claudeSource} onChange={e => patch({ claudeSource: e.target.value })}><option value="account">Shared Claude account (desktop, web and Code)</option><option value="statusline">Claude Code status-line report</option></select></label>
      {config.claudeSource === "account" ? <>
        <div className="source-group"><strong>Claude account</strong><p>{info?.claudeAccount.explanation ?? (offline ? "Account sign-in is paused with online data." : "Checking your local Claude sign-in…")}</p><p>Reads the allowance shared by the Claude desktop app, website and Code. Uses your Claude Code sign-in; no prompt is sent and no model tokens are consumed.</p><button type="button" disabled={offline || !!pending || !!info?.claudeAccount.connecting} onClick={() => run("Complete Claude sign-in in your browser", async () => { await invoke("connect_claude_account",{profile:config.claudeProfile}); setInfo(await invoke<SourceInfo>("source_info",{profile:config.claudeProfile})); setNotice("Claude account connected. The allowance will update shortly."); })}>{info?.claudeAccount.connecting ? "Sign-in open in your browser" : "Connect Claude account"}</button></div>
        <details><summary>Another Claude account profile</summary><label>Claude Code profile folder (optional)<input value={config.claudeProfile} onChange={e => patch({claudeProfile:e.target.value})} placeholder="Leave empty for your default sign-in"/></label><p>Use the absolute folder you configured with CLAUDE_CONFIG_DIR. This account can have different allowances. Connect uses the chosen profile; percentages are never added together.</p></details>
        <p>Refreshes every five minutes. Missing or expired allowances show a reconnect explanation rather than a made-up percentage.</p>
      </> : <>
      <div className="source-group"><strong>Claude Code bridge</strong><p>{info?.claudeBridge.explanation ?? "Connect Claude Code's status line to report usage locally."}</p><p>Uses the most recent report in your local profile. A report arrives after an eligible Claude Code session receives an API response.</p><button type="button" disabled={offline || !info || !!pending} onClick={() => run("Updating bridge", async () => { await invoke("set_claude_bridge", { enabled: !info?.claudeBridge.enabled }); setInfo(await invoke<SourceInfo>("source_info",{profile:config.claudeProfile})); setNotice(info?.claudeBridge.enabled ? "Original status line restored." : "Bridge enabled. Use Claude Code to send its first report."); })}>{info?.claudeBridge.enabled ? "Restore original status line" : "Enable Claude Code bridge"}</button><details><summary>Proposed integration</summary><code>{info?.claudeBridge.command}</code><p>{info?.claudeBridge.enabled ? "The bridge is enabled. Restore original status line removes this integration and restores your saved setting." : info?.claudeBridge.hasExisting ? "Your current status-line command will be backed up and its output chained unchanged." : "No existing status-line command was found. A short Claude status line will be added."}</p></details></div>
      <details><summary>Choose a report folder or session</summary><label>Reports folder (optional)<input value={config.claudeReportDirectory} onChange={e => patch({claudeReportDirectory:e.target.value})} placeholder="Leave empty for local default reports"/></label><label>Report profile<input value={config.claudeReportProfile} onChange={e => patch({claudeReportProfile:e.target.value})}/></label><p>For WSL, use an accessible Windows path to the reports folder, and configure the helper inside WSL separately.</p><label>Session ID (optional)<input value={config.sessionId} onChange={e => patch({ sessionId: e.target.value })} placeholder="Leave empty for the most recent report" /></label></details>
      <p>Last reported by Claude Code; stale after 10 minutes. No report means waiting, never 0% usage.</p>
      </>}

    </>}
    {pending && <p role="status">{pending}…</p>}{notice && <p className="source-notice" role="status">{notice}</p>}{error && <p className="source-error" role="alert">{error}</p>}
    <div className="modal-actions"><button type="button" onClick={onClose}>Cancel</button><button type="submit" className="primary-control" disabled={!!pending || imagePending || previewPending || !!previewError || (widget.kind === "image" && !config.imagePixels)}>Apply</button></div>
    {widget.id !== "face" && <div className="remove-widget">{confirmRemove ? <><span>Remove this widget from your playlist?</span><button type="button" disabled={!!pending} onClick={() => run("Removing", async () => { if (await onRemove()) onClose(); })}>Remove widget</button><button type="button" onClick={() => setConfirmRemove(false)}>Keep it</button></> : <button type="button" onClick={() => setConfirmRemove(true)}>Remove from playlist</button>}</div>}
  </form></Modal>;
}
