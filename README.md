# DroidFun App

Private development home: **[alexwastaken/DroidFun](https://github.com/alexwastaken/DroidFun)**. Future changes and releases belong here. Customers download installers from **[droidfun.io/downloads](https://droidfun.io/downloads)**; they do not need GitHub access. Website updates remain manual, at Alex’s request. Current per-platform downloads and checksums are recorded in `distribution/current.json`.

DroidFun App is a robot-themed tray companion for a USB RP2040-Zero and SSD1306 128×64 OLED. Clock, Pomodoro, meeting countdown, CPU, memory, weather, free stock quotes, Codex allowance, Claude account allowance and a friendly face are configurable in the app. The controls and cycle preview also work without hardware.

## Screens and bot profiles: 0.4.0

Choose Boo, Claw or Meep on first launch. Boo and Meep support custom text, faces and everyday widgets; Claw offers Claude/Codex usage screens only. The editor previews exact 128 × 64 output with text layout, borders, inversion and per-screen appearance controls.

**Save to bot** stores enabled text and still face screens for USB wall power, using firmware 1.2.0. It verifies a complete saved playlist before confirming success. Live data and desktop-driven clocks/timers are not stored. **Pause online data** is a separate desktop option that cancels internet requests while local screens keep working. See the [screen setup guide](docs/SCREENS.md) and [firmware builds](firmware/releases/README.md). Physical wall-power verification remains outstanding.

## App branding: 0.3.3

Choose **DroidFun · blue** (the palette from [droidfun.io](https://droidfun.io)) or **Classic · green** in the Branding dropdown. Both keep the DroidFun name and logo, and the choice is saved independently of light/dark appearance. See [branding and upgrade notes](docs/BRANDING.md).

## Downloads: 0.4.0

Customers can get the app and **Controller setup PDF** from [droidfun.io/downloads](https://droidfun.io/downloads). Private release assets and platform verification records are kept in [v0.4.0](https://github.com/alexwastaken/DroidFun/releases/tag/v0.4.0).

- **Mac:** universal DMG and ZIP for Apple Silicon and Intel. Architecture, archive checksums and ad-hoc signature are verified. The bot chooser, custom-text editor and profile switching were checked in an isolated Apple Silicon preview. Intel runtime and upgrading the user's installed app were not tested for this release.
- **Windows:** x64 and ARM64 installers. Native behavior tests, real installation, payload comparison and window startup passed on each architecture.
- **Linux:** x86_64 DEB and RPM. The DEB configures robot permissions; **Connection → Set up robot access** repairs access through the desktop password dialog. See the release's native verification record for installation and repair checks. RPM payloads are checked; Fedora runtime is untested.
- **Firmware:** included 1.2.0 for RP2040-Zero, SSD1306 128×64, SDA GPIO 4 / SCL GPIO 5, address 0x3C. Export it from **Settings → Robot**. Firmware storage and lifecycle tests passed, but physical Save to bot, unplug and wall-power reboot acceptance remains outstanding. No connected bot was flashed during this release.

Mac packages are not Developer ID signed or notarized; Windows installers are unsigned. [Native build instructions](docs/NATIVE_BUILDS.md), [screen setup](docs/SCREENS.md) and [feature acceptance record](SPEC_COMPLETION.md) record the remaining limits. Earlier hardware and provider test results are historical and do not replace physical checks of this firmware.

![The original happy bot stays beside live data](docs/images/companion-original-face-preview.png)

## Linux robot setup

From 0.3.4, install the DEB using Ubuntu’s graphical package installer and approve its normal password prompt, then plug in your robot. Serial access is configured automatically. **Connection → Set up robot access** repairs existing installations through the desktop’s password dialog, without Terminal or a reboot. [Setup details and supported desktops](docs/LINUX_USB_ACCESS.md).

## Controls

- Add independent weather cities, stock symbols and usage windows. Enable/disable, reorder by dragging or arrow buttons, and choose each duration.
- Add clocks with local/named timezones, 12/24-hour time, seconds and date; Pomodoro timers with editable focus/break lengths and Start/Pause/Reset/Skip; meeting titles/date/time/duration; CPU sampling/history; and memory used/available, units and sampling. [Local widget guide](LOCAL_WIDGETS.md).
- Select a row for local preview. **Show now** displays it for one dwell and restores the cycle; **Pin** holds it; Pause, Next and Resume follow the saved playback state. Closing controls leaves the robot cycling.
- Data widgets keep the original happy bot in their upper-right corner, enlarged from the same face bitmap, with quicker blinks, glances toward the information, Ahh/Ooo mouths and a gentle one-pixel movement. Its reserved corner does not overlap the figures. Display settings control animation energy, glances and mouth shapes, or disable the companion/animation. Reduced motion and Pause freeze animation.
- The full face has happy, neutral, sleepy and surprised expressions, blinking and speed controls.
- Weather has Fahrenheit/Celsius and current conditions or daily high/low layouts. Stocks optionally show a verified price change while retaining currency and quote date. Public Yahoo quotes need no account or API key and may be delayed.
- Codex and Claude display **remaining account allowance percentages**, not token counts. Claude includes desktop/web/Code usage through your existing Claude Code sign-in. An optional status-line source supports selected reports; existing commands are preserved.
- Claude window-reset handling and bounded sign-in renewal are documented in [Claude recovery](docs/CLAUDE_RECOVERY.md). Mac natural renewal was verified in the recorded preceding build. Windows automatic recovery now passes native ConPTY/credential/HTTP/cycle fixtures; see [Windows recovery and its live-account boundary](docs/WINDOWS_CLAUDE_RECOVERY.md).
- Settings include orientation, overnight quiet hours, a 15-minute wake override, system/light/dark appearance, opt-in start at login, and tray/window behavior.
- Review diagnostics before saving a local report. Export the included firmware with manual BOOT instructions. Reset only DroidFun App preferences with a backup; account sign-ins remain intact.
- Invalid or expired data is skipped. Requests are bounded, cancel when configuration changes, share cached results, honor retry limits, and persist only usable public weather/stock cards. Credentials, prompts and transcripts are not persisted by DroidFun App.

On macOS the tray is the menu bar. On Windows it is the notification area beside the clock. Linux uses a compatible status notifier/system tray; without one, controls remain available as a normal minimized window and through the application launcher.

## Development

Install Node.js 22+, Rust and [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). Ordinary users run the packaged app and need no development tools. Codex/Claude Code are optional dependencies for their account integrations.

```sh
npm ci
npm run tauri -- dev
npm run check
CARGO_TARGET_DIR="$HOME/.cache/deskbot/target" cargo test --manifest-path src-tauri/Cargo.toml
npm run release -- --target universal-apple-darwin
```

Native packaging runs only on the current OS: macOS app/DMG, Windows NSIS x64 or ARM64, Linux x86_64 DEB/RPM. `--signed` requires platform signing configuration. It does not dispatch hosted builds or publish artifacts. Dependency versions and lockfiles are pinned. [Manual GitHub builds](docs/NATIVE_BUILDS.md) use native Windows/Linux runners and upload checked Actions artifacts for review; they do not publish automatically. The launcher uses an internal-disk Cargo cache on this Mac and removes only AppleDouble metadata sidecars before parsing project configuration.

Windows/Linux physical robot and live-account behavior, GNOME/KDE tray interaction, Fedora RPM runtime, Intel Mac runtime, full installer lifecycle, accessibility at enlarged text sizes, signing/notarization, timed hardware recovery and the 24-hour soak remain release gates. Disable Start at login before manually removing the macOS/Linux app, and Quit DroidFun App first. No automatic updater is enabled.
