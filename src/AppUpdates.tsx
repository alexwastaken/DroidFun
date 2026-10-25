import { useEffect, useId, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type UpdatePhase = "idle" | "unconfigured" | "unsupported" | "checking" | "up_to_date" | "available" | "downloading" | "ready" | "restarting" | "error";
type AppUpdateStatus = {
  currentVersion: string;
  platform: "macos" | "windows" | "linux";
  supported: boolean;
  phase: UpdatePhase;
  version: string | null;
  date: string | null;
  notes: string | null;
  downloadedBytes: number;
  totalBytes: number | null;
  readyToRestart: boolean;
  message: string | null;
};
type Operation = "check" | "download" | "restart" | null;
const activePhase = (phase?: UpdatePhase) => phase === "checking" || phase === "downloading" || phase === "restarting";
const releaseUrl = "https://droidfun.io/downloads";

// Keep this hook in App so progress and restart readiness survive closing Settings.
// Reading this status is local; only the Check button makes a network request.
export function useAppUpdates() {
  const [status, setStatus] = useState<AppUpdateStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [operation, setOperation] = useState<Operation>(null);
  const [error, setError] = useState("");
  const activeOperation = useRef<Operation>(null);
  const latestStatus = useRef(status);
  const revision = useRef(0);
  latestStatus.current = status;

  function receive(value: AppUpdateStatus) {
    revision.current += 1;
    latestStatus.current = value;
    setStatus(value);
  }

  async function reload() {
    setLoading(true);
    setError("");
    const startedAt = revision.current;
    try {
      const value = await invoke<AppUpdateStatus>("get_app_update_status");
      if (startedAt === revision.current) receive(value);
    } catch (e) { setError(String(e)); }
    finally { setLoading(false); }
  }

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void (async () => {
      try {
        const stop = await listen<AppUpdateStatus>("app-update-progress", event => {
          if (!disposed) receive(event.payload);
        });
        if (disposed) { stop(); return; }
        unlisten = stop;
      } catch { /* Command responses still provide the final status. */ }
      const startedAt = revision.current;
      try {
        const value = await invoke<AppUpdateStatus>("get_app_update_status");
        if (!disposed && startedAt === revision.current) receive(value);
      } catch (e) { if (!disposed) setError(String(e)); }
      finally { if (!disposed) setLoading(false); }
    })();
    return () => { disposed = true; unlisten?.(); };
  }, []);

  async function run(action: Exclude<Operation, null>, beforeRestart?: () => Promise<void>) {
    if (activeOperation.current || activePhase(latestStatus.current?.phase)) return;
    activeOperation.current = action;
    setOperation(action);
    setError("");
    try {
      if (action === "restart") {
        await beforeRestart?.();
        await invoke("restart_app_after_update");
      } else {
        receive(await invoke<AppUpdateStatus>(action === "check" ? "check_app_update" : "install_app_update"));
      }
    } catch (e) {
      setError(String(e));
      // Reconcile after failures, including a failed restart with a download still ready.
      try { receive(await invoke<AppUpdateStatus>("get_app_update_status")); } catch { /* Keep the last known status. */ }
    } finally {
      activeOperation.current = null;
      setOperation(null);
    }
  }

  return { status, loading, operation, error, busy: operation !== null || activePhase(status?.phase), reload,
    check: () => run("check"), download: () => run("download"), restart: (before?: () => Promise<void>) => run("restart", before) };
}

export type AppUpdatesController = ReturnType<typeof useAppUpdates>;

