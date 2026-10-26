# Screen animation acceptance

DroidFun App 0.5.3 and firmware 1.2.4 target up to 30 animation updates per second on Meep and Claw's shared RP2040-Zero / SSD1306 GP4–GP5 profile.

On a connected Claw, a moving-footer test with full 1024-byte host frames measured:

| Firmware | Duration | Verified frames | Measured FPS |
| --- | ---: | ---: | ---: |
| 1.2.3 | 15 seconds | 74 | 4.94 |
| 1.2.4 | 30 seconds | 882 | 29.40 |

Every acknowledged frame matched the host bitmap's CRC. The optimized run reported ready screen health throughout; its maximum interval between acknowledgements was 53.56 ms. These are measured transfer rates for that workload, not a promise that every screen change will take exactly 33 ms.

The firmware uses the SSD1306's documented 400 kHz I2C timing and writes only changed 16-byte display regions. A full-screen change can take longer. Failed fast initialization or a failed transfer downgrades to 100 kHz / 5 FPS until restart. Partial transfer failures invalidate the screen cache and retry the entire frame. Both standard display addresses are supported. See [SSD1306 Table 13-6](https://cdn-shop.adafruit.com/datasheets/SSD1306.pdf).

The host follows the board's advertised limit, including a downgrade reported by a frame acknowledgement. Transfer time counts inside the frame interval. Older firmware retains its own limit. A 60 Hz engine tick samples animation; it does not mean 60 physical screen updates per second. Fractional tick time is preserved so animation and playlist timers do not run slowly.

Codex and Claude have a short animated accent confined to the logo corner. Labels, readings and bars stay fixed. Pausing, reduced motion and disabling animation stop it. Usage data polling remains independent: Codex checks about once a minute.

Firmware sanitizer tests cover cached identical frames, separated changed regions, simulated panel contents, partial-transfer fallback, slow-only startup and missing screens. Native tests also check frequent distinct animation frames, stationary data and fractional timer accumulation. Physical USB throughput is measured separately from visual confirmation and full computer sleep/wake testing.
