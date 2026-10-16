import { useEffect, useLayoutEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import SettingsPanel, { type Preferences } from "./Settings";
import BotPicker, { botName, widgetOptions, widgetAllowed } from "./BotPicker";
import Oled from "./Oled";
import LinuxRobotAccess from "./LinuxRobotAccess";

type Playback = "running" | "paused" | "pinned";
import SourceEditor, { type Widget, type SourceConfig } from "./SourceEditor";
type DataCard = { sourceId: string; state: string; detail: string; fetchedAt: number; observedAt: number; expiresAt: number; headline: string; footer: string };
type Snapshot = { settings: { version: number; widgets: Widget[]; brightness: number; selectedDeviceSerial: string | null; preferences: Preferences; manualDisplayOff: boolean }; currentId: string; currentLabel: string; playback: Playback; remainingSec: number; totalSec: number; displayOn: boolean; temporary: boolean; displayReason: string; quietActive: boolean; quietOverrideUntil: number; frame: number[]; animationPhase: number; data: Record<string, DataCard> };
type Connection = { state: string; detail: string; port: string | null; boardId: string | null; firmware: string | null; displayHealth: string | null; standaloneSupported: boolean; standaloneSaved: boolean; standaloneCount: number };
type StandaloneResult = { saved: boolean; count: number; message: string };
type RobotPort = { path: string; serial: string; product: string; vid: number; pid: number };
type Editor = Widget;

const icons: Record<string, string> = { text: "Aa", face: "◡", weather: "☀", stock: "↗", codex: "⌘", claude: "✦", clock:"◷", pomodoro:"◴", meeting:"▣", cpu:"▥", memory:"▤" };
const subtitles: Record<string, string> = {
  text: "Your words, your screen",
  face: "A little company for your desk",
  clock: "Local clock", pomodoro: "Local focus timer", meeting: "Local countdown", cpu: "This computer’s CPU", memory: "This computer’s memory",
  weather: "Sample weather • connect later",
  stock: "Sample quote • connect later",
  codex: "Sample usage • connect later",
  claude: "Sample usage • connect later",
};


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
  const [addKind, setAddKind] = useState("text");
  const [choosingBot, setChoosingBot] = useState(false);
  const [savingToBot, setSavingToBot] = useState(false);
  const [savedToBot, setSavedToBot] = useState("");
  const [saveError, setSaveError] = useState("");
  const model = state?.settings.preferences.botModel ?? "";
  const offline = state?.settings.preferences.offlineMode ?? false;
  const standaloneDraftKey = JSON.stringify({ orientation: state?.settings.preferences.orientation, brightness: state?.settings.brightness, selectedDeviceSerial: state?.settings.selectedDeviceSerial, connectedBoard: connection?.boardId, connectedPort: connection?.port, widgets: state?.settings.widgets.filter(w => ["face", "text"].includes(w.kind)) });
  useEffect(() => { setSavedToBot(""); setSaveError(""); }, [standaloneDraftKey]);
  useEffect(() => { setSelected(null); setEditor(null); setAdding(false); setAddKind(model === "claw" ? "codex" : "text"); setSavedToBot(""); setSaveError(""); }, [model]);
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

  useLayoutEffect(() => {
    document.documentElement.dataset.branding = state?.settings.preferences.branding ?? "droidfun";
  }, [state?.settings.preferences.branding]);

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

  async function saveToBot() {
    setSavingToBot(true); setBusy(true); setSavedToBot(""); setSaveError("");
    try {
      const result = await invoke<StandaloneResult>("save_to_bot");
      if (result.saved) setSavedToBot(result.message || `${result.count} screens saved. Your bot is ready for a USB wall charger.`);
      else setSaveError(result.message || "The bot did not confirm the save. Keep it connected and try again.");
    } catch (e) { setSaveError(String(e)); }
    finally { setSavingToBot(false); setBusy(false); }
  }

  if (!state) return <main className="loading">Waking up DroidFun App…{error && <p role="alert">{error}</p>}</main>;
  if (!botName(model) || choosingBot) return <BotPicker current={model} busy={busy} error={error} onCancel={botName(model) ? () => setChoosingBot(false) : undefined} onSelect={async botModel => { if (await act("set_bot_model", { botModel })) { setChoosingBot(false); setSettingsOpen(false); } }} />;
  const widgets = state.settings.widgets.filter(w => widgetAllowed(model, w.kind));
  const selectedWidget = widgets.find(w => w.id === selected);
  const current = widgets.find(w => w.id === state.currentId);
  const activeCount = widgets.filter(w => w.enabled).length;
  const standaloneCount = widgets.filter(w => w.enabled && ["face", "text"].includes(w.kind)).length;
  const status = !state.displayOn ? state.displayReason : state.playback === "pinned" ? "Pinned" : state.playback === "paused" ? "Paused" : state.temporary ? "Showing now" : "Cycling";
  const visibleFrame = selected && preview ? preview : state.frame;
  const visibleWidget = selectedWidget ?? current;
  const liveCard = visibleWidget ? state.data?.[visibleWidget.id] : undefined;
  const isLocal = visibleWidget && ["text","clock","pomodoro","meeting","cpu","memory"].includes(visibleWidget.kind);
  const sourceLabel = !visibleWidget ? "" : offline && !isLocal && visibleWidget?.kind !== "face" && visibleWidget?.config.mode === "live" ? "paused offline" : visibleWidget?.kind === "face" ? "" : isLocal ? "local" : visibleWidget?.config.mode === "live" ? (liveCard?.state ?? "waiting") : "demo";

  return <main className="app-shell">
    <div className="topline"><div className="app-brand"><img src="/brand/droid-fun-wordmark.png" alt="Droid Fun" /><span>App</span></div><span className="demo-pill">{connection?.state === "connected" ? "ROBOT CONNECTED" : "PREVIEW MODE"}</span></div>
    <section className="intro"><div><div className="eyebrow">YOUR DROID, YOUR WAY</div><h1>{botName(model)}<br /><em>{model === "claw" ? "Keep usage in view." : "Make it yours."}</em></h1><p>{model === "claw" ? "Claude and Codex usage, right on your desk." : "Your words, favorite faces, and everyday updates."}</p></div></section>
    <section className="bot-controls" aria-label="Bot and connectivity"><div className="bot-control-row"><span>{botName(model)}</span><button disabled={busy} onClick={() => setChoosingBot(true)}>Change bot</button></div><label className="offline-control"><span><strong>Pause online data</strong><small>{offline ? "Internet requests paused · USB available" : "Internet sources available"}</small></span><input type="checkbox" checked={offline} disabled={busy} onChange={e => act("set_offline_mode", { offline: e.target.checked })}/></label>{offline && <p role="status">{model === "claw" ? "Live usage needs internet. Sample screens still work over USB." : "Text, faces, clocks, timers and computer stats work over USB. Live weather and stocks are paused."} Keep DroidFun App running and your bot connected.</p>}</section>
    <label className="branding-control">Branding<select value={state.settings.preferences.branding} disabled={busy} onChange={e => act("set_preferences", {preferences:{...state.settings.preferences,branding:e.target.value}})}><option value="droidfun">DroidFun · blue</option><option value="green">Classic · green</option></select></label>
    <section className="robot-wrap" aria-label="Robot screen preview">
      <div className="antenna"><span /></div>
      <div className="robot-head"><span className="head-highlight" /><div className="screen-bezel"><Oled frame={visibleFrame} brightness={state.displayOn || selected ? state.settings.brightness : 0} orientation={state.settings.preferences.orientation} description={`${selectedWidget?.label ?? current?.label ?? "Happy face"} OLED preview. ${liveCard?.headline ?? visibleWidget?.headline ?? ""}. ${liveCard?.footer ?? visibleWidget?.footer ?? ""}. ${liveCard?.detail ?? visibleWidget?.config.faceExpression ?? ""}`} /></div><div className="robot-cheek left" /><div className="robot-cheek right" /></div>
      <div className="robot-neck" />
    </section>
    <div className="screen-caption"><div><div className="eyebrow">ON THE SCREEN {sourceLabel && <span className="demo-text">· {sourceLabel.toUpperCase().replaceAll("_", " ")}</span>}</div><h2>{selectedWidget ? selectedWidget.label : (current?.label ?? state.currentLabel)}</h2><span className="caption-sub">{selectedWidget ? "Local preview only — cycle continues" : `${status}${state.playback === "running" && state.displayOn ? ` · ${state.remainingSec}s left` : ""}`}</span></div><span className={`status-dot ${state.displayOn ? "" : "off"}`} /></div>
    {visibleWidget?.config.mode === "live" && visibleWidget.kind !== "text" && visibleWidget.kind !== "face" && <div className="live-source-detail"><span>{offline && !isLocal ? "Live data is paused. Any preview values are saved data, not a current reading." : (liveCard?.detail ?? "Waiting for this source to be configured or refreshed.")}</span>{liveCard?.observedAt ? <small>{visibleWidget.kind === "stock" ? "Quote age" : "Report age"}: {reportAge(liveCard.observedAt)}</small> : null}{!isLocal && !offline && <button disabled={busy || offline} onClick={() => act("refresh_widget", { id: visibleWidget.id })}>Refresh within source limits</button>}</div>}
    {selected && <div className="preview-actions"><button onClick={() => setSelected(null)}>Back to live cycle</button><button disabled={!selectedWidget?.enabled || busy} onClick={() => act("show_now", { id: selected })}>Show now</button><button disabled={!selectedWidget?.enabled || busy} onClick={() => act("pin_widget", { id: selected })}>Pin</button></div>}
    {(state.quietActive || state.quietOverrideUntil > Date.now()/1000) && <div className="quiet-status" role="status"><span>{state.displayReason}</span>{state.quietActive && <button disabled={busy} onClick={() => act("override_quiet")}>Wake for 15 minutes</button>}</div>}
    <div className="playback-controls"><button className="primary-control" disabled={busy} onClick={() => act("toggle_pause")}>{state.playback === "running" ? "Ⅱ  Pause cycle" : "▶  Resume cycle"}</button><button className="secondary-control" disabled={busy} onClick={() => act("next_widget")}>Next  →</button></div>
    <section className="robot-connection" aria-label="Robot connection"><div className="section-heading"><div><div className="eyebrow">USB ROBOT</div><h2>Connection</h2></div><button className="scan-button" disabled={scanning} onClick={scanPorts}>{scanning ? "Scanning…" : "Scan USB"}</button></div>
      <p className={`connection-state ${connection?.state ?? "preview"}`}>{connection?.state.replaceAll("_", " ") ?? "Preview only"}</p>
      <p className="connection-detail">{connection?.detail ?? "Checking USB devices…"}</p>
      {connection?.boardId && <p className="connection-detail">Board {connection.boardId} · Firmware {connection.firmware} · OLED {connection.displayHealth}</p>}
      {ports.length === 0 ? <p className="connection-detail">No RP2040-Zero serial port found. A USB data cable is required.</p> : ports.map(port => <div className="port-row" key={port.serial}><div><strong>{port.product}</strong><small>{port.path} · {port.serial}</small></div><button disabled={busy} onClick={() => act("select_robot", { serial: port.serial })}>Connect</button></div>)}
      <LinuxRobotAccess permissionNeeded={connection?.state === "permission_needed"} onSetup={scanPorts} />
      {connection?.state === "setup_needed" && <p className="connection-detail">This device has not answered the DroidFun App protocol. It may be running a diagnostic or another sketch; install the DroidFun App firmware to control it here.</p>}
    </section>
    <section className="standalone-controls" aria-label="Use without a computer"><div className="section-heading"><div><div className="eyebrow">TAKE IT WITH YOU</div><h2>Use without a computer</h2></div></div>{model === "claw" ? <p>Live Claude and Codex usage needs DroidFun App running on your computer. Keep Claw Bot connected with a USB data cable.</p> : <><p>Save your enabled text and face screens to {botName(model)}, then plug it into a USB wall charger. The saved screens cycle on their own, even after you unplug and reconnect power.</p><p className="standalone-count">{standaloneCount} {standaloneCount === 1 ? "screen" : "screens"} ready to save · Text and faces only</p><p>Save up to 30 screens, each shown for 3–120 seconds. Faces save as still images. Clocks, weather, stocks and computer stats need the app running.</p>{connection?.state !== "connected" ? <p>Connect your bot by USB to save its screens.</p> : !connection.standaloneSupported ? <p>Update your bot’s firmware first. Open Settings → Robot → Install firmware manually, then reconnect.</p> : null}{connection?.state === "connected" && connection.standaloneSaved && !savedToBot && <p>{connection.standaloneCount} {connection.standaloneCount === 1 ? "screen is" : "screens are"} currently stored on your bot. Save again after making changes.</p>}<button className="primary-control" disabled={busy || savingToBot || connection?.state !== "connected" || !connection.standaloneSupported || !standaloneCount || standaloneCount > 30} onClick={saveToBot}>{savingToBot ? "Saving to your bot…" : "Save to bot"}</button>{savedToBot && <p role="status" className="source-notice">{savedToBot}</p>}{saveError && <p role="alert" className="source-error">{saveError}</p>}</>}</section>
    <section className="playlist"><div className="section-heading"><div><div className="eyebrow">YOUR PLAYLIST</div><h2>Widgets</h2></div><span className="count-pill">{activeCount} enabled</span></div>
      <p className="section-intro">Switch widgets on, change their order, and choose how long each appears.</p>
      <div className="widget-list">{widgets.map((widget, index) => <div className={`widget-row ${selected === widget.id ? "selected" : ""}`} key={widget.id} onDragOver={e => {if(dragging && !busy)e.preventDefault();}} onDrop={e => {e.preventDefault();if(dragging && !busy)act("reorder_widget",{id:dragging,before:widget.id});setDragging(null);}}>
        <button className="widget-main" onClick={() => setSelected(widget.id)} aria-label={`Preview ${widget.label}`}><span className={`widget-icon ${widget.kind}`}>{icons[widget.kind]}</span><span className="widget-copy"><strong>{widget.label}</strong><small>{offline && widget.config.mode === "live" && !["text","face","clock","pomodoro","meeting","cpu","memory"].includes(widget.kind) ? "Paused offline" : ["text","face","clock","pomodoro","meeting","cpu","memory"].includes(widget.kind) ? subtitles[widget.kind] : widget.config.mode === "live" ? (state.data?.[widget.id]?.state === "fresh" ? state.data[widget.id].detail : (state.data?.[widget.id]?.state.replaceAll("_", " ") ?? "Waiting for source")) : (subtitles[widget.kind] ?? "Sample widget")}</small></span></button>
        <label className="switch" aria-label={`Enable ${widget.label}`}><input type="checkbox" checked={widget.enabled} disabled={busy} onChange={e => act("set_enabled", { id: widget.id, enabled: e.target.checked })} /><span /></label>
        <div className="widget-tools"><button className="drag-handle" disabled={busy} draggable={!busy} onDragStart={e => {if(busy){e.preventDefault();return;}setDragging(widget.id);e.dataTransfer.setData("text/plain",widget.id);e.dataTransfer.effectAllowed="move";}} onDragEnd={() => setDragging(null)} title="Drag to reorder; use the arrow buttons with a keyboard" aria-label={`Drag ${widget.label} to reorder`}>⠿</button><span>Show for <select aria-label={`${widget.label} duration`} value={widget.durationSec} disabled={busy} onChange={e => act("set_duration", { id: widget.id, durationSec: Number(e.target.value) })}>{[3, 5, 8, 10, 12, 15, 20, 30, 45, 60, 90, 120].includes(widget.durationSec) ? null : <option value={widget.durationSec}>{widget.durationSec}s</option>}{[3, 5, 8, 10, 12, 15, 20, 30, 45, 60, 90, 120].map(n => <option key={n} value={n}>{n}s</option>)}</select></span><div><button title="Move up" aria-label={`Move ${widget.label} up`} disabled={busy || index === 0} onClick={() => act("move_widget", { id: widget.id, direction: -1 })}>↑</button><button title="Move down" aria-label={`Move ${widget.label} down`} disabled={busy || index === widgets.length - 1} onClick={() => act("move_widget", { id: widget.id, direction: 1 })}>↓</button><button className="edit-button" disabled={busy} onClick={() => setEditor(widget)}>Edit</button></div></div>
        {widget.kind === "pomodoro" && <div className="timer-controls" aria-label={`${widget.label} timer controls`}><span>{state.data?.[widget.id]?.detail ?? "Ready to focus"}</span><div><button disabled={busy} onClick={() => act("timer_action",{id:widget.id,action:widget.config.timerRunning ? "pause":"start"})}>{widget.config.timerRunning ? "Pause timer":"Start timer"}</button><button disabled={busy} onClick={() => act("timer_action",{id:widget.id,action:"reset"})}>Reset timer</button><button disabled={busy} onClick={() => act("timer_action",{id:widget.id,action:"skip"})}>Skip phase</button></div></div>}
      </div>)}</div>
      <div className="add-widget-controls">{adding ? <><select aria-label="New widget type" value={addKind} onChange={e => setAddKind(e.target.value)}>{widgetOptions(model).map(([kind, name]) => <option key={kind} value={kind}>{name}</option>)}</select><button disabled={busy || state.settings.widgets.length >= 30} onClick={async () => { setBusy(true); setError(""); try { const next = await invoke<Snapshot>("add_widget", { kind: addKind }); setState(next); setEditor(next.settings.widgets[next.settings.widgets.length - 1]); setAdding(false); } catch (e) { setError(String(e)); } finally { setBusy(false); } }}>Add</button><button onClick={() => setAdding(false)}>Cancel</button></> : <button onClick={() => setAdding(true)}>＋ Add widget</button>}</div>
      <div className="cycle-summary"><span>{activeCount ? `Full cycle · ${Math.floor(state.totalSec / 60)}m ${String(state.totalSec % 60).padStart(2, "0")}s` : "No active widgets · idle display"}</span><span>↻</span></div>{activeCount > 0 && state.totalSec === 0 && <p className="section-intro">No configured widgets are ready. Your bot shows its idle face until a screen is available.</p>}
    </section>
    <section className="display-settings"><div className="section-heading"><div><div className="eyebrow">THE LITTLE DETAILS</div><h2>Display</h2></div></div><div className="display-row"><label htmlFor="brightness">Brightness</label><input id="brightness" type="range" min="0" max="100" value={state.settings.brightness} onChange={e => setState({ ...state, settings: { ...state.settings, brightness: Number(e.target.value) } })} onPointerUp={e => act("set_brightness", { brightness: Number((e.target as HTMLInputElement).value) })} onKeyUp={e => act("set_brightness", { brightness: Number((e.target as HTMLInputElement).value) })} /><span>{state.settings.brightness}%</span></div><div className="display-row"><span>Screen</span><button onClick={() => act("set_display_on", { on: state.settings.manualDisplayOff })}>{state.settings.manualDisplayOff ? "Turn on" : "Turn off"}</button>{model !== "claw" && <button onClick={() => act("show_face")}>Show face</button>}</div></section>
    <button className="settings-link" onClick={() => setSettingsOpen(true)}>Settings · display, quiet hours and app</button>
    <footer>{model === "claw" ? "Claude and Codex show account allowance, not token counts. Unavailable live widgets are skipped." : "Text, faces, clocks, timers and computer stats run locally. Weather by Open-Meteo; quotes by Yahoo Finance."} Keep the app open and your bot connected by USB.</footer>
    {error && <div className="error" role="alert">{error}<button onClick={() => setError("")}>Dismiss</button></div>}
    {settingsOpen && <SettingsPanel preferences={state.settings.preferences} connection={connection} onChangeBot={() => { setSettingsOpen(false); setChoosingBot(true); }} onClose={() => setSettingsOpen(false)} onApply={p => act("set_preferences",{preferences:p})} onReset={() => act("reset_deskbot")} />}
    {editor && <SourceEditor widget={editor} offline={offline} orientation={state.settings.preferences.orientation} onClose={() => setEditor(null)} onApply={(label: string, config: SourceConfig, headline: string, footer: string) => act("configure_source", { id: editor.id, label, config, headline, footer })} onRemove={async () => { const ok = await act("remove_widget", { id: editor.id }); if (ok && selected === editor.id) setSelected(null); return ok; }} />}

  </main>;
}
