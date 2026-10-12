# Deskbot 0.2.0 implementation and acceptance

Updated October 11–12, 2026. Mac, Windows x64/ARM64 and Linux x86_64 development packages are built. Mac hardware behavior is commissioned; native Windows/Linux package checks do not establish full hardware or desktop acceptance. The original damaged OLED diagnostic and prior app/firmware builds remain archived.

## Current deliverables

- `build/Deskbot.app` — universal Apple Silicon/Intel executable; Apple Silicon runtime verified on this Mac.
- `build/Deskbot-0.2.0-macOS-universal.zip` and `.dmg` — local development packages, without Developer ID signing/notarization.
- `firmware/releases/deskbot-rp2040-zero-gp4-gp5-v1.1.0.uf2` — installed and verified on the replacement RP2040-Zero; exportable from Settings → Robot.
- [docs/verification/0.2.0.json](docs/verification/0.2.0.json) — hashes, checks and remaining gates. Mac, Windows and Linux installer files are attached to the [GitHub development release](https://github.com/greensky5656/DroidFun/releases/tag/v0.2.0).
- Windows x64/ARM64 NSIS and Linux x86_64 DEB/RPM packages, platform verification JSON and checksums are attached to that release. [Native build instructions](docs/NATIVE_BUILDS.md), [Windows x64 evidence](docs/verification/native-0.2.0/verification-0.2.0-Windows-x64.json), [Windows ARM64 evidence](docs/verification/native-0.2.0/verification-0.2.0-Windows-ARM64.json), [Linux evidence](docs/verification/native-0.2.0/verification-0.2.0-Linux-x86_64.json).

## Implemented features and evidence

| Feature | Evidence and practical boundary |
| --- | --- |
| Tray app and recovery window | Resident native app; saved playlist survives upgrades/relaunch. macOS controls and hardware reconnect observed. Linux detects a status notifier host and retains a normal window when absent; native Windows window startup and Linux window fallback without a tray host passed automated checks. Human tray interaction and full desktop recovery remain unverified. |
| Widget cycle and multiple instances | Enable, duration, keyboard/drag reorder, Preview, Show now, Pin, Pause, Next and Resume. Fake-clock tests cover dwell preservation, changes, temporary playback and wake behavior. User confirms all five live screens cycle and remain readable. |
| Original robot identity on every data widget | Original happy-face bitmap is reused at half size, with margins cropped into a reserved 40×26 upper-right corner. Rounded hollow eyes, original smile, periodic blink and one-pixel movement. Main figures begin at row 28; usage bars and footers are separate. Tests prove no data pixel changes during animation, reduced motion freezes it, and off produces a blank frame. Native stock preview visually verifies the enlarged face and complete price/date. The user confirmed blinking/readability for the earlier smaller corner face; physical approval of the enlarged revision is recorded only if separately supplied. |
| Face controls | Happy, neutral, sleepy and surprised; blinking and slow/normal/quick speed. Distinct frame tests and native surprised preview. |
| Weather and stocks | Configurable cities/units, daily high/low layout; free keyless public stock symbol and optional verified change. Native previews showed 64 F / H71 L64 F and MSFT 535.07 / +2.4% / Oct 9. Original layout choices restored. Quotes are dated and may be delayed. |
| Codex and shared Claude allowance | Separate returned account windows; default installed sign-ins; Claude account includes desktop/web/Code usage. Default live account preview verified; missing/expired values never become zero. Advanced Claude profile folder and report directory/profile/session selectors are implemented; real multi-account switching remains a native acceptance gate. |
| Optional Claude report integration | Allowlisted local reports, original status-line output/settings preserved, restore control. Fixture coverage and earlier local helper checks; native Windows/Linux integration remains unverified. |
| Provider reliability | Independent workers, shared in-flight requests/cache, cancellation on configuration change, bounded response sizes, deadlines, jittered backoff, shared HTTP 429 cooldown, and minimum public startup cache. Local HTTP tests verify one fetch for concurrent identical requests, quota blocking across different requests, and cancellation before timeout. Public-cache tests verify restart, configuration isolation and expiry. |
| Orientation and quiet hours | 0/180° command, local overnight schedule, full-dwell wake, 15-minute override and manual-off precedence. Deterministic tests and native quiet/off/override controls. DST/local-time policy tested; real OS suspend and rotated physical patterns remain gates. |
| App preferences | System/light/dark, opt-in startup, automatic/minimized-window/background behavior, first-launch choice. Native dark mode and macOS startup enable/remove verified; final startup disabled and system theme restored. |
| Settings recovery | Atomic writes, validated last-known backup, unreadable-file preservation, future-version rejection and rollback on save failure. Recovery tests also verify a corrupt primary cannot overwrite the usable backup. |
| Diagnostics and reset | Exact reviewed allowlisted report saved via native dialog. Export verified to omit credentials, account identity, city, symbol, full device serial and local paths. Reset creates a backup and restores only Deskbot preferences/startup; global sign-ins and optional bridge setup preserved. Reset was not applied to the user's live playlist. |
| Firmware and USB transport | Firmware 1.1.0 advertises 5 fps, shares fallback face assets and handles USB suspend. Selected board only, bounded packets, ACK/retry, newest unsent frame and explicit OLED error state. Upload verified; native handshake and automatic app-relaunch recovery observed. Board-level corruption recovery and physical suspend/fallback timing remain gates. |
| Keyboard and preview | Labeled canvas/controls, dialog focus trap, Escape/cancel and focus return, keyboard reorder alternatives, integer pixel scale and work-area clamping. Native reverse Tab wraps to Apply in Settings. Full enlarged-text, screen-reader and contrast matrix remains an acceptance gate. |
| Packaging | Pinned dependencies/lockfiles and native release script for Mac universal app/DMG, Windows NSIS x64/ARM64, Linux x86_64 DEB/RPM. Current universal build succeeds and both executable architectures are present. Windows x64/ARM64 installers and Linux DEB/RPM are built on native runners. Each target passed 27 behavior tests; installers and native windows were checked on Windows and Ubuntu 24.04. RPM metadata and ELF architecture are checked; Fedora runtime remains open. Windows signing remains open. |

## Checks performed

- **27 Rust behavior tests passed**, no failures; TypeScript check and production frontend build passed.
- Windows x64/ARM64 and Linux x86_64: 27 Rust behavior tests per target, frontend check/build, correct executable architecture, installation and 15-second native window startup checks passed. Package SHA-256 hashes matched both downloaded Actions artifacts and GitHub release assets. RPM was inspected; runtime check used the DEB. Windows ARM64 uses a newer source commit with only a mock HTTP test-fixture fix; production app, frontend and firmware source match the original release.
- Universal app and DMG packaging succeeded; final native app reconnects to board `505431651938581C`, firmware `1.1.0`, OLED ready.
- User confirmed all five live screens remain complete/readable; first corner-face blinking is physically confirmed.
- Enlarged original-face stock preview is saved in `build/companion-original-face-preview.png`. Additional dated UI evidence: `quiet-hours.png`, `settings-dark.png`, `face-surprised.png`, `weather-high-low.png`, `stock-change-preview.png`, `Deskbot-diagnostics.json`.
- Final preferences: all five enabled at 3 seconds each, brightness 88, happy expression, normal orientation, current-weather layout, price change disabled, system appearance, quiet hours and start-at-login disabled. Companion and animation enabled.

## Remaining release and hardware gates

These are not represented as passed by builds, protocol ACKs or fixture tests:

1. Windows 11 x64/ARM64 and Linux Ubuntu/Fedora GNOME/KDE physical USB access, human tray recovery, sign-in/profile behavior and full installation/upgrade/uninstall acceptance. Windows x64 startup ran on the Windows 2025 runner; ARM64 on Windows 11. Linux DEB startup ran under Xvfb/D-Bus without a tray host; Fedora RPM runtime and actual GNOME/KDE sessions remain untested. Intel Mac runtime is also untested.
2. Developer ID signing/notarization and Windows signing require the release owner's certificates. No automatic updater is enabled; a future updater requires signed artifacts.
3. Full A01–A19 physical/platform acceptance: timed recording with controls closed, unplug/replug with changed path, host-loss fallback/dim/off, malformed packets on the real board, OLED disconnection, suspend/wake, checkerboard/corner patterns in both orientations, and supply/pull-up voltage checks.
4. Full keyboard/screen-reader/enlarged-text/contrast matrix and a measured 24-hour network/USB soak, including closed-panel CPU/memory targets. No five-minute or 24-hour performance claim is made.
5. Automatic per-user startup cleanup during macOS/Linux manual removal is not available through drag-to-Trash/package removal. Disable Start at login before uninstall and Quit the app. Windows Tauri NSIS includes Run-entry cleanup; its native lifecycle still needs acceptance. This is the remaining A16 lifecycle limitation.
6. Real multi-account/profile switching, exchange calendar handling, public-provider terms/retention/licensing and commercial weather service entitlement review. Yahoo and Claude account readers use compatibility endpoints that may change.

Refer to the specification's A01–A19 contracts for the expected input/action/output evidence. This build is ready for personal testing on the commissioned Mac/robot pair; it is not a cross-platform signed release.

Packaging reference: [Tauri NSIS installer source for the pinned CLI](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.2/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi) includes startup Run-entry cleanup during uninstall while preserving it during update. Native Windows acceptance remains open.
