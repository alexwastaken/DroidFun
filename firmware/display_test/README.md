# Meep standalone display test

This temporary diagnostic sketch runs independently of DroidFun app frames. Build
for **Waveshare RP2040 Zero**, with the documented Meep display wiring: GP4 SDA,
GP5 SCL, 3.3 V and GND. It uses only those I2C pins and the board's GP16 RGB LED.
It does not erase or write the filesystem, and the Arduino-Pico 1200-baud USB
restart remains available to restore regular firmware with the case closed.

The sketch probes only `0x3C` and `0x3D` at 100 kHz. For a responding address it
alternates SSD1306 and SH1106 initialization: two seconds of all pixels on at low
contrast, then six seconds of an 8-pixel checkerboard. The RGB LED pulses green
when display writes succeed and red when they fail; it starts blue. The LED may
be hidden inside the enclosure. An ACK or successful write does not prove the
physical panel is visible. If both addresses respond, `0x3C` is tested first.

Wire writes have a 20 ms timeout and stop on the first failed transaction. The
Arduino-Pico empty-write probe uses bounded clock-stretch waits. There is no
automatic search across other GPIOs, no serial command interface, and no need for
a connected computer to animate the display.

USB serial at 115200 emits one newline-delimited JSON object each second when a
reader is connected. Example:

```json
{"kind":"droidfun_display_test","version":1,"boardId":"53032847490B8B1C","uptime_ms":1250,"sda":1,"scl":1,"ack_3c":false,"ack_3d":false,"address":null,"phase":"no_ack","write_ok":false,"rgb_ready":true}
```

`sda`/`scl` are idle logic levels sampled before probing; high levels do not prove
a panel is connected. `address` is `null`, `60` (0x3C), or `61` (0x3D). `phase` is
`starting`, `no_ack`, `write_error`, `ssd1306_all_on`, `ssd1306_checker`,
`sh1106_all_on`, or `sh1106_checker`. `rgb_ready` confirms only that the LED PIO
driver initialized. `write_ok` describes the most recent display step.

Build from the repository workspace root:

```sh
work/standalone-firmware/arduino-cli compile \
  --config-file work/standalone-firmware/arduino.yaml \
  --fqbn rp2040:rp2040:waveshare_rp2040_zero:flash=2097152_262144 \
  --build-path work/meep-display-test-build \
  --output-dir work/meep-display-test \
  work/private-app/DroidFun/firmware/display_test
```

Command references: [SSD1306 datasheet](https://cdn-shop.adafruit.com/datasheets/SSD1306.pdf)
for `A5`/`A4`, and [Adafruit's SH1106 driver](https://github.com/adafruit/Adafruit_SH110x/blob/master/Adafruit_SH1106G.cpp)
for the SH1106 DC/DC initialization and page offset. The bundled Arduino-Pico
6.3.0 board variant defines the I2C and RGB pins, and its Wire implementation
defines the timeout and bounded probe behavior used here.
