import SupportDiagnostics from "./SupportDiagnostics";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import SettingsPanel, { type Preferences } from "./Settings";
import BotPicker, { botName, widgetOptions, widgetAllowed } from "./BotPicker";
import Oled from "./Oled";
import BotPreview from "./BotPreview";
import BotConnection, { type Connection, type RobotPort, type SetupDrive, type DisplayTestReport, type DroidProfile } from "./BotConnection";
import { AppUpdateLink, useAppUpdates } from "./AppUpdates";
import DroidModel from "./DroidModel";
import DroidSwitcher from "./DroidSwitcher";

type Playback = "running" | "paused" | "pinned";
import SourceEditor, { type Widget, type SourceConfig } from "./SourceEditor";
type DataCard = { sourceId: string; state: string; detail: string; fetchedAt: number; observedAt: number; expiresAt: number; headline: string; footer: string };
type Snapshot = { settings: { version: number; widgets: Widget[]; brightness: number; selectedDeviceSerial: string | null; droids: DroidProfile[]; preferences: Preferences; manualDisplayOff: boolean }; currentId: string; currentLabel: string; playback: Playback; remainingSec: number; totalSec: number; displayOn: boolean; hostDisplayAsleep: boolean; temporary: boolean; displayReason: string; quietActive: boolean; quietOverrideUntil: number; frame: number[]; animationPhase: number; data: Record<string, DataCard> };
type StandaloneResult = { saved: boolean; count: number; message: string };
type Editor = Widget;

