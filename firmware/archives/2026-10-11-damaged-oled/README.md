# Saved damaged-screen diagnostic

Saved October 11, 2026 at the user's request, after they identified the original OLED as broken and connected a replacement robot.

- Original RP2040-Zero serial: `5054316558976B9C`.
- Wiring supplied by user: SDA GP4; SCK/SCL GP5; I²C address `0x3C`.
- Source: `oled_diagnostic/oled_diagnostic.ino`.
- Flashable build: `oled_diagnostic.ino.uf2`, compiled with Arduino CLI 1.5.1, Arduino-Pico 6.3.0, board `rp2040:rp2040:waveshare_rp2040_zero`.
- Libraries: Adafruit SSD1306 2.5.17, Adafruit GFX 1.12.6, Adafruit BusIO 1.17.4.
- Photograph: `damaged-screen.jpeg`. The photograph is sideways; the OLED text orientation was correct.
- `checksums.json` contains SHA-256 for the saved source, UF2, and photo. Each saved file was checked against its original during copying.

The diagnostic initially lights every pixel at reduced contrast. Its USB serial commands at 115200 baud are `t` (border/text), `a` (all pixels on), `c` (sequential COM mapping), and `n` (default COM mapping). It uses GP4/GP5 and 100 kHz I²C. It does not answer the Deskbot app protocol.

The original panel displayed incomplete HELLO letters and a broken border with both horizontal and explicit page addressing. The user subsequently reported the screen was broken. This archive preserves the test for future comparisons; it is not a complete backup of the original ALIEN_SMILES sketch.
