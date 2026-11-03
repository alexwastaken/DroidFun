import { BotAvatar } from "./BotPreview";
import type { DroidProfile, RobotPort } from "./BotConnection";

const same = (a: string | null, b: string) => a?.toLowerCase() === b.toLowerCase();

export default function DroidSwitcher({ ports, profiles, selectedSerial, busy, onSelect }: {
  ports: RobotPort[]; profiles: DroidProfile[]; selectedSerial: string | null;
  busy: boolean; onSelect: (serial: string) => void;
}) {
  const differentDroidAttached = Boolean(selectedSerial) && ports.some(p => !same(selectedSerial, p.serial));
  if (ports.length < 2 && profiles.length < 2 && !differentDroidAttached) return null;
  const choices = ports.filter(p => p.connectable !== false);
  const index = choices.findIndex(p => same(selectedSerial, p.serial));
  const profile = profiles.find(p => same(selectedSerial, p.serial));
  const name = (serial: string) => profiles.find(p => same(serial, p.serial))?.name ?? "New droid";
  const label = (port: RobotPort) => {
    const title = name(port.serial);
    if (choices.filter(p => name(p.serial) === title).length < 2) return title;
    const tail = port.serial.slice(-6);
    const suffix = choices.some(p => p.serial !== port.serial && p.serial.slice(-6) === tail) ? port.serial : tail;
    return `${title} · ${suffix}`;
  };
  const step = (direction: number) => {
    const next = index < 0 ? (direction > 0 ? 0 : choices.length - 1) : (index + direction + choices.length) % choices.length;
    if (choices[next]) onSelect(choices[next].serial);
  };
  const disabled = busy || choices.length === 0 || (choices.length === 1 && index === 0);
  const overflow = ports.filter(p => p.connectable === false).length;
  return <section className="droid-switcher" aria-label="Droid switcher" aria-busy={busy}>
    <div className="droid-switcher-controls">
      <button type="button" aria-label="Previous droid" disabled={disabled} onClick={() => step(-1)}>‹</button>
      <div className="droid-switcher-choice">
        {profile && <BotAvatar model={profile.model}/>}
        <label><span>Your droids <small>{ports.length} {ports.length === 1 ? "droid" : "droids"} plugged in</small></span>
          <select aria-label="Selected droid" value={selectedSerial && index >= 0 ? choices[index].serial : ""} disabled={busy || !choices.length} onChange={e => {if (e.target.value) onSelect(e.target.value);}}>
            {index < 0 && <option value="" disabled>{profile ? `${profile.name} · ${ports.some(p => same(selectedSerial, p.serial)) ? "Waiting for a free slot" : "Not plugged in"}` : "Choose a droid"}</option>}
            {choices.map(p => <option key={p.serial} value={p.serial}>{label(p)}</option>)}
          </select>
        </label>
      </div>
      <button type="button" aria-label="Next droid" disabled={disabled} onClick={() => step(1)}>›</button>
    </div>
    {overflow > 0 ? <p role="status">Up to 4 droids at once. Unplug one to use {overflow === 1 ? "the extra droid" : "an extra droid"}.</p> : <p>Switch to edit a droid’s live screens. The others show their saved screens.</p>}
  </section>;
}
