import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { BotAvatar } from "./BotPreview";
import { failedInspectionAttempt, inspectionNeedsWiringConfirmation, nextInspectionAttempt, programmingBlocker, rememberedModel, type Inspection, type InspectionAttempt, type Wiring } from "./workshop-readiness";
import WorkshopRegistry from "./WorkshopRegistry";

type Board = { id: string; name: string; kind: string; serial: string | null };
type Scan = { boards: Board[]; consumerRunning: boolean; wipeSupported:boolean };
type Status = {
  phase: string; message: string; busy: boolean; firmwareVersion: string; model: string | null; boardId: string | null;
  wiring: Wiring;
  identity: { model: string; hardwareProfile: string; generation: number } | null;
  firmwareVerified: boolean; identityVerified: boolean; displayReady: boolean; disconnectedObserved: boolean;
  powerOffConfirmed: boolean; reconnectVerified: boolean; screenConfirmed: boolean;
};
const models = [{ id: "nimbus", name: "Nimbus", detail: "Cloud companion" }, { id: "boo", name: "Boo", detail: "Little ghost" }, { id: "meep", name: "Meep", detail: "Desk companion" }, { id: "claw", name: "Claw", detail: "AI usage display" }];
const modelName = (id: string | null | undefined) => models.find(m => m.id === id)?.name ?? "board";
const wiringLabel = (value: Wiring) => value === "mistake" ? "Mistake · SDA → GP5 · SCL → GP4" : "Standard · SDA → GP4 · SCL → GP5";
const inspectionTitle = (inspection: Inspection) => {
  const name = inspection.identity ? modelName(inspection.identity.model) : "Droid";
  return ({ current: `${name} already programmed`, update_available: `${name} already programmed · update available`, newer: `${name} already programmed · newer firmware`, unknown: `${name} already programmed · review firmware`, bootloader: "Ready to install firmware", error: "USB found · firmware could not be read" }[inspection.state] ?? "Checking this board");
};

