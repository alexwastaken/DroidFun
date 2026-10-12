# Initial hardware observations

`IMG_0308.HEIC` shows the assembled, USB-powered robot displaying a blue face. `IMG_0299.HEIC` shows its opened enclosure, a four-wire OLED, and a spare display with pads marked **GND, VDD, SCK, SDA**. These labels are observations from the hardware, not instructions. The controller's exact board marking, GPIO wiring, OLED supply voltage, and I²C address are not legible in the photos. The proposed software therefore treats those as values to confirm on the physical robot.

- The [Waveshare RP2040-Zero](https://www.waveshare.com/wiki/RP2040-Zero) uses an RP2040 with dual Cortex-M0+ cores, 264 KB RAM, 2 MB flash, USB-C, and two I²C controllers. Its USB port can provide a device connection to a host computer. The RP2040-Zero has no built-in Wi-Fi; the host computer should fetch internet data.
- [MicroPython publishes a board-specific RP2040-Zero UF2](https://micropython.org/download/WAVESHARE_RP2040_ZERO/) and documents BOOTSEL flashing. Its [SSD1306 driver guide](https://docs.micropython.org/en/latest/esp8266/tutorial/ssd1306.html) supports a 128×64 I²C display, with `0x3C` as a common default address. An I²C scan must confirm the actual address. The full product specification selects the Pico C/C++ SDK for production firmware.
- A 128×64 monochrome frame uses 1,024 bytes. This fits the RP2040's RAM, though I²C refresh and OLED persistence favor modest animation rates. The display is suited to faces and a few short lines of text.
- The AITRIP listing in the request identifies the OLED as SSD1306. The photo's four pad labels are consistent with an I²C module. Its specific regulator, pull-ups, and allowed VDD voltage have **not** been verified from a maker datasheet. Confirm the actual module before rewiring or powering it.

## Connected-device findings, October 11

The user confirms RP2040-Zero, SDA on GP4, and SCK/SCL on GP5. USB now enumerates the Waveshare board after changing to a data cable. The prior sketch reported `ALIEN_SMILES v1` and an OLED at `0x3C`. The user authorized overwriting it without a backup. Firmware uploads and Deskbot USB frame acknowledgments succeeded, but the user reports a blank OLED. Physical display integration is therefore **failing**, with a persistent independent panel diagnostic currently installed. See [commissioning record](firmware/COMMISSIONING.md).

Power wiring, pull-up voltage, and actual panel controller remain unverified. The desktop app does not automatically flash devices; the authorized diagnostic and firmware uploads were performed separately during commissioning. The firmware now uses Arduino-Pico C/C++ to match the existing Arduino workflow.
