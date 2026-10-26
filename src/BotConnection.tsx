import { useState } from "react";
import { botName, botModels } from "./BotPicker";
import { BotAvatar } from "./BotPreview";
import LinuxRobotAccess from "./LinuxRobotAccess";

export type Connection = { state: string; detail: string; port: string | null; boardId: string | null; firmware: string | null; displayHealth: string | null; standaloneSupported: boolean; standaloneSaved: boolean; standaloneCount: number };
export type RobotPort = { path: string; serial: string; product: string; vid: number; pid: number };
export type SetupDrive = { path: string; name: string };
export type DroidProfile = { serial: string; name: string; model: string };
export type DisplayTestReport = { kind: string; version: number; boardId: string; uptime_ms: number; sda: number; scl: number; ack_3c: boolean; ack_3d: boolean; address: number | null; phase: string; write_ok: boolean; rgb_ready: boolean };

type Props = {
  model: string; selectedSerial: string | null; profiles: DroidProfile[]; connection: Connection | null; ports: RobotPort[]; setupDrives: SetupDrive[];
  busy: boolean; scanning: boolean; scanFinished: boolean; scanIssue: string; installing: boolean; setupNotice: string; setupError: string;
  testBoardId: string; testReport: DisplayTestReport | null; testIssue: string; readingTest: boolean; supportsDisplayTest: boolean;
  onScan: () => Promise<void>; onConnect: (serial: string, model?: string) => void; onInstall: (path: string, model: string) => void; onUpdate: (serial?: string) => void;
  onTest: (path?: string) => void; onReadTest: (serial: string) => void; onRename: (serial: string, name: string) => void;
};
const same = (a: string | null | undefined, b: string | null | undefined) => Boolean(a && b && a.toLowerCase() === b.toLowerCase());
function needsUpdate(version: string | null | undefined) {
  const parts = version?.match(/^(\d+)\.(\d+)\.(\d+)$/)?.slice(1).map(Number);
  return parts && (parts[0] < 1 || parts[0] === 1 && (parts[1] < 2 || parts[1] === 2 && parts[2] < 3));
}
function SetupChoice({ initial, name, disabled, onChoose }: { initial: string; name: string; disabled: boolean; onChoose: (model: string) => void }) {
  const [model, setModel] = useState(initial);
  return <div className="droid-add"><label>Which droid is this?<select value={model} disabled={disabled} onChange={e => setModel(e.target.value)}><option value="">Choose your droid</option>{botModels.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}</select></label><button className="primary-control" disabled={disabled || !model} onClick={() => onChoose(model)}>{name}</button></div>;
}
function Rename({ profile, busy, onRename }: { profile: DroidProfile; busy: boolean; onRename: Props["onRename"] }) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(profile.name);
  return editing ? <form className="droid-name" onSubmit={e => {e.preventDefault(); onRename(profile.serial, name); setEditing(false);}}><label>Name your droid<input autoFocus maxLength={40} value={name} onChange={e => setName(e.target.value)} /></label><button disabled={busy || !name.trim()}>Save name</button><button type="button" onClick={() => setEditing(false)}>Cancel</button></form> : <button className="droid-rename" disabled={busy} onClick={() => {setName(profile.name);setEditing(true);}}>Rename {profile.name}</button>;
}
export default function BotConnection(p: Props) {
  const selected = p.profiles.find(d => same(d.serial, p.selectedSerial));
  const selectedPresent = p.ports.some(port => same(port.serial, p.selectedSerial));
  const ready = !p.installing && p.connection?.state === "connected" && same(p.connection.boardId, p.selectedSerial);
  const screenIssue = p.connection?.state === "display_error";
  const phase = ready ? 3 : /verified|software sent|checking its connection|restarted/i.test(p.setupNotice) ? 2 : 1;
  const title = p.installing ? phase === 2 ? "Restarting your droid…" : "Getting your droid ready…" : p.setupError ? "Setup needs another try" : ready ? `${selected?.name ?? botName(p.model)} is ready` : p.testBoardId ? "Let’s check the screen" : screenIssue ? "Your droid’s screen needs attention" : p.setupDrives.length ? "Let’s get your droid ready" : p.scanning ? "Looking for your droid…" : selectedPresent ? "Connecting your droid…" : "Plug in your droid";
  const description = p.installing ? "Keep the cable connected. We’ll finish setup and reconnect for you." : p.setupError ? "Keep your droid plugged in and try again. Your saved screens in the app are kept. Open Help below if you need more details." : ready ? "Choose the screens you want below. Your droid reconnects automatically when it’s plugged back in." : screenIssue ? "Your droid is connected, but its screen hasn’t responded. Try reconnecting its power, or open Help below." : p.setupDrives.length ? "One quick setup, then you’re ready to choose your screens. Keep the case closed." : p.testBoardId ? "Check the pattern on your droid, then finish setup to return to your usual screens." : p.connection?.state === "permission_needed" ? "Allow this computer to connect to your droid below." : selectedPresent ? "Checking your droid. This usually takes a moment." : "Use a USB data cable connected to this computer. We’ll find it automatically. Installed droid software stays on the droid.";
  const available = p.testBoardId ? [] : p.ports.filter(port => !p.profiles.some(d => same(d.serial, port.serial)));
  return <section id="robot-connection" className="robot-connection" aria-label="Your droids">
    <div className="section-heading"><div><div className="eyebrow">YOUR DROID</div><h2>{p.profiles.length > 1 ? "Your droids" : "Connection"}</h2></div><button className="scan-button" aria-busy={p.scanning} disabled={p.busy || p.scanning || p.installing} onClick={p.onScan}>{p.scanning && <span className="usb-scan-spinner" aria-hidden="true" />}{p.scanning ? "Looking…" : "Find my bot"}</button></div>
    <div className={`droid-status${ready ? " is-ready" : ""}`} role="status" aria-live="polite"><strong>{title}</strong><p>{description}</p>{p.scanning || p.scanFinished ? <small>{p.scanning ? "Looking for connected droids…" : p.scanIssue ? "The search couldn’t finish. Try again, or open Help below." : p.setupDrives.length ? "Droid found · Ready for setup" : p.ports.length ? `${p.ports.length} ${p.ports.length === 1 ? "droid" : "droids"} found` : "No droid connection found. Try a different data cable or computer port."}</small> : null}</div>
    {(p.installing || p.setupNotice && ready) && <ol className="droid-setup-steps" aria-label="Setup progress">{["Droid found", "Getting ready", "Restarting", "Ready to use"].map((text, i) => <li key={text} className={i < phase ? "is-complete" : i === phase ? "is-current" : ""}>{i < phase ? "✓ " : ""}{text}</li>)}</ol>}

    {p.profiles.length > 1 && <p className="connection-detail">Choose one droid to control. Each remembers its own screens and display settings. Other powered droids keep their face or saved screens.</p>}
    <div className="droid-list">{p.profiles.map(profile => {
      const present = p.ports.some(port => same(port.serial, profile.serial));
      const active = same(profile.serial, p.selectedSerial);
      return <div key={profile.serial} className={`droid-card${active ? " is-selected" : ""}`}><div className="droid-card-heading"><BotAvatar model={profile.model}/><div><strong>{profile.name}</strong><small>{botName(profile.model)} · {active && ready ? "Connected · Selected" : active ? "Selected · Waiting for connection" : present ? "Plugged in" : "Not plugged in"}</small></div>{!active && present && <button disabled={p.busy || p.installing} onClick={() => p.onConnect(profile.serial)}>Use this droid</button>}</div>{active && <Rename key={profile.serial} profile={profile} busy={p.busy || p.installing} onRename={p.onRename}/>}</div>;
    })}</div>
    {available.map((port, i) => <div className="robot-setup" key={port.serial}><h3>{available.length > 1 ? `New droid ${i + 1}` : "Your droid was found"}</h3><p>{p.profiles.length ? "Add it once, then we’ll remember it for next time. To tell identical droids apart during first setup, connect one new droid at a time." : "Choose your droid and we’ll connect it for you."}</p><SetupChoice initial={p.profiles.length ? "" : p.model} name="Connect droid" disabled={p.busy || p.installing} onChoose={model => p.onConnect(port.serial, model)}/></div>)}
    {p.setupDrives.map((drive, i) => <div className="robot-setup" key={drive.path}><h3>{p.setupDrives.length > 1 ? `New droid ${i + 1}` : "Set up your droid"}</h3><p>We’ll put the DroidFun software on this droid and reconnect it. Keep it plugged in until setup finishes.</p>{p.setupDrives.length > 1 ? <p>For first setup, connect one new droid at a time so we prepare the right one.</p> : <SetupChoice initial={p.profiles.length ? "" : p.model} name="Set up droid" disabled={p.busy || p.installing} onChoose={model => p.onInstall(drive.path, model)}/>}</div>)}
    {!p.installing && !p.testBoardId && selectedPresent && same(p.connection?.boardId, p.selectedSerial) && needsUpdate(p.connection?.firmware) && <div className="robot-update"><p>A droid update is ready. It keeps a face visible while your computer sleeps.</p><button disabled={p.busy} onClick={() => p.onUpdate()}>Update droid</button></div>}
    {p.testBoardId && <div className="display-test-card"><p>{p.readingTest ? "Checking your droid’s screen…" : p.testReport?.write_ok ? "Your screen is responding. Check that its pattern is visible." : "Check whether a pattern is visible on your droid’s screen."}</p><button disabled={p.busy || p.installing || p.readingTest} onClick={() => p.onUpdate(p.testBoardId)}>Finish setup</button></div>}
    {p.connection?.state === "setup_needed" && selectedPresent && !p.testBoardId && <div className="robot-update"><p>Your droid needs a quick setup before the app can control it. This installs DroidFun software on the selected droid.</p><button disabled={p.busy || p.installing} onClick={() => p.onUpdate(p.selectedSerial ?? undefined)}>Finish setup</button></div>}
    <LinuxRobotAccess permissionNeeded={p.connection?.state === "permission_needed"} onSetup={p.onScan}/>
    <details className="droid-help"><summary>Help and connection details</summary><p>If your droid disappears, try a different USB data cable and a direct computer port. A cable can supply power without carrying data.</p><p>Only one droid receives the app’s live screens at a time. To add a new droid safely, connect one new droid during its first setup.</p>{p.connection && <p>Status: {p.connection.state}<br/>Details: {p.connection.detail}<br/>Board: {p.connection.boardId ?? "Not detected"}<br/>Software: {p.connection.firmware ?? "Not checked"}<br/>Screen: {p.connection.displayHealth ?? "Not checked"}</p>}{p.ports.map(port => <p key={port.serial}>{port.product}<br/>{port.path}<br/>{port.serial}</p>)}{p.scanIssue && <p>{p.scanIssue}</p>}{p.setupError && <p>{p.setupError}</p>}{p.setupNotice && <p>{p.setupNotice}</p>}{p.testIssue && <p>{p.testIssue}</p>}{p.testReport && <p>Display test: {p.testReport.phase} · Address {p.testReport.address === null ? "none" : `0x${p.testReport.address.toString(16)}`}</p>}
      {!p.testBoardId && selectedPresent && <button disabled={p.busy || p.installing} onClick={() => p.onUpdate()}>Reinstall droid software</button>}
      {p.supportsDisplayTest && !p.testBoardId && screenIssue && <button disabled={p.busy || p.installing || !selectedPresent} onClick={() => p.onTest()}>Run screen test</button>}
      {p.setupDrives.length === 1 && p.setupDrives[0].path.startsWith("picoboot:") && <button disabled={p.busy || p.installing} onClick={() => p.onTest(p.setupDrives[0].path)}>Install screen test</button>}
      {p.testBoardId && <button disabled={p.busy || p.readingTest || p.installing} onClick={() => p.onReadTest(p.testBoardId)}>Read screen test result</button>}
    </details>
  </section>;
}
