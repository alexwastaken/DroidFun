# Workshop 0.4.2

Workshop 0.4.1 and earlier cannot update themselves. Download the Mac or Windows Workshop package once from https://droidfun.io/admin#downloads, then replace the previous app/run the installer. Keep the same app identity; pairing, inventory and screen settings are retained.

In 0.4.2, open Updates → Check for updates. If a newer release is available, choose Download update, then Install and restart. Opening Updates releases the Screens USB connection and stops a Live text bridge. Board preparation must finish before installing. Normal Quit cannot interrupt bundle replacement. An available update is never installed without the explicit restart action.

Updates use the existing revocable Workshop pairing, held in native storage. Connect in Droid list first if needed. The manifest and packages are served only through authenticated `/api/workshop/updates/` routes. A separate Workshop Minisign public key is embedded in the private build; its signed version must match the manifest, downgrades are rejected, package size/time are bounded, redirects are disabled, and download URLs are restricted to the same two private DroidFun endpoints. The webview cannot supply a URL, key, version, signature or file path. Public companion configuration and its release feed are unchanged.

On Screens, turn individual cards on/off, set 3–120 seconds and use ↑/↓ to change their order. Play enabled screens starts/resumes the playlist; selecting a screen holds it until playback resumes. At least one screen stays enabled. The enabled flags, durations and order are saved in the existing settings file and restored on the next launch. Disabled screens remain editable and keep their contents. Saving scrolling text can extend its time enough for one pass, matching the companion’s screen editor.

## Publishing a later private release

Bump the private config/header/workflow package version, build the Mac and Windows packages with the private CI workflow, collect its artifacts and remove the temporary Actions artifacts. The owner’s signing key is stored outside this repository under `~/.config/droidfun-workshop-signing/` with private filesystem permissions. Preserve it in a secure backup; never upload it to CI, the website or GitHub. `WORKSHOP_UPDATER_SIGNING_KEY_PATH` can override its local path.

On the owner’s Mac, place packages under `<collected>/mac/` and `<collected>/windows/`, then run:

```sh
node scripts/prepare-workshop-release.cjs <collected> <site-directory>
```

The command checks the Mac bundle identity/version, universal architecture and code signature/helper; creates the Mac updater archive; signs both update packages with the exact app version; independently verifies their signatures; copies the assets and prepares the owner-download manifest and authenticated updater feed. It never deploys automatically. Review `<collected>/prepared/release-evidence.json`, run the backend authorization tests and deploy the existing site Worker. Verify anonymous update/download requests fail and public catalog files are unchanged.

Updater cryptographic signatures are separate from Apple notarization or Windows publisher signing. Private packages retain their existing OS trust limitations.
