# DroidFun App branding and upgrades

DroidFun App is the desktop controller for Droid Fun bots. Version 0.3.1 replaces the Deskbot name in the app, settings, tray menu, diagnostics export, window title and future installer names. The logo comes directly from droidfun.io; use the assets in public/brand rather than inventing another logo. The Branding dropdown offers **DroidFun · blue** (the website palette and default for existing/new settings without a saved selection) and **Classic · green** (the original green robot and controls). Both preserve the DroidFun name, supplied logo and native app/tray icons. The selection is saved with preferences and works independently of system/light/dark appearance. The main controls apply it immediately; Settings → App applies it with the other draft settings. Changing it does not affect widgets, account connections or OLED frames.

The user approved the October 14 selector and Claude recovery changes for publication. Version 0.3.3 packages are published from the approved source; [upload verification](verification/native-0.3.3/release-upload-verification.json) records the exact application commit and asset hashes.

## Compatibility

Keep the bundle identifier `com.deskbot.companion`, Cargo executable `deskbot`, event/command IDs, settings paths, status-line bridge names and firmware wire protocol stable. These are compatibility identifiers, not customer-facing branding. Changing them without migration can disconnect existing users from preferences or installed integrations. Existing firmware 1.1.0 remains compatible and its verified binary is unchanged; the exported download has a DroidFun filename. Historical release URLs, evidence and records intentionally keep their original names.

## Upgrading an installed app

Before replacing the old Deskbot app, turn off Start at login in its settings and quit it. Install DroidFun App, confirm your existing playlist, then re-enable Start at login if wanted. The renamed app lives at a different application path; this prevents an old login entry from opening the previous app. Version 0.3.3 is available as a development release after native Windows/Linux installation checks and Mac package verification. Its exact universal Mac package was installed and launched on Apple Silicon. Saved settings and branding were retained, and a fresh live Claude card was observed. The robot was absent before this update, so its current physical connection is unverified. Full installer lifecycle, other OS hardware acceptance, signing/notarization and Intel runtime remain open.

## Packaging and publishing

Run `npm ci`, `npm run check`, `npm run build`, and the existing native Rust tests. Build on each supported OS using the documented packaging scripts. Future package artifacts are named `DroidFun-App-VERSION-PLATFORM`. Native window smoke checks now expect DroidFun App. Check light/dark appearance, the robot-face app/tray icons, settings, and an upgrade with an existing playlist. The blue face is the application icon; macOS gets a monochrome template of the same face for the menu bar.

Version 0.3.3 packages are manually published on GitHub with native build evidence. The website download configuration was not changed by this rebuild and remains a separate update. A source-code push alone does not replace website downloads. Do not relabel old installers as a newer version. Preserve release signing/notarization disclosures and the current firmware/hardware verification limits.
