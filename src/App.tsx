import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import SettingsPanel, { type Preferences } from "./Settings";
import Modal from "./Modal";

type Playback = "running" | "paused" | "pinned";
import SourceEditor, { type Widget, type SourceConfig } from "./SourceEditor";
type DataCard = { sourceId: string; state: string; detail: string; fetchedAt: number; observedAt: number; expiresAt: number; headline: string; footer: string };
type Snapshot = { settings: { version: number; widgets: Widget[]; brightness: number; preferences: Preferences; manualDisplayOff: boolean }; currentId: string; currentLabel: string; playback: Playback; remainingSec: number; totalSec: number; displayOn: boolean; temporary: boolean; displayReason: string; quietActive: boolean; quietOverrideUntil: number; frame: number[]; animationPhase: number; data: Record<string, DataCard> };
type Connection = { state: string; detail: string; port: string | null; boardId: string | null; firmware: string | null; displayHealth: string | null };
type RobotPort = { path: string; serial: string; product: string; vid: number; pid: number };
type Editor = Widget;

const icons: Record<string, string> = { face: "◡", weather: "☀", stock: "↗", codex: "⌘", claude: "✦", clock:"◷", pomodoro:"◴", meeting:"▣", cpu:"▥", memory:"▤" };
const subtitles: Record<string, string> = {
  face: "A little company for your desk",
  weather: "Sample weather • connect later",
  stock: "Sample quote • connect later",
  codex: "Sample usage • connect later",
  claude: "Sample usage • connect later",
};

