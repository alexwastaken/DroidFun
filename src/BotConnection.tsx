import { useState } from "react";
import { botName, botModels } from "./BotPicker";
import { BotAvatar } from "./BotPreview";
import LinuxRobotAccess from "./LinuxRobotAccess";

export type Connection = { state: string; detail: string; port: string | null; boardId: string | null; firmware: string | null; displayHealth: string | null; standaloneSupported: boolean; standaloneSaved: boolean; standaloneCount: number; identity?: { model: string; hardwareProfile: string; generation: number } | null; pinProfile?: string | null; provisioningSupported?: boolean; firmwareUpdateAvailable?: boolean; firmwareManaged?: boolean; firmwareRecoverySupported?: boolean };
export type RobotPort = { path: string; serial: string; product: string; vid: number; pid: number; connectable?: boolean };
export type SetupDrive = { path: string; name: string };
export type DroidProfile = { serial: string; name: string; model: string };
export type DisplayTestReport = { kind: string; version: number; boardId: string; uptime_ms: number; sda: number; scl: number; ack_3c: boolean; ack_3d: boolean; address: number | null; phase: string; write_ok: boolean; rgb_ready: boolean };

type Props = {
  model: string; selectedSerial: string | null; profiles: DroidProfile[]; connection: Connection | null; ports: RobotPort[]; setupDrives: SetupDrive[];
  busy: boolean; scanning: boolean; scanFinished: boolean; scanIssue: string; installing: boolean; setupNotice: string; setupError: string;
  testBoardId: string; testReport: DisplayTestReport | null; testIssue: string; readingTest: boolean; supportsDisplayTest: boolean; pendingModelSerial: string;
  onDiagnostics: () => void; onScan: () => Promise<void>; onConnect: (serial: string, model?: string) => void; onUpdate: (serial?: string) => void;
  onTest: (path?: string) => void; onReadTest: (serial: string) => void; onRename: (serial: string, name: string) => void;
};
const same = (a: string | null | undefined, b: string | null | undefined) => Boolean(a && b && a.toLowerCase() === b.toLowerCase());
function SetupChoice({ initial, name, disabled, onChoose }: { initial: string; name: string; disabled: boolean; onChoose: (model: string) => void }) {
  const [model, setModel] = useState(initial);
  return <div className="droid-add"><label>Droid model<select value={model} disabled={disabled} onChange={e => setModel(e.target.value)}><option value="">Choose your droid</option>{botModels.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}</select></label><button className="primary-control" disabled={disabled || !model} onClick={() => onChoose(model)}>{name}</button></div>;
}
function Rename({ profile, busy, onRename }: { profile: DroidProfile; busy: boolean; onRename: Props["onRename"] }) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(profile.name);
  return editing ? <form className="droid-name" onSubmit={e => {e.preventDefault(); onRename(profile.serial, name); setEditing(false);}}><label>Name your droid<input autoFocus maxLength={40} value={name} onChange={e => setName(e.target.value)} /></label><button disabled={busy || !name.trim()}>Save name</button><button type="button" onClick={() => setEditing(false)}>Cancel</button></form> : <button className="droid-rename" disabled={busy} onClick={() => {setName(profile.name);setEditing(true);}}>Rename</button>;
}
export default function BotConnection(p: Props) {
  const selectedPresent = p.ports.some(port => same(port.serial, p.selectedSerial));
  const ready = !p.installing && p.connection?.state === "connected" && same(p.connection.boardId, p.selectedSerial);
  const screenIssue = p.connection?.state === "display_error";
  const needsModel = Boolean(p.pendingModelSerial) || p.connection?.state === "needs_model";
  const modelSerial = p.pendingModelSerial || p.connection?.boardId;
  const discoveryError = p.connection?.state === "discovery_error";
  const choosingDevice = p.connection?.state === "choose_device";
  const deviceBusy = p.connection?.state === "device_busy";
  const selectedMissing = Boolean(p.selectedSerial) && !selectedPresent;
  const disconnected = p.connection?.state === "disconnected";
  const waitingForReply = p.connection?.state === "reconnecting" && selectedPresent;
  const managed = Boolean(p.connection?.firmwareManaged);
  const liveReply = ["connected", "display_error"].includes(p.connection?.state ?? "");
  const canRecover = liveReply && Boolean(p.connection?.firmwareRecoverySupported) && !managed && !p.setupDrives.length;
  const blocked = p.busy || p.installing;
  const phase = ready ? 3 : /verified|software sent|checking its connection|restarted/i.test(p.setupNotice) ? 2 : 1;
  const available = p.testBoardId ? [] : p.ports.filter(port => port.connectable !== false && !p.profiles.some(d => same(d.serial, port.serial)));
  const title = p.installing ? phase === 2 ? "Restarting your droid…" : "Getting your droid ready…" : p.setupError ? "Let’s try that again" : ready ? "Connected. Ready to play." : p.testBoardId ? "Screen test in progress" : screenIssue ? "Your droid’s screen needs attention" : p.connection?.state === "permission_needed" ? "One connection permission needed" : needsModel ? "We found your droid" : p.setupDrives.length ? "Your droid is in recovery mode" : discoveryError ? "We couldn’t check your droid" : p.connection?.state === "setup_needed" ? "Let’s check this connection" : deviceBusy ? "Your droid is in use" : choosingDevice ? "Choose a connected droid" : disconnected && selectedMissing ? "Your selected droid is unplugged" : waitingForReply ? "Waiting for your droid to respond" : p.ports.length ? "Checking your droid…" : "Plug in your droid";
  const description = p.installing ? "Keep USB connected while your droid restarts. We’ll reconnect it for you." : p.setupError ? "Keep your droid plugged in. The details below can help you choose the next step." : ready ? "Your screens are playing over USB. Reconnect this droid any time to pick up where you left off." : screenIssue ? "The droid responds over USB, but its screen needs attention. Reconnect the cable once, then check Help if it continues." : p.testBoardId ? "Check the pattern on the physical screen, then finish setup." : p.connection?.state === "permission_needed" ? "Allow USB access below, then we’ll check your droid again." : needsModel ? "Its software is installed. Choose its model once so we can show the right screens." : p.setupDrives.length ? "A droid shipped to you should already be prepared. Unplug it and reconnect normally first. If it stays here, open recovery help below." : discoveryError ? "Unplug and reconnect your droid, then choose Check connection. If it still cannot connect, open Help below." : deviceBusy ? "Quit DroidFun Workshop or any other app using this USB connection. We’ll try again automatically. Closing a window may leave an app running in the background." : choosingDevice ? selectedMissing ? "Your previously selected droid is not plugged in. Use the droid menu above to choose the one connected now; each droid keeps its own settings." : "Use the droid menu above to choose which plugged-in droid to edit." : disconnected && selectedMissing ? "Reconnect this droid with a USB data cable. Its saved screens and settings are still here." : waitingForReply ? "USB is present, but the droid has not replied yet. Quit Workshop, then reconnect the cable if this continues. You can also run diagnostics below." : p.connection?.state === "setup_needed" ? "We found a USB device, but it has not responded as a DroidFun droid yet." : p.ports.length ? "Reading its installed software. A prepared droid will connect automatically." : "Connect directly to this computer with a USB data cable. We look automatically; there’s no pairing step.";

  return <section id="robot-connection" className={`robot-connection${ready ? " connection-ready" : ""}`} aria-label="Droid connection">
    <div className="connection-heading"><span className={`connection-indicator${ready ? " is-ready" : ""}`} aria-hidden="true"/><span>Connection</span><button className="scan-button" aria-busy={p.scanning} disabled={blocked || p.scanning} onClick={p.onScan}>{p.scanning && <span className="usb-scan-spinner" aria-hidden="true"/>}{p.scanning ? "Looking…" : "Check connection"}</button></div>
    <div className="droid-status" role="status" aria-live="polite"><strong>{title}</strong><p>{description}</p>{ready && p.connection?.firmware && <small className="firmware-confirmation">✓ Droid software {p.connection.firmware} installed{p.connection.firmwareUpdateAvailable && !managed ? " · Update available below" : ""}</small>}{!ready && (p.scanning || p.scanFinished) && <small>{p.scanning ? "Looking for connected droids…" : p.scanIssue ? "The search couldn’t finish. Try again or open Help." : p.ports.length || p.setupDrives.length ? "USB device found" : "No USB data connection found. Try another cable or computer port."}</small>}</div>
    {p.setupError && <p className="connection-error" role="alert">{p.setupError}</p>}
    {p.installing && <ol className="droid-setup-steps" aria-label="Setup progress">{["Found", "Installing", "Restarting", "Ready"].map((text, i) => <li key={text} className={i < phase ? "is-complete" : i === phase ? "is-current" : ""}>{i < phase ? "✓ " : ""}{text}</li>)}</ol>}

    {p.ports.filter(port => needsModel && same(port.serial, modelSerial) && port.connectable !== false).map(port => <SetupChoice key={port.serial} initial="" name="Connect droid" disabled={blocked} onChoose={model => p.onConnect(port.serial, model)}/>)}
    {available.filter(port => !(needsModel && same(port.serial, modelSerial)) && (p.selectedSerial || choosingDevice || available.length > 1)).map((port, index) => <div className="new-droid" key={port.serial}><div><strong>{available.length > 1 ? `New droid ${index + 1}` : "Another droid is plugged in"}</strong><small>USB ID ending {port.serial.slice(-6)}</small></div><button className="secondary-control" disabled={blocked || p.scanning} onClick={() => p.onConnect(port.serial)}>Connect droid</button></div>)}
    {available.length > 1 && !needsModel && <p className="connection-detail">If you’re not sure which is which, connect one new droid at a time.</p>}
    {p.connection?.state === "setup_needed" && !p.testBoardId && !p.setupDrives.length && <div className="robot-update"><p>{managed ? "This droid uses factory-prepared software. Contact DroidFun support before attempting recovery." : "The USB device was found, but DroidFun software has not responded. Reconnect the cable and check again. If this is a new delivery, contact DroidFun support before reinstalling."}</p>{selectedPresent && canRecover && <button disabled={blocked} onClick={() => p.onUpdate(p.selectedSerial ?? undefined)}>Set up selected droid</button>}</div>}
    {!p.installing && !p.testBoardId && liveReply && selectedPresent && same(p.connection?.boardId, p.selectedSerial) && p.connection?.firmwareUpdateAvailable && !managed && <div className="robot-update"><strong>Droid software update available</strong><p>Keep USB connected during the update. Your saved screens in the app will be kept.</p><button disabled={blocked} onClick={() => p.onUpdate()}>Update droid software</button></div>}
    {p.testBoardId && <div className="display-test-card"><p>{p.readingTest ? "Reading the screen test…" : p.testReport?.write_ok ? "The screen is responding. Check that its pattern is visible." : "Check whether a pattern is visible on your droid’s screen."}</p>{canRecover ? <button disabled={blocked || p.readingTest} onClick={() => p.onUpdate(p.testBoardId)}>Finish screen test</button> : <p>Open Help below for support finishing your droid’s screen test.</p>}</div>}
    <LinuxRobotAccess permissionNeeded={p.connection?.state === "permission_needed"} onSetup={p.onScan}/>

    <button type="button" className="diagnostics-entry" disabled={blocked || p.scanning} onClick={p.onDiagnostics}>Run diagnostics</button>
    <details className="droid-help"><summary>{p.profiles.length > 1 ? "Your droids, help & connection details" : "Help & connection details"}</summary>
      <p>A USB cable can provide power without carrying data. If the screen has power but the app cannot find it, try another data cable and a direct computer port.</p>
      <p><a href="https://droidfun.io/downloads" target="_blank" rel="noreferrer">{p.setupDrives.length ? "Contact DroidFun support for recovery" : "Connection help & support"} ↗</a></p>
      {!!p.profiles.length && <div className="droid-list">{p.profiles.map(profile => {
        const port = p.ports.find(port => same(port.serial, profile.serial));
        const present = Boolean(port);
        const limited = port?.connectable === false;
        const active = same(profile.serial, p.selectedSerial);
        return <div key={profile.serial} className={`droid-card${active ? " is-selected" : ""}`}><div className="droid-card-heading"><BotAvatar model={profile.model}/><div><strong>{profile.name}</strong><small>{botName(profile.model)} · {limited ? "Waiting for a free slot" : active && ready ? "Connected" : present ? "Plugged in" : "Not plugged in"}</small></div>{!active && present && <button disabled={blocked || limited || p.scanning || Boolean(p.testBoardId)} onClick={() => p.onConnect(profile.serial)}>Use this droid</button>}</div>{active && <Rename key={profile.serial} profile={profile} busy={blocked} onRename={p.onRename}/>}</div>;
      })}</div>}
      {p.profiles.length > 1 && <p>Use the arrows at the top to switch between up to 4 plugged-in droids. Each keeps its own screens and display settings. The selected droid receives live screens; the others show saved screens or their fallback face.</p>}
      {p.connection && <dl className="connection-facts"><div><dt>Status</dt><dd>{deviceBusy ? "In use by another app" : p.connection.state.replaceAll("_", " ")}</dd></div><div><dt>Droid software</dt><dd>{p.connection.firmware ?? "Not checked"}</dd></div><div><dt>Screen</dt><dd>{p.connection.displayHealth ?? "Not checked"}</dd></div><div><dt>Board</dt><dd>{p.connection.boardId ?? "Not detected"}</dd></div></dl>}
      {p.connection?.detail && <p>{p.connection.detail}</p>}
      {p.scanIssue && <p>{p.scanIssue}</p>}{p.setupNotice && <p>{p.setupNotice}</p>}{p.testIssue && <p>{p.testIssue}</p>}
      {p.testReport && <p>Display test: {p.testReport.phase} · Address {p.testReport.address === null ? "none" : `0x${p.testReport.address.toString(16)}`}</p>}
      {managed ? <p>This droid uses factory-prepared firmware. Contact DroidFun support for software recovery.</p> : <>
        {!!p.setupDrives.length && <p>Your droid is in recovery mode. Unplug it and reconnect normally. If it stays here, contact DroidFun support so we can restore the right software for your hardware.</p>}
        {canRecover && !p.testBoardId && selectedPresent && <button disabled={blocked} onClick={() => p.onUpdate()}>Reinstall droid software</button>}
        {canRecover && p.supportsDisplayTest && !p.testBoardId && screenIssue && <button disabled={blocked || !selectedPresent} onClick={() => p.onTest()}>Run screen test</button>}
      </>}
      {p.testBoardId && <button disabled={blocked || p.readingTest} onClick={() => p.onReadTest(p.testBoardId)}>Read screen test result</button>}
    </details>
  </section>;
}
