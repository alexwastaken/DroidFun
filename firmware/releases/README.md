# DroidFun firmware builds

Use **droidfun-rp2040-zero-gp4-gp5-v1.2.0.uf2** for the connected RP2040-Zero / SSD1306 profile: SDA GP4, SCL GP5, OLED 0x3C, 128×64.

Version 1.2.0 adds persistent saved text/face playlists for wall USB power. The app exports this embedded UF2 through Settings → Robot. It reserves 256 KiB of flash for two checksummed playlist slots; an incomplete upload leaves the current slot intact. Only an explicit save can write flash. The firmware never automatically formats a nonblank filesystem. This build has compiled successfully and is covered by host-side protocol/storage tests; physical unplug/reboot/wall-charger acceptance still needs checking on a bot.

Rebuild with Arduino CLI 1.5.1 and Arduino-Pico 6.3.0:

```sh
arduino-cli compile --fqbn rp2040:rp2040:waveshare_rp2040_zero:flash=2097152_262144 firmware/deskbot_firmware
```

Keep the same flash partition setting on later updates to preserve saved playlists. Never run a full flash erase as part of an ordinary app update.

Version 1.1.0 was installed on replacement board 505431651938581C. Arduino CLI verified the upload; the user confirmed all five live app screens remain complete and readable. Version 1.1.0 is retained for recovery. Source is ../deskbot_firmware/deskbot_firmware.ino, with generated shared fallback_face.h. Toolchain: Arduino-Pico 6.3.0 / Arduino CLI 1.5.1. Checksums are in checksums.json.

Changes from 1.0.2: 5 fps limit advertised to the host, shared fallback face asset, and USB-suspend panel-off/reconnect handling. Orientation is controlled by the desktop app. Frame animation is host-rendered; installing another firmware is not needed for changes to the small companion face.

Version 1.0.2 and its source snapshot under ../archives/firmware-1.0.2 are retained for recovery. Version 1.0.1 is historical only: do not install it, because its idle timer can underflow and immediately blank the screen.

The damaged-panel diagnostic under ../archives/2026-10-11-damaged-oled is preserved and does not implement the Deskbot protocol. See ../COMMISSIONING.md for the observed results and outstanding physical acceptance.