function Oled({ frame, brightness, orientation, description }: { frame: number[]; brightness: number; orientation: number; description: string }) {
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

function reportAge(observedAt: number) {
  const minutes = Math.max(0, Math.floor((Date.now() / 1000 - observedAt) / 60));
  if (minutes < 60) return `${minutes} min`;
  if (minutes < 1440) return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
  return `${Math.floor(minutes / 1440)}d ${Math.floor(minutes % 1440 / 60)}h`;
}

export default function App() {
  const [state, setState] = useState<Snapshot | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [preview, setPreview] = useState<number[] | null>(null);
  const [editor, setEditor] = useState<Editor | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [connection, setConnection] = useState<Connection | null>(null);
  const [ports, setPorts] = useState<RobotPort[]>([]);
  const [scanning, setScanning] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(() => Boolean((window as Window & { __deskbotOpenSettings?: boolean }).__deskbotOpenSettings));
  const [dragging, setDragging] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [addKind, setAddKind] = useState("weather");
  const previewKey = selected ? JSON.stringify({ widget: state?.settings.widgets.find(w => w.id === selected), data: state?.data?.[selected] }) : "";

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    invoke<Snapshot>("get_state").then(value => { if (!disposed) setState(value); }).catch(e => setError(String(e)));
    invoke<Connection>("get_connection").then(value => { if (!disposed) setConnection(value); }).catch(e => setError(String(e)));
    invoke<RobotPort[]>("list_robot_ports").then(value => { if (!disposed) setPorts(value); }).catch(e => setError(String(e)));
    listen<Snapshot>("deskbot-state", event => { if (!disposed) setState(event.payload); }).then(fn => { if (disposed) fn(); else unlisten = fn; });
    let unlistenConnection: (() => void) | undefined;
    listen<Connection>("deskbot-connection", event => { if (!disposed) setConnection(event.payload); }).then(fn => { if (disposed) fn(); else unlistenConnection = fn; });
    return () => { disposed = true; unlisten?.(); unlistenConnection?.(); };
  }, []);

  useEffect(() => {
    const dark = window.matchMedia("(prefers-color-scheme: dark)");
    const appearance = state?.settings.preferences.appearance ?? "system";
    const apply = () => { document.documentElement.dataset.theme = appearance === "system" ? (dark.matches ? "dark" : "light") : appearance; };
    apply(); dark.addEventListener("change",apply); return () => dark.removeEventListener("change",apply);
  },[state?.settings.preferences.appearance]);
  useEffect(() => {
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const apply = () => { invoke<Snapshot>("set_reduced_motion",{reduced:motion.matches}).then(setState).catch(e => setError(String(e))); };
    apply(); motion.addEventListener("change",apply);
    let off: (() => void) | undefined;
    let disposed = false;
    listen("deskbot-open-settings",() => setSettingsOpen(true)).then(fn => {if(disposed)fn();else off=fn;});
    return () => { disposed=true; motion.removeEventListener("change",apply);off?.(); };
  },[]);

  async function scanPorts() {
    setScanning(true); setError("");
    try { setPorts(await invoke<RobotPort[]>("list_robot_ports")); }
    catch (e) { setError(String(e)); }
    finally { setScanning(false); }
  }

  useEffect(() => {
    if (!selected) { setPreview(null); return; }
    let active = true;
    invoke<number[]>("preview_frame", { id: selected }).then(frame => { if (active) setPreview(frame); }).catch(e => setError(String(e)));
    return () => { active = false; };
  }, [selected, previewKey, state?.animationPhase, state?.settings.preferences.companionFace, state?.settings.preferences.companionAnimation]);

  async function act(command: string, args: Record<string, unknown> = {}) {
    setBusy(true); setError("");
    try { setState(await invoke<Snapshot>(command, args)); return true; }
    catch (e) { setError(String(e)); return false; }
    finally { setBusy(false); }
  }

  if (!state) return <main className="loading">Waking up DroidFun App…{error && <p role="alert">{error}</p>}</main>;
  const widgets = state.settings.widgets;
  const selectedWidget = widgets.find(w => w.id === selected);
  const current = widgets.find(w => w.id === state.currentId);
  const activeCount = widgets.filter(w => w.enabled).length;
  const status = !state.displayOn ? state.displayReason : state.playback === "pinned" ? "Pinned" : state.playback === "paused" ? "Paused" : state.temporary ? "Showing now" : "Cycling";
  const visibleFrame = selected && preview ? preview : state.frame;
  const visibleWidget = selectedWidget ?? current;
  const liveCard = visibleWidget ? state.data?.[visibleWidget.id] : undefined;
  const isLocal = visibleWidget && ["clock","pomodoro","meeting","cpu","memory"].includes(visibleWidget.kind);
  const sourceLabel = visibleWidget?.kind === "face" ? "" : isLocal ? "local" : visibleWidget?.config.mode === "live" ? (liveCard?.state ?? "waiting") : "demo";

  return <main className="app-shell">
    <div className="topline"><div className="app-brand"><img src="/brand/droid-fun-wordmark.png" alt="Droid Fun" /><span>App</span></div><span className="demo-pill">{connection?.state === "connected" ? "ROBOT CONNECTED" : "PREVIEW MODE"}</span></div>
    <section className="intro"><div><div className="eyebrow">YOUR DROID, YOUR WAY</div><h1>Your desk could use<br /><em>some company.</em></h1><p>Choose what your droid shows. Add your favorite widgets and connect with USB.</p></div></section>
    <section className="robot-wrap" aria-label="Robot screen preview">
      <div className="antenna"><span /></div>
      <div className="robot-head"><span className="head-highlight" /><div className="screen-bezel"><Oled frame={visibleFrame} brightness={state.displayOn || selected ? state.settings.brightness : 0} orientation={state.settings.preferences.orientation} description={`${selectedWidget?.label ?? current?.label ?? "Happy face"} OLED preview. ${liveCard?.headline ?? visibleWidget?.headline ?? ""}. ${liveCard?.footer ?? visibleWidget?.footer ?? ""}. ${liveCard?.detail ?? visibleWidget?.config.faceExpression ?? ""}`} /></div><div className="robot-cheek left" /><div className="robot-cheek right" /></div>
      <div className="robot-neck" />
    </section>
    <div className="screen-caption"><div><div className="eyebrow">ON THE SCREEN {sourceLabel && <span className="demo-text">· {sourceLabel.toUpperCase().replaceAll("_", " ")}</span>}</div><h2>{selectedWidget ? selectedWidget.label : (current?.label ?? state.currentLabel)}</h2><span className="caption-sub">{selectedWidget ? "Local preview only — cycle continues" : `${status}${state.playback === "running" && state.displayOn ? ` · ${state.remainingSec}s left` : ""}`}</span></div><span className={`status-dot ${state.displayOn ? "" : "off"}`} /></div>
    {visibleWidget?.config.mode === "live" && <div className="live-source-detail"><span>{liveCard?.detail ?? "Waiting for this source to be configured or refreshed."}</span>{liveCard?.observedAt ? <small>{visibleWidget.kind === "stock" ? "Quote age" : "Report age"}: {reportAge(liveCard.observedAt)}</small> : null}{!isLocal && <button disabled={busy} onClick={() => act("refresh_widget", { id: visibleWidget.id })}>Refresh within source limits</button>}</div>}
    {selected && <div className="preview-actions"><button onClick={() => setSelected(null)}>Back to live cycle</button><button disabled={!selectedWidget?.enabled || busy} onClick={() => act("show_now", { id: selected })}>Show now</button><button disabled={!selectedWidget?.enabled || busy} onClick={() => act("pin_widget", { id: selected })}>Pin</button></div>}
    {(state.quietActive || state.quietOverrideUntil > Date.now()/1000) && <div className="quiet-status" role="status"><span>{state.displayReason}</span>{state.quietActive && <button disabled={busy} onClick={() => act("override_quiet")}>Wake for 15 minutes</button>}</div>}
    <div className="playback-controls"><button className="primary-control" disabled={busy} onClick={() => act("toggle_pause")}>{state.playback === "running" ? "Ⅱ  Pause cycle" : "▶  Resume cycle"}</button><button className="secondary-control" disabled={busy} onClick={() => act("next_widget")}>Next  →</button></div>
    <section className="robot-connection" aria-label="Robot connection"><div className="section-heading"><div><div className="eyebrow">USB ROBOT</div><h2>Connection</h2></div><button className="scan-button" disabled={scanning} onClick={scanPorts}>{scanning ? "Scanning…" : "Scan USB"}</button></div>
      <p className={`connection-state ${connection?.state ?? "preview"}`}>{connection?.state.replaceAll("_", " ") ?? "Preview only"}</p>
      <p className="connection-detail">{connection?.detail ?? "Checking USB devices…"}</p>
      {connection?.boardId && <p className="connection-detail">Board {connection.boardId} · Firmware {connection.firmware} · OLED {connection.displayHealth}</p>}
      {ports.length === 0 ? <p className="connection-detail">No RP2040-Zero serial port found. A USB data cable is required.</p> : ports.map(port => <div className="port-row" key={port.serial}><div><strong>{port.product}</strong><small>{port.path} · {port.serial}</small></div><button disabled={busy} onClick={() => act("select_robot", { serial: port.serial })}>Connect</button></div>)}
      {connection?.state === "setup_needed" && <p className="connection-detail">This device has not answered the DroidFun App protocol. It may be running a diagnostic or another sketch; install the DroidFun App firmware to control it here.</p>}
    </section>
    <section className="playlist"><div className="section-heading"><div><div className="eyebrow">YOUR PLAYLIST</div><h2>Widgets</h2></div><span className="count-pill">{activeCount} enabled</span></div>
      <p className="section-intro">Switch widgets on, change their order, and choose how long each appears.</p>
      <div className="widget-list">{widgets.map((widget, index) => <div className={`widget-row ${selected === widget.id ? "selected" : ""}`} key={widget.id} onDragOver={e => {if(dragging)e.preventDefault();}} onDrop={e => {e.preventDefault();if(dragging)act("reorder_widget",{id:dragging,before:widget.id});setDragging(null);}}>
        <button className="widget-main" onClick={() => setSelected(widget.id)} aria-label={`Preview ${widget.label}`}><span className={`widget-icon ${widget.kind}`}>{icons[widget.kind]}</span><span className="widget-copy"><strong>{widget.label}</strong><small>{widget.config.mode === "live" ? (state.data?.[widget.id]?.state === "fresh" ? state.data[widget.id].detail : (state.data?.[widget.id]?.state.replaceAll("_", " ") ?? "Waiting for source")) : (subtitles[widget.kind] ?? "Sample widget")}</small></span></button>
        <label className="switch" aria-label={`Enable ${widget.label}`}><input type="checkbox" checked={widget.enabled} disabled={busy} onChange={e => act("set_enabled", { id: widget.id, enabled: e.target.checked })} /><span /></label>
        <div className="widget-tools"><button className="drag-handle" draggable onDragStart={e => {setDragging(widget.id);e.dataTransfer.setData("text/plain",widget.id);e.dataTransfer.effectAllowed="move";}} onDragEnd={() => setDragging(null)} title="Drag to reorder; use the arrow buttons with a keyboard" aria-label={`Drag ${widget.label} to reorder`}>⠿</button><span>Show for <select aria-label={`${widget.label} duration`} value={widget.durationSec} disabled={busy} onChange={e => act("set_duration", { id: widget.id, durationSec: Number(e.target.value) })}>{[3, 5, 8, 10, 12, 15, 20, 30, 45, 60, 90, 120].includes(widget.durationSec) ? null : <option value={widget.durationSec}>{widget.durationSec}s</option>}{[3, 5, 8, 10, 12, 15, 20, 30, 45, 60, 90, 120].map(n => <option key={n} value={n}>{n}s</option>)}</select></span><div><button title="Move up" aria-label={`Move ${widget.label} up`} disabled={busy || index === 0} onClick={() => act("move_widget", { id: widget.id, direction: -1 })}>↑</button><button title="Move down" aria-label={`Move ${widget.label} down`} disabled={busy || index === widgets.length - 1} onClick={() => act("move_widget", { id: widget.id, direction: 1 })}>↓</button><button className="edit-button" onClick={() => setEditor(widget)}>Edit</button></div></div>
        {widget.kind === "pomodoro" && <div className="timer-controls" aria-label={`${widget.label} timer controls`}><span>{state.data?.[widget.id]?.detail ?? "Ready to focus"}</span><div><button disabled={busy} onClick={() => act("timer_action",{id:widget.id,action:widget.config.timerRunning ? "pause":"start"})}>{widget.config.timerRunning ? "Pause timer":"Start timer"}</button><button disabled={busy} onClick={() => act("timer_action",{id:widget.id,action:"reset"})}>Reset timer</button><button disabled={busy} onClick={() => act("timer_action",{id:widget.id,action:"skip"})}>Skip phase</button></div></div>}
      </div>)}</div>
      <div className="add-widget-controls">{adding ? <><select aria-label="New widget type" value={addKind} onChange={e => setAddKind(e.target.value)}><option value="clock">Clock</option><option value="pomodoro">Pomodoro focus timer</option><option value="meeting">Meeting countdown</option><option value="cpu">Computer CPU</option><option value="memory">Computer memory</option><option value="weather">Weather location</option><option value="stock">Stock symbol</option><option value="codex">Codex allowance window</option><option value="claude">Claude allowance window</option></select><button disabled={busy || widgets.length >= 30} onClick={async () => { setBusy(true); setError(""); try { const next = await invoke<Snapshot>("add_widget", { kind: addKind }); setState(next); setEditor(next.settings.widgets[next.settings.widgets.length - 1]); setAdding(false); } catch (e) { setError(String(e)); } finally { setBusy(false); } }}>Add</button><button onClick={() => setAdding(false)}>Cancel</button></> : <button onClick={() => setAdding(true)}>＋ Add widget</button>}</div>
      <div className="cycle-summary"><span>{activeCount ? `Full cycle · ${Math.floor(state.totalSec / 60)}m ${String(state.totalSec % 60).padStart(2, "0")}s` : "No active widgets · happy face fallback"}</span><span>↻</span></div>{activeCount > 0 && state.totalSec === 0 && <p className="section-intro">No configured widgets are ready. Showing the built-in face until a source is available.</p>}
    </section>
    <section className="display-settings"><div className="section-heading"><div><div className="eyebrow">THE LITTLE DETAILS</div><h2>Display</h2></div></div><div className="display-row"><label htmlFor="brightness">Brightness</label><input id="brightness" type="range" min="0" max="100" value={state.settings.brightness} onChange={e => setState({ ...state, settings: { ...state.settings, brightness: Number(e.target.value) } })} onPointerUp={e => act("set_brightness", { brightness: Number((e.target as HTMLInputElement).value) })} onKeyUp={e => act("set_brightness", { brightness: Number((e.target as HTMLInputElement).value) })} /><span>{state.settings.brightness}%</span></div><div className="display-row"><span>Screen</span><button onClick={() => act("set_display_on", { on: state.settings.manualDisplayOff })}>{state.settings.manualDisplayOff ? "Turn on" : "Turn off"}</button><button onClick={() => act("show_face")}>Show face</button></div></section>
    <button className="settings-link" onClick={() => setSettingsOpen(true)}>Settings · display, quiet hours and app</button>
    <footer>Clocks, timers and computer stats run locally. Weather by Open-Meteo; public quotes by Yahoo Finance. Unavailable live widgets are skipped in the cycle.</footer>
    {error && <div className="error" role="alert">{error}<button onClick={() => setError("")}>Dismiss</button></div>}
    {settingsOpen && <SettingsPanel preferences={state.settings.preferences} connection={connection} onClose={() => setSettingsOpen(false)} onApply={p => act("set_preferences",{preferences:p})} onReset={() => act("reset_deskbot")} />}
    {!state.settings.preferences.setupComplete && !settingsOpen && <Modal title="Welcome to DroidFun App" onClose={() => act("set_preferences",{preferences:{...state.settings.preferences,setupComplete:true}})}><section className="editor source-editor"><div className="eyebrow">HELLO, DROID FUN</div><h2>Your little desk companion</h2><p>Try the face and widget previews, or connect a robot with a USB data cable. A fresh playlist starts with the face; data widgets are opt-in.</p><button onClick={() => act("set_preferences",{preferences:{...state.settings.preferences,setupComplete:true}})}>Try preview</button><button onClick={async () => {await act("set_preferences",{preferences:{...state.settings.preferences,setupComplete:true}});await scanPorts();document.querySelector(".robot-connection")?.scrollIntoView({behavior:"instant"});}}>Connect robot</button></section></Modal>}
    {editor && <SourceEditor widget={editor} onClose={() => setEditor(null)} onApply={(label: string, config: SourceConfig, headline: string, footer: string) => act("configure_source", { id: editor.id, label, config, headline, footer })} onRemove={async () => { const ok = await act("remove_widget", { id: editor.id }); if (ok && selected === editor.id) setSelected(null); return ok; }} />}

  </main>;
}
