# Firmware host tests

These tests compile the production playlist/storage headers and the complete production firmware sketch with in-memory substitutes for Arduino, USB, I²C, and LittleFS. They never access a serial device or flash hardware.

Run from the repository root with Clang:

```sh
clang++ -std=c++17 -Wall -Wextra -Werror -fsanitize=address,undefined -Ifirmware/tests/mocks firmware/tests/standalone_test.cpp -o /tmp/droidfun-standalone-test
/tmp/droidfun-standalone-test
clang++ -std=c++17 -Wall -Wextra -Werror -Wno-unused-function -fsanitize=address,undefined -Ifirmware/tests/mocks firmware/tests/standalone_lifecycle_test.cpp -o /tmp/droidfun-standalone-lifecycle-test
/tmp/droidfun-standalone-lifecycle-test
clang++ -std=c++17 -Wall -Wextra -Werror -Wno-unused-function -fsanitize=address,undefined -Ifirmware/tests/mocks firmware/tests/oled_address_test.cpp -o /tmp/droidfun-oled-address-test
/tmp/droidfun-oled-address-test
clang++ -std=c++17 -Wall -Wextra -Werror -Wno-unused-function -fsanitize=address,undefined -Ifirmware/tests/mocks firmware/tests/packet_bounds_test.cpp -o /tmp/droidfun-packet-bounds-test
/tmp/droidfun-packet-bounds-test
```

The storage test checks the DFP1 format, 30-screen and duration bounds, checksums, incomplete/out-of-order uploads, readback verification, torn writes, corrupt inactive slots, newest-valid-generation selection, and preservation of existing storage when mounting fails. The lifecycle test checks saved playback with no USB enumeration or DTR, frame cycling, USB suspension, host stalls, stale session packets, desktop handoff, and cancellation of partial uploads by a new handshake.

Passing host tests does not prove physical flash persistence, OLED output, USB electrical behavior, or power-loss resilience of the hardware filesystem. Verify those on a bot separately before claiming hardware acceptance.

The OLED test exercises the production initialization/frame code at 0x3C and 0x3D, no-response failure, and recovery when a display becomes available again.

The packet-bounds test covers the exact 1044-byte decoded capacity, excess copied
and implicit-zero bytes, malformed source blocks, and recovery into a valid
full-size frame in the same serial batch. It also verifies that a fresh board
boots into its fallback face, malformed input preserves the board identity, and
all-zero/all-one frames and an explicit fallback request remain supported.

The powered-standby test compiles the full shared firmware used by Meep, Boo and Claw. It checks panel power and face pixels during unenumerated wall power, USB suspend, long idle and timer rollover, then verifies same-session and new-session frame handoff. Explicit screen-off remains authoritative. Run it with the same sanitizer command as `packet_bounds_test`, replacing its name with `powered_standby_test`.
