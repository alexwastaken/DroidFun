# 0.10.2 — Mac menu-bar companion

This is a local arm64 candidate. Public downloads and the updater remain on 0.10.0 while Developer ID signing, Apple notarization and public package acceptance are pending. See [Mac signing](../../MAC_SIGNING.md). This change does not remove Gatekeeper warnings from unsigned public downloads.

DroidFun starts as a menu-bar accessory by default, including when its controls are open. Closing the controls leaves the engine and USB connection running. Clicking the menu-bar icon offers Open DroidFun App, Settings and Quit. Finder can still reopen the same process. Customers who prefer a Dock icon can select **Settings → App → Show DroidFun App in → Dock and menu bar**.

Start at login stays opt-in. Menu-bar mode launches with no control window. A background launch of an already-running process also stays quiet. Applying an enabled login setting refreshes the executable path in its registration. Existing settings, droid profiles and firmware are preserved; the private Workshop app remains a normal window app.

## Validation

- 155 native tests passed; zero failures, one existing live-account test ignored.
- Production TypeScript/Vite build and local arm64 app/DMG packaging passed.
- [12 settings UI scenarios](settings-ui.json) passed, covering macOS modes, legacy background preferences, explicit login choice and unchanged Windows/Linux choices.
- Independent native lifecycle review found no blockers.

[Installed Mac acceptance](mac-acceptance.json): the owner confirmed the menu icon and commands work. Switching to Dock mode added the app to the native regular-app inventory; restoring menu-bar mode removed it. Closing and reopening controls kept the same process and showed the physical Nimbus still connected. Start at login was enabled on the owner’s Mac, and its registration targets the installed executable with `--background`. Cold and duplicate background process launch checks passed. Other saved settings and firmware were preserved. The final app passed strict ad-hoc signature/helper checks and matched its DMG.

A full Mac sign-out/sign-in cycle and public notarized download acceptance remain untested. Menu interaction was confirmed by the owner because desktop automation could not inspect the status item directly.
