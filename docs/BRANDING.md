# DroidFun App branding and upgrades

DroidFun App is the desktop controller for Droid Fun bots. Version 0.3.1 replaces the Deskbot name in the app, settings, tray menu, diagnostics export, window title and future installer names. The logo comes directly from droidfun.io; use the assets in public/brand rather than inventing another logo. The primary blue is #3268dc, text #17283d, background #f8fafc, and surface #edf2f8. Dark mode uses the same blue with navy surfaces.

## Compatibility

Keep the bundle identifier `com.deskbot.companion`, Cargo executable `deskbot`, event/command IDs, settings paths, status-line bridge names and firmware wire protocol stable. These are compatibility identifiers, not customer-facing branding. Changing them without migration can disconnect existing users from preferences or installed integrations. Existing firmware 1.1.0 remains compatible and its verified binary is unchanged; the exported download has a DroidFun filename. Historical release URLs, evidence and records intentionally keep their original names.

## Upgrading an installed app

Before replacing the old Deskbot app, turn off Start at login in its settings and quit it. Install DroidFun App, confirm your existing playlist, then re-enable Start at login if wanted. The renamed app lives at a different application path; this prevents an old login entry from opening the previous app. Native upgrade/install verification is still required before distributing 0.3.1.

## Packaging and publishing

Run `npm ci`, `npm run check`, `npm run build`, and the existing native Rust tests. Build on each supported OS using the documented packaging scripts. Future package artifacts are named `DroidFun-App-VERSION-PLATFORM`. Native window smoke checks now expect DroidFun App. Check light/dark appearance, the robot-face app/tray icons, settings, and an upgrade with an existing playlist. The blue face is the application icon; macOS gets a monochrome template of the same face for the menu bar.

The website still serves the verified 0.3.0 release until 0.3.1 native packages are built, tested, and manually published. A source-code push alone does not replace website downloads. Do not relabel the old installers as 0.3.1. Preserve release signing/notarization disclosures and the current firmware/hardware verification limits.