export default function AppUpdates({ updates, offline, pending, hasUnsavedSettings, beforeRestart, home = false, onResumeOnline }: {
  updates: AppUpdatesController;
  offline: boolean;
  pending: boolean;
  hasUnsavedSettings: boolean;
  beforeRestart: () => Promise<void>;
  home?: boolean;
  onResumeOnline?: () => void;
}) {
  const headingId = useId();
  const { status, loading, operation, error, busy } = updates;
  const phase = operation === "check" ? "checking" : operation === "download" && status?.phase !== "ready" ? "downloading" : operation === "restart" ? "restarting" : status?.phase;
  const ready = Boolean(status?.readyToRestart);
  const manual = status && (!status.supported || phase === "unsupported" || phase === "unconfigured");
  const total = status?.totalBytes;
  const percent = total && total > 0 ? Math.min(100, Math.max(0, Math.floor(status.downloadedBytes / total * 100))) : undefined;
  const downloaded = ((status?.downloadedBytes ?? 0) / (1024 * 1024)).toFixed(1);
  const date = status?.date ? new Date(status.date) : null;
  const releaseDate = date && !Number.isNaN(date.getTime()) ? date.toLocaleDateString() : "";
  let summary = "Check when you’re ready. Updates are never downloaded automatically.";
  if (!status) summary = loading ? "Reading app version…" : "Update controls could not be loaded.";
  else if (phase === "checking") summary = "Checking for updates…";
  else if (phase === "up_to_date") summary = "You’re up to date.";
  else if (phase === "available") summary = `DroidFun ${status.version} is available.`;
  else if (phase === "downloading") summary = percent === 100 ? "Download complete. Verifying the update…" : "Downloading update…";
  else if (phase === "restarting") summary = "Installing the update and restarting DroidFun…";
  else if (ready) summary = `DroidFun ${status.version} is ready to install.`;
  else if (phase === "unsupported" || status.platform === "linux") summary = "On Linux, download the latest package and install it with your package manager.";
  else if (phase === "unconfigured") summary = "Updates aren’t available from this copy of DroidFun. You can download the latest version below.";
  else if (phase === "error") summary = "The update could not finish. You can try again.";
  const issue = error || (phase === "error" ? status?.message : "");

  return <section className={`app-updates${home ? " home-app-updates" : ""}`} aria-labelledby={headingId} aria-busy={busy}>
    <div className="app-updates-heading"><h3 id={headingId}>App updates</h3>{status && <span>Version {status.currentVersion}</span>}</div>
    <p role="status">{summary}</p>
    {offline && status?.supported && !manual && !ready && phase !== "restarting" && <p className="app-update-note">Online data is paused. Resume it in the main controls to check for or download updates.</p>}
    {phase === "downloading" && <div className="app-update-progress"><progress max={100} value={percent} aria-label="Update download"/><span>{percent === undefined ? `${downloaded} MB downloaded` : `${percent}% · ${downloaded} MB downloaded`}</span><p>You can close Settings while the download continues.</p></div>}
    {ready && phase !== "restarting" && <p>Your saved settings will be kept. {hasUnsavedSettings ? "Your changes in Settings will be saved before restarting." : "Restart when you’re ready to install the update."}</p>}
    {status?.version && status.notes && <details className="app-update-notes"><summary>What’s new in {status.version}{releaseDate ? ` · ${releaseDate}` : ""}</summary><div>{status.notes}</div></details>}
    {issue && <p className="source-error" role="alert">{issue}</p>}
    <div className="app-update-actions">
      {offline && status?.supported && !manual && !ready && onResumeOnline && <button type="button" disabled={busy || pending} onClick={onResumeOnline}>Resume online data</button>}
      {!status && !loading && <button type="button" disabled={pending} onClick={() => void updates.reload()}>Try again</button>}
      {(manual || phase === "error") && <a href={releaseUrl} target="_blank" rel="noreferrer">Download the latest installer ↗</a>}
      {status?.supported && !manual && !ready && phase !== "restarting" && <>
        {(phase === "available" || phase === "downloading") && <button type="button" className="primary-control" disabled={busy || pending || offline} onClick={() => void updates.download()}>{phase === "downloading" ? "Downloading…" : "Download update"}</button>}
        {phase !== "downloading" && <button type="button" disabled={busy || pending || offline} onClick={() => void updates.check()}>{phase === "checking" ? "Checking…" : phase === "available" ? "Check again" : "Check for updates"}</button>}
      </>}
      {(ready || phase === "restarting") && <button type="button" className="primary-control" disabled={busy || pending} onClick={() => void updates.restart(beforeRestart)}>{phase === "restarting" ? "Restarting…" : hasUnsavedSettings ? "Save settings and restart" : "Restart to update"}</button>}
    </div>
  </section>;
}
