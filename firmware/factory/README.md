# Factory firmware 1.3.3

This is the reproducible standard and Mistake-wiring factory firmware source used by Workshop. The initial source was verified against every SHA-256 in `../releases/droidfun-rp2040-zero-sda5-scl4-v1.3.2-mistake.build.json` before being copied from the prior Workshop checkout. The public customer recovery sketch remains separate.

1.3.3 adds a 200 ms blink after four seconds to the fallback smile and any saved screen exactly matching the factory smile. Other saved pixels, durations, brightness, orientation and persistent identity/storage formats are preserved. The animation writes only RAM/display memory; it does not repeatedly write flash. Desktop frames and explicit screen-off take priority. USB suspend and host timeout use the same blinking standby face.

Build with Arduino CLI 1.5.1, Arduino-Pico 6.3.0 and `rp2040:rp2040:waveshare_rp2040_zero:flash=2097152_262144`. Set `compiler.cpp.extra_flags=-DDROIDFUN_MISTAKE_WIRING` only for the Mistake build. Keep the 256 KiB filesystem partition. See tests/README.md for the mock hardware tests. Physical blink and power-cycle acceptance must be checked on a droid.