export default function Workshop({section="prepare"}:{section?:string}) {
  const [view,setView]=useState(section);
  useEffect(()=>setView(section),[section]);
  const [status, setStatus] = useState<Status | null>(null);
  const [scan, setScan] = useState<Scan>({ boards: [], consumerRunning: false, wipeSupported:false });
  const [model, setModel] = useState(() => {
    try { return rememberedModel(localStorage.getItem("droidfun.workshop.model")); }
    catch { return ""; }
  });
  const [target, setTarget] = useState("");
  const [wiring, setWiring] = useState<Wiring>("standard");
  const [inspection, setInspection] = useState<Inspection | null>(null);
  const [inspectionStale, setInspectionStale] = useState(false);
  const [inspecting, setInspecting] = useState(false);
  const [wiringConfirmed, setWiringConfirmed] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [pending, setPending] = useState(false);
  const [wipeReview, setWipeReview] = useState<{target: string; model: string; wiring: Wiring} | null>(null);
  const [wipeConfirmed, setWipeConfirmed] = useState(false);
  const [error, setError] = useState("");
  const [scanError, setScanError] = useState("");
  const [closingConsumer, setClosingConsumer] = useState(false);
  const wipeTitle = useRef<HTMLHeadingElement>(null);
  useEffect(() => { if (wipeReview) { wipeTitle.current?.focus(); wipeTitle.current?.scrollIntoView({block:"start"}); } }, [wipeReview]);
  const current = useRef(status);
  const scanActive = useRef(false);
  const actionActive = useRef(false);
  const inspectionAttempt = useRef<InspectionAttempt | null>(null);
  current.current = status;

  useEffect(() => {
    if (rememberedModel(model)) {
      try { localStorage.setItem("droidfun.workshop.model", model); } catch { /* Selection still works without persistence. */ }
    }
  }, [model]);

  const scanBoards = useCallback(async (visible = false) => {
    if (actionActive.current || current.current?.busy) return;
    if (scanActive.current) { if (visible) setScanning(true); return; }
    scanActive.current = true;
    if (visible) setScanning(true);
    try {
      const found = await invoke<Scan>("workshop_scan");
      if (actionActive.current) return;
      setScan(found); setScanError("");
      const board = found.boards.length === 1 ? found.boards[0] : null;
      setTarget(previous => board?.id ?? (found.boards.some(b => b.id === previous) ? previous : ""));
      if (!board) {
        inspectionAttempt.current = null;
        setInspection(null);
        setInspectionStale(false);
        setWiringConfirmed(false);
      } else if (found.consumerRunning) {
        inspectionAttempt.current = null;
        // Another app taking USB control does not erase firmware that we
        // already read. Retain only this exact board's successful readback,
        // clearly marked as last checked; native write guards still apply.
        setInspection(previous => previous?.target === board.id && previous.firmware ? previous : null);
        setInspectionStale(true);
        setWiringConfirmed(false);
      } else if (["idle", "error"].includes(current.current?.phase ?? "idle")) {
        const attempt = nextInspectionAttempt(inspectionAttempt.current, board.id, Date.now(), visible);
        if (!attempt) return;
        const changedBoard = inspectionAttempt.current?.target !== board.id;
        inspectionAttempt.current = attempt;
        setInspecting(true); setInspection(null); setInspectionStale(false); setWiringConfirmed(false);
        // Never carry a repair wiring choice from one board to the next.
        if (changedBoard) setWiring("standard");
        try {
          const result = await invoke<Inspection>("workshop_inspect", { target: board.id });
          setInspection(result);
          if (result.identity) setModel(result.identity.model);
          if (result.wiring) setWiring(result.wiring);
        } catch (e) {
          inspectionAttempt.current = failedInspectionAttempt(attempt, Date.now());
          const retry = inspectionAttempt.current.retryAt !== null;
          setInspection({ target: board.id, state: "error", message: `${String(e)} Firmware may already be installed; this is not proof of a blank board. ${retry ? "Workshop will check again automatically in a moment." : "Close other apps using USB, then choose Check again. Reinstall only if you want to replace the board’s existing program."}`, firmware: null, boardId: board.serial, identity: null, wiring: null, displayHealth: null, startupSaved: false, readyToVerify: false });
        } finally { setInspecting(false); }
      }
    } catch (e) {
      setScanError(String(e)); setScan({ boards: [], consumerRunning: false, wipeSupported:false }); setTarget("");
      setInspection(null); setInspectionStale(false); inspectionAttempt.current = null;
    } finally { scanActive.current = false; setScanning(false); }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let received = false;
    void listen<Status>("workshop-progress", event => {
      received = true;
      if (!disposed) { current.current = event.payload; setStatus(event.payload); }
    }).then(fn => { if (disposed) fn(); else unlisten = fn; }).then(async () => {
      const saved = await invoke<Status>("workshop_status");
      if (!disposed && !received) { current.current = saved; setStatus(saved); }
    }).catch(e => { if (!disposed) setError(String(e)); });
    void scanBoards(true);
    const timer = window.setInterval(() => void scanBoards(), 1800);
    return () => { disposed = true; unlisten?.(); window.clearInterval(timer); };
  }, [scanBoards]);

  async function run(command: string, args: Record<string, unknown> = {}) {
    if (actionActive.current || current.current?.busy || inspecting) return;
    actionActive.current = true; setPending(true); setError("");
    try {
      const next = await invoke<Status>(command, args);
      current.current = next; setStatus(next);
    } catch (e) {
      setError(String(e));
      try { const next = await invoke<Status>("workshop_status"); current.current = next; setStatus(next); } catch { /* Keep the last progress state. */ }
    } finally { actionActive.current = false; setPending(false); void scanBoards(); }
  }

  async function closeConsumer() {
    actionActive.current = true; setClosingConsumer(true); setError("");
    try {
      await invoke("workshop_close_consumer");
      setScan(previous => ({ ...previous, consumerRunning: false }));
      inspectionAttempt.current = null;
    } catch (e) { setError(String(e)); }
    finally { actionActive.current = false; setClosingConsumer(false); void scanBoards(true); }
  }

  async function prepareAnother() {
    inspectionAttempt.current = null;
    setInspection(null); setInspectionStale(false); setWiring("standard"); setWiringConfirmed(false);
    await run("workshop_next");
  }

  const busy = pending || Boolean(status?.busy);
  const phase = status?.phase ?? "idle";
  const activeJob = !["idle", "error", "complete"].includes(phase);
  const editing = ["idle", "error"].includes(phase);
  const jobWiring = editing ? wiring : status?.wiring ?? wiring;
  const needsWiringConfirmation = inspectionNeedsWiringConfirmation(inspection, wiring);
  const blocker = programmingBlocker({ loaded: Boolean(status), model, target, boardIds: scan.boards.map(b => b.id), consumerRunning: scan.consumerRunning, scanError, busy: busy || closingConsumer, scanning, activeJob, phase, inspecting, inspectedTarget: inspectionStale ? "" : inspection?.target ?? "", wiringConfirmed: !needsWiringConfirmation || wiringConfirmed });
  const matchesInstalled = inspection?.identity?.model === model && inspection.wiring === wiring;
  const canCheckInstalled = blocker === null && inspection?.readyToVerify && matchesInstalled;
  const knownBoardPresent = scan.boards.length === 1 && scan.boards[0].serial?.toLowerCase() === status?.boardId?.toLowerCase();
  const canVerify = Boolean(status?.powerOffConfirmed && !busy && !scanning && !scan.consumerRunning && knownBoardPresent && ["awaiting_reconnect", "error"].includes(phase));
  const checks = [
    { label: "Firmware and board ID", passed: status?.firmwareVerified },
    { label: "Bot identity saved", passed: status?.identityVerified },
    { label: "Screen communication", passed: status?.displayReady },
    { label: "Power unplugged · confirmed by you", passed: status?.powerOffConfirmed },
    { label: "USB reconnect and saved face", passed: status?.reconnectVerified },
    { label: "Readable face · confirmed by you", passed: status?.screenConfirmed },
  ];
  const wipeReady = Boolean(wipeReview && wipeReview.target === target && wipeReview.model === model && wipeReview.wiring === wiring && blocker === null);
  const reviewReset = () => { setWipeReview({target,model,wiring}); setWipeConfirmed(false); };
  const wipeAction = <details className="workshop-reinstall"><summary>Wipe and reinstall</summary>
    {!scan.wipeSupported && <p>Full flash erase requires Workshop on Mac. To replace firmware here, use Reprogram below.</p>}
    <p>Start fresh: erase all firmware, saved identity and screens from the board’s 2 MiB flash, then install the selected model and wiring. The hardware board ID and private inventory record stay the same.</p>
    <button type="button" className="workshop-secondary" disabled={blocker !== null || !scan.wipeSupported} onClick={reviewReset}>Wipe and reinstall…</button>
  </details>;
  const programAction = <>
    <p>Install firmware {wiring === "mistake" ? "1.3.2-mistake" : "1.3.2"}, save the {modelName(model)} identity and add a startup face. Everything is included. This replaces the board’s firmware, identity and saved screens.</p>
    {inspection?.state === "newer" && <p className="workshop-warning">This will install an older firmware version. Use a newer Workshop if you want to keep this board’s current version.</p>}
    <button type="button" className="workshop-primary" disabled={blocker !== null} aria-describedby="program-readiness" onClick={() => void run("workshop_program", { target, model, wiring })}>{busy ? "Working…" : `${inspection?.firmware ? "Reprogram" : "Install firmware for"} ${modelName(model)}${wiring === "mistake" ? " · Mistake wiring" : ""}`}</button>
  </>;

  return <main className="workshop-shell">
    <div className="workshop-page-title"><span className="eyebrow">{view==='list'?'YOUR INVENTORY':'BOARD SETUP'}</span><h1>{view==='list'?'Your droids.':'Ready for a new desk.'}</h1><p>{view==='list'?'Saved board IDs and preparation checks.':'Check an installed droid, prepare a new board, or reinstall firmware.'}</p></div>
    <WorkshopRegistry visible={view==="list"} connectedIds={scan.boards.flatMap(board=>board.serial?[board.serial]:[])}/>
    <div hidden={view!=="prepare"}>
    {wipeReview && <section className="workshop-card workshop-wipe-review" role="dialog" aria-modal="false" aria-labelledby="wipe-title">
      <h2 id="wipe-title" ref={wipeTitle} tabIndex={-1}>Wipe this board and reinstall?</h2>
      <p>This permanently erases the board’s current firmware, identity and saved screens. Workshop then installs firmware {wipeReview.wiring === "mistake" ? "1.3.2-mistake" : "1.3.2"}, saves the {modelName(wipeReview.model)} identity and adds a fresh startup face.</p>
      <p>Board <code>{scan.boards.find(b => b.id === wipeReview.target)?.serial ?? wipeReview.target}</code><br/>{wiringLabel(wipeReview.wiring)}</p>
      <p>Your private inventory record and notes are kept. Preparation checks must be completed again.</p>
      <label className="workshop-confirm-wiring"><input type="checkbox" checked={wipeConfirmed} disabled={!wipeReady || busy} onChange={e => setWipeConfirmed(e.target.checked)}/>I checked this board and want to erase its stored contents.</label>
      {!wipeReady && <p className="workshop-warning">The board or settings changed, or USB is busy. Cancel and review the connected board again.</p>}
      <button type="button" className="workshop-primary" disabled={!wipeReady || !wipeConfirmed || busy} onClick={() => { const reviewed = wipeReview; setWipeReview(null); setWipeConfirmed(false); void run("workshop_wipe_reinstall", {target: reviewed.target, model: reviewed.model, wiring: reviewed.wiring, confirmedTarget: reviewed.target}); }}>Erase board and reinstall</button>
      <button type="button" className="workshop-secondary" disabled={busy} onClick={() => {setWipeReview(null);setWipeConfirmed(false);}}>Cancel wipe</button>
    </section>}
    <p className="workshop-intro">From bare board to ready-to-ship bot. Connect one board to see what’s installed, then prepare it and check the screen.</p>

    <section className="workshop-card" aria-labelledby="board-title">
      <div className="workshop-section-heading"><h2 id="board-title"><span>1</span> Connect a board</h2><button type="button" className="workshop-secondary" disabled={busy || scanning || inspecting || closingConsumer} onClick={() => void scanBoards(true)}>{scanning || inspecting ? "Checking…" : "Check again"}</button></div>
      <p>Plug in one board with a USB data cable. Workshop checks installed firmware automatically.</p>
      <label className="workshop-field">Connected board<select value={target} disabled={busy || inspecting || !scan.boards.length || activeJob || phase === "complete"} onChange={e => setTarget(e.target.value)}><option value="">{scan.boards.length ? "Choose a board" : scanning ? "Checking USB…" : "Waiting for a board"}</option>{scan.boards.map(board => <option value={board.id} key={board.id}>{board.name}</option>)}</select></label>
      {!scan.boards.length && !scanning && <p>For a bare board that isn’t detected, hold its BOOT button while plugging it in. To check existing firmware, reconnect normally without holding BOOT.</p>}
      {scan.boards.length > 1 && <p className="workshop-warning">More than one board is connected. Leave only the board you want to prepare.</p>}
      {scan.consumerRunning && <div className="workshop-warning" role="status"><strong>Switch USB control to Workshop</strong><p>DroidFun App is still running and may be using this board. Quit it so Workshop can identify the installed firmware. Closing its window alone may leave it running. Your droid’s program and saved screens are kept.</p><button type="button" className="workshop-secondary" disabled={busy || closingConsumer} onClick={() => void closeConsumer()}>{closingConsumer ? "Closing DroidFun App…" : "Close DroidFun App"}</button></div>}
      {editing && (inspecting || inspection) && <div className={`workshop-inspection ${inspection?.state === "current" ? "current" : ""}`} aria-live="polite">
        <strong>{inspecting ? "Reading installed firmware…" : `${inspectionStale ? "Last checked: " : ""}${inspectionTitle(inspection!)}`}</strong>
        <p>{inspecting ? "Reading this board’s details. Nothing is being installed or changed." : inspectionStale ? "These details were read before DroidFun App took USB control. Quit DroidFun App to check this board again or reset it." : inspection!.message}</p>
        {!inspecting && inspection && <>
          {inspection.firmware && <><dl className="workshop-device-facts"><div><dt>Firmware</dt><dd>{inspection.firmware}</dd></div><div><dt>Saved bot</dt><dd>{inspection.identity ? modelName(inspection.identity.model) : "Model not stored on this board"}</dd></div><div><dt>Screen wiring</dt><dd>{inspection.wiring ? wiringLabel(inspection.wiring) : "Not reported"}</dd></div><div><dt>Screen responds</dt><dd>{inspection.displayHealth === "ready" ? "Yes · inspect the physical screen" : "No · check wiring and screen"}</dd></div></dl><p>Keep this program to use the droid as it is, or reset and reinstall below to start fresh.</p><button type="button" className="workshop-secondary" disabled={blocker !== null || !scan.wipeSupported} onClick={reviewReset}>Reset and reinstall…</button>{!inspection.identity && <p>This firmware responds, but it does not report a saved bot model. Choose its model below; this does not mean its firmware is missing.</p>}</>}
          {inspection.boardId && <p className="workshop-board-id">Board ID <code>{inspection.boardId}</code></p>}
          {inspection.boardId&&inspection.firmware&&!inspection.identity&&model&&<button className="workshop-secondary" disabled={blocker !== null} onClick={async()=>{try{await invoke("workshop_record_existing",{target,model});setView("list");}catch(e){setError(String(e));}}}>Record existing {modelName(model)} droid</button>}
        </>}
      </div>}
      {scanError && <p className="workshop-warning" role="alert">{scanError}</p>}
    </section>

    <section className="workshop-card" aria-labelledby="model-title">
      <h2 id="model-title"><span>2</span> {inspection?.identity ? "Confirm the bot" : "Choose the bot"}</h2>
      {inspection?.identity && editing && <p>The saved identity selects {modelName(inspection.identity.model)} automatically. Choose another bot only when preparing this board for a different model.</p>}
      <div className="workshop-models">{models.map(bot => <button type="button" key={bot.id} aria-pressed={model === bot.id} disabled={busy || inspecting || activeJob || phase === "complete"} onClick={() => setModel(bot.id)}><BotAvatar model={bot.id}/><strong>{bot.name}</strong><small>{bot.detail}</small></button>)}</div>
      <details className="workshop-hardware" open={wiring === "mistake" || needsWiringConfirmation || undefined}><summary>Screen wiring · {wiring === "mistake" ? "Mistake" : "Standard"}</summary>
        <label className="workshop-field workshop-wiring">Wiring to install<select value={wiring} disabled={busy || inspecting || activeJob || phase === "complete"} onChange={e => { setWiring(e.target.value as Wiring); setWiringConfirmed(false); }}><option value="standard">Standard · SDA → GP4 · SCL → GP5</option><option value="mistake">Mistake · SDA → GP5 · SCL → GP4</option></select></label>
        <dl className="workshop-pin-map" aria-label="Selected screen connections"><div><dt>SDA · data</dt><dd>GP{wiring === "mistake" ? "5" : "4"}</dd></div><div><dt>SCL / SCK · clock</dt><dd>GP{wiring === "mistake" ? "4" : "5"}</dd></div></dl>
        {wiring === "mistake" && <p className="workshop-warning">For swapped SDA/SCL wires. Workshop uses the matching Mistake firmware. Customers use the regular DroidFun App.</p>}
        {needsWiringConfirmation && editing && <label className="workshop-confirm-wiring"><input type="checkbox" checked={wiringConfirmed} onChange={e => setWiringConfirmed(e.target.checked)}/>I checked the physical wires and want to install {wiringLabel(wiring)}.</label>}
      </details>
    </section>

    <section className="workshop-card" aria-labelledby="program-title">
      <h2 id="program-title"><span>3</span> {phase === "complete" ? "Ready for its owner" : "Prepare and verify"}</h2>
      {editing && !status?.powerOffConfirmed && <>
        {scan.consumerRunning ? <p>Quit DroidFun App with the button above. Workshop will then check the installed firmware and offer verification or reset options.</p> : <>{inspection?.readyToVerify && matchesInstalled ? <><p>This board already has the included firmware, its bot identity and a saved startup face. Keep them and run the power and screen checks.</p><button type="button" className="workshop-primary" disabled={!canCheckInstalled} onClick={() => void run("workshop_verify_installed", { target, model, wiring })}>Check this board for shipping</button><details className="workshop-reinstall"><summary>Reinstall or reset this board</summary>{programAction}</details></> : programAction}
        {wipeAction}</>}
        <p id="program-readiness" aria-live="polite">{blocker ?? `Ready for ${modelName(model)}. ${wiringLabel(wiring)}.`}</p>
      </>}
      {(phase !== "idle" || busy) && <div className={`workshop-status ${phase === "complete" ? "complete" : phase === "error" ? "failed" : ""}`} aria-live="polite" aria-atomic="true"><strong>{phase === "complete" ? "Board checked" : busy ? "Keep Workshop open and USB connected" : "Next step"}</strong><p>{status?.message ?? "Opening Workshop…"}</p></div>}
      {status?.boardId && <p className="workshop-board-id">{modelName(status.model)} · board <code>{status.boardId}</code><br/>{wiringLabel(jobWiring)} · Firmware {status.firmwareVersion}</p>}
      {phase !== "idle" && <ul className="workshop-checks" aria-label="Board checks">{checks.map(check => <li key={check.label} className={check.passed ? "passed" : ""}><span aria-hidden="true">{check.passed ? "✓" : "○"}</span>{check.label}<span className="workshop-sr-only">: {check.passed ? "passed" : "not confirmed"}</span></li>)}</ul>}
      {phase === "awaiting_unplug_confirmation" && <button type="button" className="workshop-primary" disabled={busy || scanning || scan.boards.length !== 0} onClick={() => void run("workshop_confirm_unplugged")}>All power is unplugged</button>}
      {(phase === "awaiting_reconnect" || phase === "error" && status?.powerOffConfirmed) && <button type="button" className="workshop-primary" disabled={!canVerify} onClick={() => void run("workshop_verify_reconnect")}>Verify reconnected board</button>}
      {phase === "awaiting_screen" && <><p>Look at the actual screen. Confirm only when you can see a clear, readable face on this board.</p><button type="button" className="workshop-primary" disabled={busy || scanning || !knownBoardPresent} onClick={() => void run("workshop_confirm_screen")}>I can see a readable face</button></>}
      {phase === "complete" && <><p className="workshop-handoff">The owner’s next steps: plug in their bot, download DroidFun App from droidfun.io, and open it. The saved identity tells the app which bot they have. Workshop stays with you.</p><button type="button" className="workshop-primary" disabled={busy} onClick={() => void prepareAnother()}>Prepare another board</button></>}
      {error && error !== status?.message && <p className="workshop-warning" role="alert">{error}</p>}
      {(activeJob || phase === "error") && !busy && <details className="workshop-reinstall"><summary>Start with a different board</summary><p>This board’s unfinished power and screen checks still need to be completed before shipping.</p><button type="button" className="workshop-secondary" onClick={() => void prepareAnother()}>Start a new job</button></details>}
    </section>
    </div><footer>Workshop 0.3.2 · Includes firmware 1.3.2 + Mistake<br/>RP2040-Zero · SSD1306 · Works offline</footer>
  </main>;
}
