# Standalone playback host tests

These tests compile the production playlist/storage headers and the complete production firmware sketch with in-memory substitutes for Arduino, USB, I²C, and LittleFS. They never access a serial device or flash hardware.

Run from the repository root with Clang:

```sh
clang++ -std=c++17 -Wall -Wextra -Werror -fsanitize=address,undefined -Ifirmware/tests/mocks firmware/tests/standalone_test.cpp -o /tmp/droidfun-standalone-test
/tmp/droidfun-standalone-test
clang++ -std=c++17 -Wall -Wextra -Werror -Wno-unused-function -fsanitize=address,undefined -Ifirmware/tests/mocks firmware/tests/standalone_lifecycle_test.cpp -o /tmp/droidfun-standalone-lifecycle-test
/tmp/droidfun-standalone-lifecycle-test
```

The storage test checks the DFP1 format, 30-screen and duration bounds, checksums, incomplete/out-of-order uploads, readback verification, torn writes, corrupt inactive slots, newest-valid-generation selection, and preservation of existing storage when mounting fails. The lifecycle test checks saved playback with no USB enumeration or DTR, frame cycling, USB suspension, host stalls, stale session packets, desktop handoff, and cancellation of partial uploads by a new handshake.

Passing host tests does not prove physical flash persistence, OLED output, USB electrical behavior, or power-loss resilience of the hardware filesystem. Verify those on a bot separately before claiming hardware acceptance.
