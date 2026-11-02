# Workshop 0.4.5

Workshop 0.4.1 and earlier cannot update themselves. Download the Mac or Windows Workshop package once from https://droidfun.io/admin#downloads, then replace the previous app/run the installer. Keep the same app identity; pairing, inventory and screen settings are retained.

In 0.4.2 and later, open Updates → Check for updates. If a newer release is available, choose Download update, then Install and restart. Opening Updates releases the Screens USB connection and stops a Live text bridge. Board preparation must finish before installing. Normal Quit cannot interrupt bundle replacement. An available update is never installed without the explicit restart action.

Updates use the existing revocable Workshop pairing, held in native storage. Connect in Droid list first if needed. The manifest and packages are served only through authenticated `/api/workshop/updates/` routes. A separate Workshop Minisign public key is embedded in the private build; its signed version must match the manifest, downgrades are rejected, package size/time are bounded, redirects are disabled, and download URLs are restricted to the same two private DroidFun endpoints. The webview cannot supply a URL, key, version, signature or file path. Public companion configuration and its release feed are unchanged.

On Screens, turn individual cards on/off, set 3–120 seconds and use ↑/↓ to change their order. Play enabled screens starts/resumes the playlist; selecting a screen holds it until playback resumes. At least one screen stays enabled. The enabled flags, durations and order are saved in the existing settings file and restored on the next launch. Disabled screens remain editable and keep their contents. Saving scrolling text can extend its time enough for one pass, matching the companion’s screen editor.

## Clock and Calendar

Workshop 0.4.3 uses a time-only Clock with 12-hour AM/PM or 24-hour display. Calendar is a separate screen showing the full month and day, with a weekday, year or empty bottom line. Both use the computer’s real time (or the existing configured time zone); they do not use simulated data. The public companion renderer is unchanged.

Calendar is added next to Clock and starts off, preserving the existing enabled playlist. Turn it on in Screens and set its duration/order like any other screen. Existing saved contents, timing and screen order are retained; migration is idempotent. Calendar uses a second clock widget internally so earlier Workshop versions can still read saved settings.

## Smooth scrolling

Workshop 0.4.4 removes the 150 ms Screens loop delay and uses the existing USB link’s negotiated frame pacing (up to 30 FPS). OLED transfer/ACK time is included in the budget. Legacy or fallback displays that advertise 5 FPS retain their safe limit. Screen previews refresh at up to 30 FPS; USB polling no longer renders every playlist thumbnail on every pass.

Text motion uses elapsed milliseconds to advance individual pixels, rather than 50 ms buckets. The configured slow/normal/fast rates remain 24/40/72 pixels per second; borders, case, pause, reduced motion, and wraparound are preserved. The shared renderer includes this correction, but this release publishes only the private Workshop packages.

Live text uses the same adaptive USB pacing. Its heartbeat runs asynchronously while text moves, with at most one request in flight. Stop drops the USB link and drains a pending heartbeat before releasing the backend bridge, preventing a late heartbeat from reacquiring a released bridge. Claim/complete transitions remain ordered and backend-authorized. No intake or SMS settings are changed by this update.

## Wall-power blinking and reprogramming

Workshop 0.4.5 includes factory firmware 1.3.3 and 1.3.3-mistake. With no desktop controlling it, the factory smile closes its eyes for 200 ms every 4.2 seconds. Both a fresh fallback face and the saved factory face blink. Custom saved images retain their pixels. Firmware tests cover wall boot, repeated cycles, timer wrap, USB suspension/handoff, manual screen-off and unchanged flash contents. No physical-board acceptance is claimed by this release.

To install: update Workshop, open Prepare a droid, connect one board normally with a USB data cable and quit the customer DroidFun App. Keep the detected model and wiring (Mistake only for the known swapped-wire board). Choose Reprogram; it installs the matching firmware and replaces saved board screens with the startup smile. Follow unplug/reconnect verification, watch for at least five seconds and confirm the blink. Then move the USB cable to a wall adapter. Updating Workshop alone does not update a connected droid. A full wipe is not required for this change.

The firmware image preserves the filesystem partition, while the explicit Workshop reprogram action replaces its saved identity/startup playlist as described in the UI. Source is in firmware/factory; build metadata records the source hashes, flash ranges and standard/Mistake artifacts.

## Publishing a later private release

Bump the private config/header/workflow package version, build the Mac and Windows packages with the private CI workflow, collect its artifacts and remove the temporary Actions artifacts. The owner’s signing key is stored outside this repository under `~/.config/droidfun-workshop-signing/` with private filesystem permissions. Preserve it in a secure backup; never upload it to CI, the website or GitHub. `WORKSHOP_UPDATER_SIGNING_KEY_PATH` can override its local path.

On the owner’s Mac, place packages under `<collected>/mac/` and `<collected>/windows/`, then run:

```sh
node scripts/prepare-workshop-release.cjs <collected> <site-directory>
```

The command checks the Mac bundle identity/version, universal architecture and code signature/helper; creates the Mac updater archive; signs both update packages with the exact app version; independently verifies their signatures; copies the assets and prepares the owner-download manifest and authenticated updater feed. It never deploys automatically. Review `<collected>/prepared/release-evidence.json`, run the backend authorization tests and deploy the existing site Worker. Verify anonymous update/download requests fail and public catalog files are unchanged.

Updater cryptographic signatures are separate from Apple notarization or Windows publisher signing. Private packages retain their existing OS trust limitations.
