# DroidFun App branding and upgrades

DroidFun App keeps the supplied Droid Fun wordmark and blue app icon. From 0.4.1, the selected bot automatically themes its controls, settings, screen editor and preview housing: **Boo** uses pearl/violet with a ghost silhouette, **Claw** uses orange/copper with a stacked, square body, and **Meep** uses mint/teal with a rounded antenna robot. The bot chooser shows each identity on its own card.

Change bot applies the matching theme immediately and remembers the bot through the existing saved botModel preference. Light/dark/system appearance remains independent. The old blue/green Branding selectors have been removed; legacy branding values remain readable and are preserved in settings for compatibility, but no longer override a bot's theme. The Droid Fun logo stays blue. OLED frame pixels, firmware, widgets and account settings are unchanged by theming.

The user approved the October 14 selector and Claude recovery changes for publication. Version 0.3.3 packages are published from the approved source; [upload verification](verification/native-0.3.3/release-upload-verification.json) records the exact application commit and asset hashes.

## Compatibility

Keep the bundle identifier `com.deskbot.companion`, Cargo executable `deskbot`, event/command IDs, settings paths, status-line bridge names and firmware wire protocol stable. These are compatibility identifiers, not customer-facing branding. Changing them without migration can disconnect existing users from preferences or installed integrations. The exported firmware remains 1.2.0, unchanged by 0.4.1; older compatible firmware can still drive live screens but Save to bot needs 1.2.0. Historical release URLs, evidence and records intentionally keep their original names.

## Upgrading an installed app

Before replacing the old Deskbot app, turn off Start at login in its settings and quit it. Install DroidFun App, confirm your existing playlist, then re-enable Start at login if wanted. The renamed app lives at a different application path; this prevents an old login entry from opening the previous app. Version 0.3.3 is available as a development release after native Windows/Linux installation checks and Mac package verification. Its exact universal Mac package was installed and launched on Apple Silicon. Saved settings and branding were retained, and a fresh live Claude card was observed. The robot was absent before this update, so its current physical connection is unverified. Full installer lifecycle, other OS hardware acceptance, signing/notarization and Intel runtime remain open.

## Packaging and publishing

Run `npm ci`, `npm run check`, `npm run build`, and the existing native Rust tests. Build on each supported OS using the documented packaging scripts. Future package artifacts are named `DroidFun-App-VERSION-PLATFORM`. Native window smoke checks now expect DroidFun App. Check light/dark appearance, the robot-face app/tray icons, settings, and an upgrade with an existing playlist. The blue face is the application icon; macOS gets a monochrome template of the same face for the menu bar.

Version 0.3.3 packages are manually published on GitHub with native build evidence. The website download configuration was not changed by this rebuild and remains a separate update. A source-code push alone does not replace website downloads. Do not relabel old installers as a newer version. Preserve release signing/notarization disclosures and the current firmware/hardware verification limits.

App 0.5.4 restores Boo and adds Nimbus with a cloud silhouette and blue light/dark palettes. Nimbus supports the same everyday screens and saved text/face playlists as Meep. Existing per-board settings stay intact.
