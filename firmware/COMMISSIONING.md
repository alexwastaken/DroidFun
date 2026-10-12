# Robot commissioning record

Date: October 11, 2026. The original panel failed image-fidelity checks. **The replacement displays complete test text and cycles readable app-driven screens, confirmed by the user.** Remaining acceptance is listed below.

## Current configuration: firmware 1.1.0 / app 0.2.0

The replacement board 505431651938581C uses the confirmed SDA GP4 / SCL GP5 wiring and 0x3C panel. Firmware 1.1.0 was compiled and uploaded with verification. HELLO reports the expected dimensions, wiring profile, OLED ready, and maximum five frames per second. The app automatically reconnects to the selected board after relaunch.

The user confirmed all five **live** screens are complete and readable after this firmware update. They subsequently confirmed the small upper-right face blinks while data remains readable. That initial corner-face implementation was then enlarged using the original happy-face bitmap; its final verification is in ../SPEC_COMPLETION.md.

Native settings checks cover orientation selection, quiet-hours blanking and 15-minute override, dark appearance, startup registration/removal, diagnostics review/save, surprised-face preview, weather high/low, and dated stock-change output. These UI observations support implementation; they do not establish physical orientation/pixel fidelity or timed fallback/suspend acceptance. Brightness 88, all five 3-second durations, original weather/stock layout choices, and normal orientation were retained.

Original and initial replacement records below are chronological evidence from the earlier firmware versions. Their older sample-screen limitations do not supersede the newer user confirmations above. Remaining hardware gates include unplug/replug, OLED-disconnect health, malformed-packet recovery on the board, timed fallback/dim/off, suspend/resume, both orientations with a pattern photo, voltage checks, and the 24-hour soak.

## Assembly and authorization

- User identifies the controller as RP2040-Zero and the display listing as an AITRIP 0.96-inch 128×64 SSD1306.
- User reports SDA connected to GP4 and SCK/SCL connected to GP5.
- USB identifies Waveshare RP2040 Zero, VID 2E8A / PID 0003, serial `5054316558976B9C`. Current macOS path: `/dev/cu.usbmodem1101`; paths may change.
- Original sketch reported `ALIEN_SMILES v1` and OLED address `0x3C`. Source unavailable.
- User authorized erasing the original sketch without a backup. Arduino uploads succeeded using the USB 1200-baud reset route.

## Verified transport; failed physical output

Deskbot firmware replied with board ID `5054316558976B9C`, firmware `1.0.0`, dimensions 128×64, and successful I²C writes on GP4/GP5 to `0x3C`.

| Host input | Host CRC32 | Firmware ACK CRC32 | Nonzero bytes | Observed screen |
| --- | --- | --- | --- | --- |
| HELLO / DESKBOT TEST | 1529312879 | 1529312879 | 388 | Blank |
| Horizontal stripe test (`AA` × 1024 bytes) | 1013920381 | 1013920381 | 1024 | Blank |

The ACK confirms receipt and I²C transaction completion; it cannot verify emitted light or panel pixel positions. User saw a brief face at one point, but its origin was not established. Batched SSD1306 initialization commands did not resolve the blank output. Do not describe these tests as hardware success.

## Saved independent diagnostic for original panel

`oled_diagnostic/oled_diagnostic.ino` uses Arduino Wire and Adafruit SSD1306 2.5.17, GFX 1.12.6, and BusIO 1.17.4, rather than the Deskbot protocol or direct Pico I²C calls. Bus clock is 100 kHz; startup waits 250 ms. The user supplied a photograph showing DESKBOT TEST and a partially missing HELLO and border. The photo is sideways; the text itself has the correct orientation. This establishes visible output, but image fidelity still fails.

The second diagnostic explicitly selected each page and wrote 16-byte chunks. The user reports that parts remain missing, so horizontal auto-increment alone does not explain the fault. The next diagnostic bypasses display memory with Entire Display ON (`A5`), at reduced contrast, to distinguish transfer faults from row output. It also accepts `t` for text, `a` for all pixels, `c` for sequential COM mapping (`DA 02`), and `n` for the default alternative COM mapping (`DA 12`). These are diagnostic comparisons, not confirmed panel settings.

The user identified the original screen as broken and connected another device. Its diagnostic source, flashable UF2, photograph, pinned tool versions, and verified SHA-256 values are preserved in [the damaged-panel archive](archives/2026-10-11-damaged-oled/README.md). The diagnostic deliberately does not answer the Deskbot handshake. It is no longer installed on the connected replacement.

