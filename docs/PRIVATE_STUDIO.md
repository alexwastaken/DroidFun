# Private Workshop with screen controls

DroidFun Workshop 0.4.0 combines the previous photography app and the owner’s board preparation tool in one Mac/Windows app. Owner dashboard → Downloads → Workshop is its distribution channel. Do not add it to the public customer catalog or updater.

Screens offers all twelve screen types, local generated readings, Codex/Claude allowance sliders, text/image editing, brightness, rotation, cycle/hold controls and a clean view for pictures. No provider login is needed. The screen UI and OLED output omit demo labels. Choose a connected USB droid to send frames to its display. This changes the live display only, not its stored startup screens.

Prepare a droid retains installed-firmware inspection, model identity programming, standard/Mistake wiring, saved startup face and power/reconnect/shipping checks. Whole-flash wipe is available only through the supported Mac USB helper and remains explicitly confirmed; Windows can reinstall firmware through the board’s ROM boot drive. Droid list retains the local records and dashboard pairing/sync workflow. Windows can also open the owner dashboard from the pairing panel.

Switching to either Workshop section releases the screen serial handle and its USB operation guard before mounting the board tools. Screen mode cannot start while a board operation or shipping check sequence is pending. Native guards also reject board operations in screen mode. Closing or quitting during a board write is prevented. Other DroidFun apps using USB must be quit separately.

The private build uses `com.droidfun.workshop.private`, preserving existing Workshop registry and pairing files. Its local screen preferences use `studio-settings.json`; on first launch they can be read from the previous Studio configuration. No existing user files are overwritten by installation. Credentials live in each owner installation and are never embedded into packages or sent into the webview. A new installation requires its own one-use dashboard connection code to sync private inventory. Local board preparation and screen previews work without pairing.

The private IPC capability allows only its local bundled main window. Native owner/Workshop modules are compiled only with the private feature; the public app retains its original command list and capability. All navigation and new-window blocking from the screen app remains in effect. Generated remote readings do not call provider APIs.

Build: `npm run tauri -- build --features showcase --config src-tauri/tauri.showcase.conf.json --target TARGET --bundles app` on Mac, or `--bundles nsis` on Windows. The wrapper bundles the Mac USB helper and library. The private workflow produces one universal Mac ZIP and a Windows x64 installer. Private packages are unsigned/ad-hoc and can trigger OS trust prompts.

Owner downloads use authenticated `/api/admin/studio-downloads` routes (kept stable for existing links). Packages reside behind the Worker's protected `/download-assets/` prefix, are excluded from the public catalog and use private/no-store headers. Remove temporary CI artifacts after copying packages to owner storage; this source repository is public.
