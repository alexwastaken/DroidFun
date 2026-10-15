import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type AccessStatus = { supported: boolean; repairAvailable: boolean; ruleInstalled: boolean };

export default function LinuxRobotAccess({ permissionNeeded, onSetup }: { permissionNeeded: boolean; onSetup: () => Promise<void> }) {
  const [status, setStatus] = useState<AccessStatus | null>(null);
  const [pending, setPending] = useState(false);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    invoke<AccessStatus>("linux_usb_status").then(value => { if (active) setStatus(value); }).catch(() => {});
    return () => { active = false; };
  }, []);
  if (!status?.supported) return null;
  async function setup() {
    setPending(true); setNotice(""); setError("");
    try {
      await invoke("setup_robot_access");
      setStatus(await invoke<AccessStatus>("linux_usb_status"));
      setNotice("USB access setup is complete. Reconnecting… If the robot does not connect, unplug it and plug it back in.");
      await onSetup();
    } catch (e) { setError(String(e)); }
    finally { setPending(false); }
  }
  return <div className="robot-access-setup">
    <p className="connection-detail">{permissionNeeded || !status.ruleInstalled ? "Set up USB access so your desktop can connect to the robot." : "USB access is installed automatically. If access stops working, you can repair it here."}</p>
    <button className="scan-button" type="button" disabled={pending || !status.repairAvailable} onClick={setup}>{pending ? "Waiting for approval…" : "Set up robot access"}</button>
    <p className="connection-detail">{status.repairAvailable ? "Your desktop may ask for an administrator password. You won’t need Terminal or a restart." : "Reinstall DroidFun using your software installer to enable this setup button."}</p>
    {notice && <p className="connection-detail" role="status">{notice}</p>}
    {error && <p className="connection-detail" role="alert">{error}</p>}
  </div>;
}