```sh
arduino-cli lib install "Adafruit SSD1306@2.5.17" "Adafruit GFX Library@1.12.6" "Adafruit BusIO@1.17.4"
arduino-cli compile --fqbn rp2040:rp2040:waveshare_rp2040_zero --output-dir "$HOME/.cache/deskbot/oled-diagnostic-build" firmware/oled_diagnostic
arduino-cli upload -p /dev/cu.usbmodem1101 --fqbn rp2040:rp2040:waveshare_rp2040_zero --input-dir "$HOME/.cache/deskbot/oled-diagnostic-build" --verify firmware/oled_diagnostic
```

Tool versions: Arduino CLI 1.5.1 and Arduino-Pico core 6.3.0.

## Replacement robot

- USB serial and firmware board ID: `505431651938581C`; Waveshare RP2040 Zero, VID 2E8A / PID 0003.
- User confirms the same GP4 SDA / GP5 SCL wiring. The original sketch was ALIEN_SMILES v1 and reported `0x3C`.
- Installed firmware: **Deskbot 1.0.2**. Profile: SSD1306 128×64, `0x3C`, 100 kHz I²C, explicit page addressing, 250 ms startup delay, scroll disabled.
- Known host text buffer: CRC32 `1529312879`, 388 nonzero bytes. Replacement ACK returned the same values.
- User confirmed HELLO / DESKBOT TEST stays visible and complete after about 15 seconds.
- Desktop app selected this board through its Connect control. UI showed firmware 1.0.2 and connected status. The user then confirmed the face, weather, stocks, Codex, and Claude sample screens cycle and remain readable.
- After updating and relaunching the desktop app, it automatically reconnected to the persisted replacement. The UI showed one callout USB port, firmware 1.0.2, and active cycling. This verifies app-relaunch recovery; physical unplug/replug is still a separate acceptance check.
- Current frontend/native builds succeeded, and 10 Rust tests passed, including a regression fixture for macOS's duplicate tty/cu aliases. These support the observed hardware result and do not replace the outstanding physical checks.

### Firmware timing correction

The first replacement run flashed text and then blanked. Firmware took a loop timestamp before processing incoming commands, then subtracted a newer `last_host_command` timestamp from it. Unsigned underflow could trigger all idle stages immediately, including display-off. Version 1.0.2 reads the clock after processing commands and restores contrast/power when reconnecting after idle sleep. This is a software defect separate from the original panel's visibly incomplete letters. Matching frame checksums alone did not reveal it.

The current desktop build is `build/Deskbot.app`, with `build/Deskbot-macOS-arm64-usb.zip`. The selected replacement is persisted in local settings. Sample values are not live provider data.

## Remaining acceptance

Complete text and app-driven widget cycling have passed with user observation on the replacement. Brightness range, screen off/on, unplug/replug, malformed-packet recovery on the board, timed fallback/dim/sleep, orientation, and a soak run still need integrated acceptance. Supply/pull-up voltages remain unverified. Windows/Linux behavior remains open. Weather and Codex now have live desktop checks, recorded in [provider acceptance](../PROVIDER_IMPLEMENTATION.md); new live provider values have not received a separate physical OLED observation.

## Configurable provider build

The desktop app now retains independent source settings for each widget and feeds the same renderer with normalized live weather and Codex cards. On relaunch, the native UI showed replacement board `505431651938581C`, firmware 1.0.2, OLED ready, saved brightness 88%, and active cycling. The live weather and allowance cards passed desktop preview checks. That connection status acknowledges frame delivery; physical readability of these specific new values is not independently established. Sample-screen readability remains user-confirmed above.

## Free stocks and shared Claude account build

October 11, 2026: stock quotes now require no API key or signup. The running macOS preview matched the public MSFT quote (535.07 USD, October 9). Claude browser sign-in completed and the shared account preview matched a separate read-only allowance response (94% remaining in the five-hour window; 39% weekly remaining). The saved playlist retains all five enabled widgets and brightness 88. The updated app automatically reconnects to the same replacement. Provider checks and 17 passing behavior tests are recorded in [provider acceptance](../PROVIDER_IMPLEMENTATION.md). These checks do not replace separate physical review of the new live values or the remaining hardware gates.