const icons: Record<string, string> = { text: "Aa", image: "▧", face: "◡", weather: "☀", stock: "↗", codex: "⌘", claude: "✦", clock:"◷", pomodoro:"◴", meeting:"▣", cpu:"▥", memory:"▤" };
const subtitles: Record<string, string> = {
  text: "Your words, your screen",
  image: "Your uploaded picture",
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
  const [actionBusy, setBusy] = useState(false);
  const [connection, setConnection] = useState<Connection | null>(null);
  const [ports, setPorts] = useState<RobotPort[]>([]);
  const [scanning, setScanning] = useState(false);
  const [scanFinished, setScanFinished] = useState(false);
  const [scanIssue, setScanIssue] = useState("");
  const [setupDrives, setSetupDrives] = useState<SetupDrive[]>([]);
  const [installing, setInstalling] = useState(false);
  const [setupNotice, setSetupNotice] = useState("");
  const [setupError, setSetupError] = useState("");
  const [pendingModelSerial, setPendingModelSerial] = useState("");
  const [testBoardId, setTestBoardId] = useState(() => { try { return localStorage.getItem("droidfun-display-test-board") ?? ""; } catch { return ""; } });
  const [testReport, setTestReport] = useState<DisplayTestReport | null>(null);
  const [testIssue, setTestIssue] = useState("");
  const [readingTest, setReadingTest] = useState(false);
  const [supportsDisplayTest, setSupportsDisplayTest] = useState(false);
  const [supportOpen,setSupportOpen] = useState(false);
  const [supportBusy,setSupportBusy] = useState(false);
  const [settingsSection, setSettingsSection] = useState("Display");
  const [settingsOpen, setSettingsOpen] = useState(() => Boolean((window as Window & { __deskbotOpenSettings?: boolean }).__deskbotOpenSettings));
  const [dragging, setDragging] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [addKind, setAddKind] = useState("text");
  const [choosingBot, setChoosingBot] = useState(false);
  const [savingToBot, setSavingToBot] = useState(false);
  const [savedToBot, setSavedToBot] = useState("");
  const [saveError, setSaveError] = useState("");
  const model = state?.settings.preferences.botModel ?? "";
  const resolvedDroid = useRef<string | null>(null);
  const recognizedModel = connection && ["connected", "display_error"].includes(connection.state)
    && Boolean(connection.boardId && state?.settings.selectedDeviceSerial)
    && connection.boardId?.toLowerCase() === state?.settings.selectedDeviceSerial?.toLowerCase()
    && connection.identity?.hardwareProfile === "rp2040-zero-gp4-gp5-ssd1306"
    && connection.identity.generation > 0 && botName(connection.identity.model) ? connection.identity.model : "";
  const offline = state?.settings.preferences.offlineMode ?? false;
  const appUpdates = useAppUpdates(Boolean(state) && !offline, actionBusy || installing || supportBusy);
  const busy = actionBusy || supportBusy || appUpdates.operation === "restart" || appUpdates.status?.phase === "restarting";
  const standaloneDraftKey = JSON.stringify({ orientation: state?.settings.preferences.orientation, brightness: state?.settings.brightness, selectedDeviceSerial: state?.settings.selectedDeviceSerial, connectedBoard: connection?.boardId, connectedPort: connection?.port, widgets: state?.settings.widgets.filter(w => ["face", "image"].includes(w.kind) || w.kind === "text" && w.config.textMotion !== "scroll") });
  useEffect(() => { setSavedToBot(""); setSaveError(""); }, [standaloneDraftKey]);
  useEffect(() => { setSelected(null); setEditor(null); setAdding(false); setAddKind(model === "claw" ? "codex" : "text"); setSavedToBot(""); setSaveError(""); }, [model, state?.settings.selectedDeviceSerial]);
  useEffect(() => {
    if (!state) return;
    const current = `${state.settings.selectedDeviceSerial?.toLowerCase() ?? ""}\0${model}`;
    const previous = resolvedDroid.current;
    resolvedDroid.current = current;
    // Keep an initial menu-bar Settings request, but never apply an old draft to a new droid.
    if (previous === null || previous === current) return;
    setSettingsOpen(false); setChoosingBot(false); setPendingModelSerial("");
    setSetupError(message => message === "Choose this droid’s model below to finish connecting it." ? "" : message);
  }, [model, state?.settings.selectedDeviceSerial]);
  useEffect(() => {
    if (pendingModelSerial && recognizedModel && connection?.boardId?.toLowerCase() === pendingModelSerial.toLowerCase()) {
      setPendingModelSerial("");
      setSetupError(message => message === "Choose this droid’s model below to finish connecting it." ? "" : message);
    }
  }, [pendingModelSerial, recognizedModel, connection?.boardId]);
  useEffect(() => { if (pendingModelSerial && !ports.some(p => p.connectable !== false && p.serial.toLowerCase() === pendingModelSerial.toLowerCase())) setPendingModelSerial(""); }, [ports, pendingModelSerial]);
  const previewKey = selected ? JSON.stringify({ widget: state?.settings.widgets.find(w => w.id === selected), data: state?.data?.[selected] }) : "";

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    invoke<Snapshot>("get_state").then(value => { if (!disposed) setState(value); }).catch(e => setError(String(e)));
    invoke<Connection>("get_connection").then(value => { if (!disposed) setConnection(value); }).catch(e => setSetupError(String(e)));
    invoke<RobotPort[]>("list_robot_ports").then(value => { if (!disposed) setPorts(value); }).catch(e => setSetupError(String(e)));
    invoke<SetupDrive[]>("list_setup_drives").then(value => { if (!disposed) setSetupDrives(value); }).catch(e => setSetupError(String(e)));
    invoke<boolean>("supports_display_test").then(value => { if (!disposed) setSupportsDisplayTest(value); }).catch(() => { /* Keep the test action hidden when support cannot be confirmed. */ });
    listen<Snapshot>("deskbot-state", event => { if (!disposed) setState(event.payload); }).then(fn => { if (disposed) fn(); else unlisten = fn; });
    let unlistenConnection: (() => void) | undefined;
    listen<Connection>("deskbot-connection", event => { if (!disposed) setConnection(event.payload); }).then(fn => { if (disposed) fn(); else unlistenConnection = fn; });
    let unlistenSetup: (() => void) | undefined;
    listen<string>("deskbot-firmware-setup", event => { if (!disposed) setSetupNotice(event.payload); }).then(fn => { if (disposed) fn(); else unlistenSetup = fn; });
    return () => { disposed = true; unlisten?.(); unlistenConnection?.(); unlistenSetup?.(); };
  }, []);

  useLayoutEffect(() => {
    // The selected bot supplies the controls' theme; keep the shared brand identity.
    // Legacy branding preferences remain stored for upgrade compatibility.
    document.documentElement.dataset.branding = "droidfun";
    if (botName(model) && !choosingBot) document.documentElement.dataset.bot = model;
    else delete document.documentElement.dataset.bot;
  }, [model, choosingBot]);

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
    if (scanning || installing) return;
    setScanning(true); setScanFinished(false); setScanIssue(""); setSetupError("");
    try {
      await invoke("rescan_robot");
      // Keep the acknowledgement visible even when enumeration finishes instantly.
      const [[serial, setup]] = await Promise.all([
        Promise.allSettled([invoke<RobotPort[]>("list_robot_ports"), invoke<SetupDrive[]>("list_setup_drives")]),
        new Promise<void>(resolve => window.setTimeout(resolve, 700)),
      ]);
      if (serial.status === "fulfilled") setPorts(serial.value);
      if (setup.status === "fulfilled") setSetupDrives(setup.value);
      if (serial.status === "rejected") throw serial.reason;
      if (setup.status === "rejected") throw setup.reason;
    }
    catch (e) { setScanIssue(String(e)); }
    finally { setScanning(false); setScanFinished(true); }
  }

  useEffect(() => {
    if (installing || scanning) return;
    let active = true;
    let pending = false;
    const timer = window.setInterval(() => {
      if (pending) return;
      pending = true;
      Promise.allSettled([invoke<RobotPort[]>("list_robot_ports"), invoke<SetupDrive[]>("list_setup_drives")])
        .then(([serial, setup]) => {
          if (!active) return;
          if (serial.status === "fulfilled") setPorts(serial.value);
          if (setup.status === "fulfilled") setSetupDrives(setup.value);
          if (serial.status === "fulfilled" && setup.status === "fulfilled") setScanIssue("");
        })
        .finally(() => { pending = false; });
    }, 2000);
    return () => { active = false; window.clearInterval(timer); };
  }, [installing, scanning]);

  async function connectDroid(serial: string, botModel?: string) {
    if (busy || installing || scanning || testBoardId) return;
    setBusy(true); setSetupError(""); setSetupNotice(""); setPendingModelSerial("");
    try { setState(await invoke<Snapshot>("select_robot", {serial, botModel: botModel ?? null})); }
    catch (e) {
      const message = String(e);
      if (message === "Choose this droid’s model below to finish connecting it.") setPendingModelSerial(serial);
      setSetupError(message);
    }
    finally { setBusy(false); }
  }

  async function installRobot(path: string, expectedBoardId?: string, botModel?: string) {
    setInstalling(true); setBusy(true); setSetupError("");
    setSetupNotice("Installing your bot’s software… Keep the USB cable connected while it restarts.");
    try {
      setState(await invoke<Snapshot>("install_robot_firmware", { path, expectedBoardId: expectedBoardId ?? null, botModel: botModel ?? null }));
      setSetupDrives([]);
      setPorts(await invoke<RobotPort[]>("list_robot_ports").catch(() => []));
      setSetupNotice("Your bot restarted. Checking its connection…");
      rememberTestBoard(""); setTestReport(null); setTestIssue("");
    } catch (e) { setSetupNotice(""); setSetupError(String(e)); }
    finally { setInstalling(false); setBusy(false); }
  }

  async function updateRobot(serial?: string) {
    setInstalling(true); setBusy(true); setSetupError("");
    setSetupNotice("Restarting your bot for its USB software update… Keep the case closed and USB connected.");
    try {
      const target = await invoke<{ path: string; boardId: string }>("prepare_robot_update", { serial: serial ?? null });
      await installRobot(target.path, target.boardId);
    } catch (e) { setSetupNotice(""); setSetupError(String(e)); }
    finally { setInstalling(false); setBusy(false); }
  }

  function rememberTestBoard(serial: string) {
    setTestBoardId(serial);
    try { if (serial) localStorage.setItem("droidfun-display-test-board", serial); else localStorage.removeItem("droidfun-display-test-board"); } catch { /* The current session can still restore the board. */ }
  }

  async function readTestReport(serial: string) {
    setReadingTest(true); setTestIssue("");
    try { setTestReport(await invoke<DisplayTestReport>("read_display_test", { serial })); }
    catch (e) { setTestReport(null); setTestIssue(String(e)); }
    finally { setReadingTest(false); }
  }

  async function startDisplayTest(setupPath?: string) {
    setInstalling(true); setBusy(true); setError(""); setTestIssue(""); setTestReport(null);
    setSetupNotice("Preparing the screen test… Keep USB connected and the case closed.");
    try {
      const target = setupPath ? { path: setupPath, boardId: null } : await invoke<{ path: string; boardId: string }>("prepare_display_test");
      if (target.boardId) rememberTestBoard(target.boardId);
      const serial = await invoke<string>("install_display_test", { path: target.path, expectedBoardId: target.boardId });
      rememberTestBoard(serial); setSetupDrives([]);
      setPorts(await invoke<RobotPort[]>("list_robot_ports").catch(() => []));
      setSetupNotice("");
      await readTestReport(serial);
    } catch (e) { setSetupNotice(""); setTestIssue(String(e)); }
    finally { setInstalling(false); setBusy(false); }
  }

  useEffect(() => {
    if (installing) return;
    if (setupNotice && connection?.state === "connected") setSetupNotice("Your bot is connected. Check its physical screen, then choose what to display below.");
    if (setupNotice && connection?.state === "display_error") setSetupNotice("USB setup is complete. Your bot responds, but its screen needs attention.");
    if (setupNotice && ["reconnecting", "disconnected", "choose_device", "device_busy", "preview", "setup_needed"].includes(connection?.state ?? "")) setSetupNotice("");
  }, [connection?.state, setupNotice, installing]);

  useEffect(() => {
    if (!selected) { setPreview(null); return; }
    let active = true;
    let timer: number;
    const scrolling = state?.settings.widgets.some(w => w.id === selected && w.kind === "text" && w.config.textMotion === "scroll");
    const refresh = async () => {
      try { const frame = await invoke<number[]>("preview_frame", { id: selected }); if (active) setPreview(frame); }
      catch(e) { if (active) setError(String(e)); return; }
      if(active && scrolling) timer = window.setTimeout(refresh, 100);
    };
    void refresh();
    return () => { active = false; window.clearTimeout(timer); };
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
  const openSettings = (section = "Display") => { setSettingsSection(section); setSettingsOpen(true); };
  const updateLink = <AppUpdateLink updates={appUpdates} onOpen={() => openSettings("App")}/>;
  const settingsPanel = <>{supportOpen && <SupportDiagnostics onBusy={setSupportBusy} onClose={() => setSupportOpen(false)} onReinstall={serial => void updateRobot(serial)}/>} {settingsOpen && <SettingsPanel recognizedModel={recognizedModel} initialSection={settingsSection} onDiagnostics={() => {setSettingsOpen(false);setSupportOpen(true);}} appUpdates={appUpdates} preferences={state.settings.preferences} connection={connection} onChangeBot={() => { setSettingsOpen(false); setChoosingBot(true); }} onClose={() => setSettingsOpen(false)} onApply={p => act("set_preferences",{preferences:p})} onReset={() => act("reset_deskbot")} />}</>;
  const connectionPanel = <BotConnection model={model} selectedSerial={state.settings.selectedDeviceSerial} profiles={(state.settings.droids ?? []).filter(d => botName(d.model))} connection={connection} ports={ports} setupDrives={setupDrives} busy={busy} scanning={scanning} scanFinished={scanFinished} scanIssue={scanIssue} installing={installing} setupNotice={setupNotice} setupError={setupError} testBoardId={testBoardId} testReport={testReport} testIssue={testIssue} readingTest={readingTest} supportsDisplayTest={supportsDisplayTest} pendingModelSerial={pendingModelSerial} onDiagnostics={() => setSupportOpen(true)} onScan={scanPorts} onConnect={(serial, botModel) => void connectDroid(serial, botModel)} onUpdate={serial => void updateRobot(serial)} onTest={path => void startDisplayTest(path)} onReadTest={serial => void readTestReport(serial)} onRename={(serial, name) => void act("rename_droid", {serial, name})}/>;
  const droidSwitcher = <DroidSwitcher ports={ports} profiles={state.settings.droids ?? []} selectedSerial={state.settings.selectedDeviceSerial} busy={busy || installing || scanning || Boolean(testBoardId)} onSelect={serial => void connectDroid(serial)}/>;
  if (choosingBot) return <><BotPicker selectedDroidName={(state.settings.droids ?? []).find(d => d.serial.toLowerCase() === state.settings.selectedDeviceSerial?.toLowerCase())?.name} footer={updateLink} current={model} busy={busy} error={error} onCancel={() => setChoosingBot(false)} onSelect={async botModel => { if (await act("set_bot_model", { botModel })) { setChoosingBot(false); setSettingsOpen(false); } }} />{settingsPanel}</>;
  if (!botName(model)) return <main className="app-shell welcome-shell">
    <header className="topline"><div className="app-brand"><img src="/brand/droid-fun-wordmark.png" alt="Droid Fun" /><span>App</span></div><button className="header-settings" onClick={() => openSettings("App")}>Settings</button></header>
    {droidSwitcher}
    <section className="intro welcome-intro"><DroidModel model={connection?.identity?.model ?? "nimbus"}/><div className="eyebrow">WELCOME HOME</div><h1>A little droid.<br/><em>A desk that feels like you.</em></h1><p>Plug your droid into this computer. We’ll find it and get its screens ready.</p></section>
    <ol className="welcome-steps" aria-label="Getting started"><li><span>1</span><div><strong>Plug in</strong><small>Use a USB data cable</small></div></li><li><span>2</span><div><strong>Meet your droid</strong><small>We’ll recognize it for you</small></div></li><li><span>3</span><div><strong>Make it yours</strong><small>Choose what’s on screen</small></div></li></ol>
    {connectionPanel}
    <div className="welcome-explore"><p>Just taking a look?</p><button className="text-action" disabled={busy || installing} onClick={() => setChoosingBot(true)}>Explore screens without a droid →</button></div>
    <footer><p>Keep DroidFun running for live screens. Your droid’s installed software stays with it when you unplug.</p>{updateLink}</footer>
    {settingsPanel}
    {error && <div className="error" role="alert">{error}<button onClick={() => setError("")}>Dismiss</button></div>}
  </main>;
  const widgets = state.settings.widgets.filter(w => widgetAllowed(model, w.kind));
  const selectedWidget = widgets.find(w => w.id === selected);
  const current = widgets.find(w => w.id === state.currentId);
  const activeCount = widgets.filter(w => w.enabled).length;
  const starterWidget = model === "claw" ? widgets.find(w => w.kind === "codex") ?? widgets[0] : widgets.find(w => w.kind === "face");
  const sampleUsage = model === "claw" ? widgets.find(w => w.enabled && w.config.mode === "demo") : undefined;


  const standaloneCount = widgets.filter(w => w.enabled && (["face", "image"].includes(w.kind) || w.kind === "text" && w.config.textMotion !== "scroll")).length;
  const idle = !selected && state.totalSec === 0 && state.playback !== "pinned" && !state.temporary;
  const standby = state.hostDisplayAsleep && state.displayOn;
  const screenTitle = selectedWidget?.label ?? (standby ? "Happy face" : idle ? "Idle display" : current?.label ?? state.currentLabel);
  const idleStatus = activeCount > 0 ? "Waiting for an available screen" : model === "claw" ? "Enable a Claude or Codex widget to begin" : "Enable a widget to begin";
  const status = !state.displayOn ? state.displayReason : standby ? "Your computer’s display is asleep" : idle ? idleStatus : state.playback === "pinned" ? "Pinned" : state.playback === "paused" ? "Paused" : state.temporary ? "Showing now" : "Cycling";
  const visibleFrame = selected && preview ? preview : state.frame;
  const visibleWidget = selectedWidget ?? (standby ? undefined : current);
  const liveCard = visibleWidget ? state.data?.[visibleWidget.id] : undefined;
  const isLocal = visibleWidget && ["text","image","clock","pomodoro","meeting","cpu","memory"].includes(visibleWidget.kind);
  const sourceLabel = !visibleWidget ? "" : offline && !isLocal && visibleWidget?.kind !== "face" && visibleWidget?.config.mode === "live" ? "paused offline" : visibleWidget?.kind === "face" ? "" : isLocal ? "local" : visibleWidget?.config.mode === "live" ? (liveCard?.state ?? "waiting") : "demo";

  return <main className="app-shell">
    <header className="topline"><div className="app-brand"><img src="/brand/droid-fun-wordmark.png" alt="Droid Fun" /><span>App</span></div><button className="header-settings" onClick={() => openSettings()}>Settings</button></header>
    {droidSwitcher}
    <section className="intro home-intro"><div><div className="eyebrow">YOUR DESK COMPANION</div><h1>{(state.settings.droids ?? []).find(d => d.serial === state.settings.selectedDeviceSerial)?.name || botName(model)}</h1><p>{model === "claw" ? "Your Claude and Codex usage, at a glance." : "A favorite face. A useful update. A little more you."}</p></div><DroidModel model={model}/></section>
    {connectionPanel}
    {offline && <div className="offline-banner"><span>Online data is paused. Local screens still work over USB.</span><button className="text-action" disabled={busy} onClick={() => act("set_offline_mode", { offline: false })}>Resume online data</button></div>}
    <section className="screen-studio" aria-label="Your droid screen">
    {sampleUsage && <div className="display-test-card"><strong>A sample screen is ready</strong><p>These allowance numbers are demo data. Connect your account to show your own Codex or Claude usage.</p><button className="edit-button" disabled={busy} onClick={() => setEditor({ ...sampleUsage, config: { ...sampleUsage.config, mode: "live", window: sampleUsage.kind === "claude" ? "five_hour" : sampleUsage.config.window } })}>Set up live usage</button></div>}
    <BotPreview model={model}><Oled frame={visibleFrame} brightness={state.displayOn || selected ? state.settings.brightness : 0} orientation={state.settings.preferences.orientation} description={`${screenTitle} OLED preview. ${liveCard?.headline ?? visibleWidget?.headline ?? ""}. ${liveCard?.footer ?? visibleWidget?.footer ?? ""}. ${liveCard?.detail ?? visibleWidget?.config.faceExpression ?? ""}`} /></BotPreview>
    <div className="screen-caption"><div><div className="eyebrow">{connection?.state === "connected" ? "ON YOUR DROID" : "SCREEN PREVIEW"} {sourceLabel && <span className="demo-text">· {sourceLabel.toUpperCase().replaceAll("_", " ")}</span>}</div><h2>{screenTitle}</h2><span className="caption-sub">{selectedWidget ? "Local preview only — cycle continues" : `${status}${!idle && state.playback === "running" && state.displayOn ? ` · ${state.remainingSec}s left` : ""}`}</span></div><span className={`status-dot ${state.displayOn ? "" : "off"}`} /></div>
    {visibleWidget?.config.mode === "live" && visibleWidget.kind !== "text" && visibleWidget.kind !== "image" && visibleWidget.kind !== "face" && <div className="live-source-detail"><span>{offline && !isLocal ? "Live data is paused. Any preview values are saved data, not a current reading." : (liveCard?.detail ?? "Waiting for this source to be configured or refreshed.")}</span>{liveCard?.observedAt ? <small>{visibleWidget.kind === "stock" ? "Quote age" : "Report age"}: {reportAge(liveCard.observedAt)}</small> : null}{!isLocal && !offline && <button disabled={busy || offline} onClick={() => act("refresh_widget", { id: visibleWidget.id })}>{["codex", "claude"].includes(visibleWidget.kind) ? "Refresh usage" : "Refresh data"}</button>}</div>}
    {visibleWidget?.config.mode === "live" && ["codex", "claude"].includes(visibleWidget.kind) && <p className="usage-refresh-note">{offline ? "Usage updates are paused. Turn off Pause online data to resume." : visibleWidget.kind === "claude" && visibleWidget.config.claudeSource !== "account" ? "Checks Claude Code’s latest report every 2 seconds. New usage appears when Claude Code writes a report." : visibleWidget.kind === "codex" ? "Codex usage updates automatically about every minute while the app is online. Refreshes may slow down if Codex asks us to wait." : "Claude usage updates automatically about every 5 minutes while the app is online. Checking again respects your account’s refresh limits."}</p>}
    {selected && <div className="preview-actions"><button onClick={() => setSelected(null)}>Back to live cycle</button><button disabled={!selectedWidget?.enabled || busy} onClick={() => act("show_now", { id: selected })}>Show now</button><button disabled={!selectedWidget?.enabled || busy} onClick={() => act("pin_widget", { id: selected })}>Pin</button></div>}
    {(state.quietActive || state.quietOverrideUntil > Date.now()/1000) && <div className="quiet-status" role="status"><span>{state.displayReason}</span>{state.quietActive && <button disabled={busy} onClick={() => act("override_quiet")}>Wake for 15 minutes</button>}</div>}
    <div className="playback-controls"><button className="primary-control" disabled={busy} onClick={() => act("toggle_pause")}>{state.playback === "running" ? "Ⅱ  Pause cycle" : "▶  Resume cycle"}</button><button className="secondary-control" disabled={busy} onClick={() => act("next_widget")}>Next  →</button></div>
    </section>
    <section className="playlist"><div className="section-heading"><div><div className="eyebrow">MAKE IT YOURS</div><h2>Your screens</h2></div><span className="count-pill">{activeCount} enabled</span></div>
      <p className="section-intro">Turn screens on, make them your own with Edit, and choose how long each stays.</p>
      {activeCount === 0 && <div className="display-test-card"><p>{model === "claw" ? "Enable an allowance screen to start. Use Edit to connect an account or choose sample data." : "Your playlist is switched off. Enable a face to give your bot a screen you can also save for use without the app."}</p>{starterWidget ? <button className="primary-control" disabled={busy} onClick={() => act("set_enabled", { id: starterWidget.id, enabled: true })}>{model === "claw" ? `Enable ${starterWidget.label}` : "Enable a face"}</button> : model !== "claw" && <button className="primary-control" disabled={busy || state.settings.widgets.length >= 30} onClick={() => act("add_widget", { kind: "face" })}>Add a face</button>}</div>}
      <div className="widget-list">{widgets.map((widget, index) => <div className={`widget-row ${selected === widget.id ? "selected" : ""}`} key={widget.id} onDragOver={e => {if(dragging && !busy)e.preventDefault();}} onDrop={e => {e.preventDefault();if(dragging && !busy)act("reorder_widget",{id:dragging,before:widget.id});setDragging(null);}}>
        <button className="widget-main" onClick={() => setSelected(widget.id)} aria-label={`Preview ${widget.label}`}><span className={`widget-icon ${widget.kind}`}>{icons[widget.kind]}</span><span className="widget-copy"><strong>{widget.label}</strong><small>{offline && widget.config.mode === "live" && !["text","image","face","clock","pomodoro","meeting","cpu","memory"].includes(widget.kind) ? "Paused offline" : ["text","image","face","clock","pomodoro","meeting","cpu","memory"].includes(widget.kind) ? (widget.kind === "text" ? `${widget.config.textMotion === "scroll" ? "Scrolling" : "Still"} · ${widget.config.customText.replaceAll("\n", " ")}` : subtitles[widget.kind]) : widget.config.mode === "live" ? (state.data?.[widget.id]?.state === "fresh" ? state.data[widget.id].detail : (state.data?.[widget.id]?.state.replaceAll("_", " ") ?? "Waiting for source")) : (subtitles[widget.kind] ?? "Sample widget")}</small></span></button>
        <label className="switch" aria-label={`Enable ${widget.label}`}><input type="checkbox" checked={widget.enabled} disabled={busy} onChange={e => act("set_enabled", { id: widget.id, enabled: e.target.checked })} /><span /></label>
        <div className="widget-tools"><button className="drag-handle" disabled={busy} draggable={!busy} onDragStart={e => {if(busy){e.preventDefault();return;}setDragging(widget.id);e.dataTransfer.setData("text/plain",widget.id);e.dataTransfer.effectAllowed="move";}} onDragEnd={() => setDragging(null)} title="Drag to reorder; use the arrow buttons with a keyboard" aria-label={`Drag ${widget.label} to reorder`}>⠿</button><span>Show for <select aria-label={`${widget.label} duration`} value={widget.durationSec} disabled={busy} onChange={e => act("set_duration", { id: widget.id, durationSec: Number(e.target.value) })}>{[3, 5, 8, 10, 12, 15, 20, 30, 45, 60, 90, 120].includes(widget.durationSec) ? null : <option value={widget.durationSec}>{widget.durationSec}s</option>}{[3, 5, 8, 10, 12, 15, 20, 30, 45, 60, 90, 120].map(n => <option key={n} value={n}>{n}s</option>)}</select></span><div><button title="Move up" aria-label={`Move ${widget.label} up`} disabled={busy || index === 0} onClick={() => act("move_widget", { id: widget.id, direction: -1 })}>↑</button><button title="Move down" aria-label={`Move ${widget.label} down`} disabled={busy || index === widgets.length - 1} onClick={() => act("move_widget", { id: widget.id, direction: 1 })}>↓</button><button className="edit-button" disabled={busy} onClick={() => setEditor(widget)}>Edit</button></div></div>
        {widget.kind === "pomodoro" && <div className="timer-controls" aria-label={`${widget.label} timer controls`}><span>{state.data?.[widget.id]?.detail ?? "Ready to focus"}</span><div><button disabled={busy} onClick={() => act("timer_action",{id:widget.id,action:widget.config.timerRunning ? "pause":"start"})}>{widget.config.timerRunning ? "Pause timer":"Start timer"}</button><button disabled={busy} onClick={() => act("timer_action",{id:widget.id,action:"reset"})}>Reset timer</button><button disabled={busy} onClick={() => act("timer_action",{id:widget.id,action:"skip"})}>Skip phase</button></div></div>}
      </div>)}</div>
      <div className="add-widget-controls">{adding ? <><select aria-label="New screen type" value={addKind} onChange={e => setAddKind(e.target.value)}>{widgetOptions(model).map(([kind, name]) => <option key={kind} value={kind}>{name}</option>)}</select><button disabled={busy || state.settings.widgets.length >= 30} onClick={async () => { setBusy(true); setError(""); try { const next = await invoke<Snapshot>("add_widget", { kind: addKind }); setState(next); setEditor(next.settings.widgets[next.settings.widgets.length - 1]); setAdding(false); } catch (e) { setError(String(e)); } finally { setBusy(false); } }}>Add</button><button onClick={() => setAdding(false)}>Cancel</button></> : <button onClick={() => setAdding(true)}>＋ Add a screen</button>}</div>
      <div className="cycle-summary"><span>{activeCount ? `Full cycle · ${Math.floor(state.totalSec / 60)}m ${String(state.totalSec % 60).padStart(2, "0")}s` : "No active widgets · idle display"}</span><span>↻</span></div>{activeCount > 0 && state.totalSec === 0 && <p className="section-intro">No configured widgets are ready. Your bot shows its idle face until a screen is available.</p>}
    </section>
    <details className="standalone-controls"><summary><span>Use without a computer</span><small>Save still text, images and faces to your droid</small></summary><p>With the latest bot software, your bot keeps a face or its saved screens on while powered. When your computer wakes, the app reconnects automatically and resumes your widgets. Keep DroidFun App running in the background.</p>{model === "claw" ? <p>Live Claude and Codex usage needs DroidFun App running on your computer. Keep Claw Bot connected with a USB data cable.</p> : <><p>Save your enabled still text, image and face screens to {botName(model)}, then plug it into a USB wall charger. The saved screens cycle on their own, even after you unplug and reconnect power.</p><p className="standalone-count">{standaloneCount} {standaloneCount === 1 ? "screen" : "screens"} ready to save · Still text, images and faces</p><p>Save up to 30 screens, each shown for 3–120 seconds. Faces save as still images. Scrolling text, clocks, weather, stocks and computer stats need the app running.</p>{connection?.state !== "connected" ? <p>Connect your bot by USB to save its screens.</p> : !connection.standaloneSupported ? <p>{connection.firmwareManaged ? "Saving screens is not supported by this droid’s current software. Keep the app running to display your screens." : "This droid needs compatible software to save screens. Check Connection for an available update."}</p> : null}{connection?.state === "connected" && connection.standaloneSaved && !savedToBot && <p>{connection.standaloneCount} {connection.standaloneCount === 1 ? "screen is" : "screens are"} currently stored on your bot. Save again after making changes.</p>}<button className="primary-control" disabled={busy || savingToBot || connection?.state !== "connected" || !connection.standaloneSupported || !standaloneCount || standaloneCount > 30} onClick={saveToBot}>{savingToBot ? "Saving to your bot…" : "Save to bot"}</button>{savedToBot && <p role="status" className="source-notice">{savedToBot}</p>}{saveError && <p role="alert" className="source-error">{saveError}</p>}</>}</details>
    <section className="display-settings"><div className="section-heading"><div><div className="eyebrow">THE LITTLE DETAILS</div><h2>Display</h2></div></div><div className="display-row"><label htmlFor="brightness">Brightness</label><input id="brightness" type="range" min="0" max="100" value={state.settings.brightness} onChange={e => setState({ ...state, settings: { ...state.settings, brightness: Number(e.target.value) } })} onPointerUp={e => act("set_brightness", { brightness: Number((e.target as HTMLInputElement).value) })} onKeyUp={e => act("set_brightness", { brightness: Number((e.target as HTMLInputElement).value) })} /><span>{state.settings.brightness}%</span></div><div className="display-row"><span>Screen</span><button onClick={() => act("set_display_on", { on: state.settings.manualDisplayOff })}>{state.settings.manualDisplayOff ? "Turn on" : "Turn off"}</button>{model !== "claw" && <button onClick={() => act("show_face")}>Show face</button>}</div></section>
    <footer><p>{model === "claw" ? "Live usage needs the app running and a USB connection. Claude and Codex show account allowance, not token counts." : "Keep the app running and your droid connected for live screens. Text, faces, clocks and timers work locally."}</p>{updateLink}</footer>
    {error && <div className="error" role="alert">{error}<button onClick={() => setError("")}>Dismiss</button></div>}
    {settingsPanel}
    {editor && <SourceEditor model={model} widget={editor} offline={offline} orientation={state.settings.preferences.orientation} onClose={() => setEditor(null)} onApply={(label: string, config: SourceConfig, headline: string, footer: string) => act("configure_source", { id: editor.id, label, config, headline, footer })} onRemove={async () => { const ok = await act("remove_widget", { id: editor.id }); if (ok && selected === editor.id) setSelected(null); return ok; }} />}

  </main>;
}
