# DroidFun firmware builds

Use **droidfun-rp2040-zero-gp4-gp5-v1.2.3.uf2** for the RP2040-Zero / SSD1306 profile: SDA GP4, SCL GP5, 128×64. Version 1.2.3 keeps a powered bot visible during USB suspension and long host inactivity. A fresh board shows its fallback smile; saved text/face playlists still play without a computer. The automatic 60-second dim and 10-minute off timers are removed. A heartbeat requests the desktop frame again after fallback, including a cached static widget. USB suspension retains the session, clears partial input and cancels incomplete RAM uploads; it does not erase saved screens. Explicit desktop screen-off and quiet-hours policy still take priority. This shared profile covers Meep, Boo and Claw with the same RP2040-Zero / SSD1306 wiring. Host tests include wall-power boot, suspend/resume, 24-hour gaps, timer rollover and manual-off; physical sleep/wake acceptance is recorded separately.

Version 1.2.2 fixes a one-byte buffer overflow when decoding an oversized, malformed COBS packet. It checks capacity for both copied bytes and implicit zero separators, preserving the board identity and allowing the next valid packet to be handled. Host regression tests cover the original failure, maximum-size frames, and the fresh-board fallback face. This change does not establish or claim a fix for intermittent USB re-enumeration observed on a physical board.

Version 1.2.1 added checks for both standard OLED addresses, 0x3C and 0x3D, without changing the GPIO wiring. Those checks remain in 1.2.2. An unresponsive display remains a display error even when USB works.

DroidFun App detects mounted RPI-RP2 setup drives under Connection and installs the embedded firmware. A running DroidFun board can enter setup mode using **Update bot software**, which uses the Arduino-Pico 1200-baud USB restart. Neither path requires opening a working assembled bot. Corrupt firmware that exposes neither serial nor the ROM drive still needs hardware recovery; software cannot guarantee recovery from every hardware fault.

`droidfun-display-test-v1.uf2` is the temporary standalone screen diagnostic, not the normal robot firmware. It runs low-contrast all-pixel and checkerboard tests and reports its fixed GP4/GP5 I2C results over USB. Its source and report format are in `../display_test/README.md`. Restore the current normal firmware after testing.

Version 1.2.0 adds persistent saved text/face playlists for wall USB power. The app exports this embedded UF2 through Settings → Robot. It reserves 256 KiB of flash for two checksummed playlist slots; an incomplete upload leaves the current slot intact. Only an explicit save can write flash. The firmware never automatically formats a nonblank filesystem. This build has compiled successfully and is covered by host-side protocol/storage tests; physical unplug/reboot/wall-charger acceptance still needs checking on a bot.

Rebuild with Arduino CLI 1.5.1 and Arduino-Pico 6.3.0:

```sh
arduino-cli compile --fqbn rp2040:rp2040:waveshare_rp2040_zero:flash=2097152_262144 firmware/deskbot_firmware
```

Keep the same flash partition setting on later updates to preserve saved playlists. Never run a full flash erase as part of an ordinary app update.

Version 1.1.0 was installed on replacement board 505431651938581C. Arduino CLI verified the upload; the user confirmed all five live app screens remain complete and readable. Version 1.1.0 is retained for recovery. Source is ../deskbot_firmware/deskbot_firmware.ino, with generated shared fallback_face.h. Toolchain: Arduino-Pico 6.3.0 / Arduino CLI 1.5.1. Checksums are in checksums.json.

Changes from 1.0.2: 5 fps limit advertised to the host, shared fallback face asset, and USB-suspend panel-off/reconnect handling (superseded by powered standby in 1.2.3). Orientation is controlled by the desktop app. Frame animation is host-rendered; installing another firmware is not needed for changes to the small companion face.

Version 1.0.2 and its source snapshot under ../archives/firmware-1.0.2 are retained for recovery. Version 1.0.1 is historical only: do not install it, because its idle timer can underflow and immediately blank the screen.

The damaged-panel diagnostic under ../archives/2026-10-11-damaged-oled is preserved and does not implement the Deskbot protocol. See ../COMMISSIONING.md for the observed results and outstanding physical acceptance.
