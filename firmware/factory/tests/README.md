# Firmware host tests

These tests compile the production playlist/storage headers and the complete production firmware sketch with in-memory substitutes for Arduino, USB, I²C, and LittleFS. They never access a serial device or flash hardware.

Run from the repository root with Clang:

```sh
clang++ -std=c++17 -Wall -Wextra -Werror -fsanitize=address,undefined -Ifirmware/factory/tests/mocks firmware/factory/tests/standalone_test.cpp -o /tmp/droidfun-standalone-test
/tmp/droidfun-standalone-test
clang++ -std=c++17 -Wall -Wextra -Werror -Wno-unused-function -fsanitize=address,undefined -Ifirmware/factory/tests/mocks firmware/factory/tests/standalone_lifecycle_test.cpp -o /tmp/droidfun-standalone-lifecycle-test
/tmp/droidfun-standalone-lifecycle-test
clang++ -std=c++17 -Wall -Wextra -Werror -Wno-unused-function -fsanitize=address,undefined -Ifirmware/factory/tests/mocks firmware/factory/tests/oled_address_test.cpp -o /tmp/droidfun-oled-address-test
/tmp/droidfun-oled-address-test
clang++ -std=c++17 -Wall -Wextra -Werror -Wno-unused-function -fsanitize=address,undefined -Ifirmware/factory/tests/mocks firmware/factory/tests/packet_bounds_test.cpp -o /tmp/droidfun-packet-bounds-test
/tmp/droidfun-packet-bounds-test
clang++ -std=c++17 -Wall -Wextra -Werror -Wno-unused-function -fsanitize=address,undefined -Ifirmware/factory/tests/mocks firmware/factory/tests/identity_test.cpp -o /tmp/droidfun-identity-test
/tmp/droidfun-identity-test
```

The storage test checks the DFP1 format, 30-screen and duration bounds, checksums, incomplete/out-of-order uploads, readback verification, torn writes, corrupt inactive slots, newest-valid-generation selection, and preservation of existing storage when mounting fails. The lifecycle test checks saved playback with no USB enumeration or DTR, frame cycling, USB suspension, host stalls, stale session packets, desktop handoff, and cancellation of partial uploads by a new handshake.

Saved-content regressions ensure normalized CRCs stay stable across storage
generations, change with pixels, duration and display settings, and appear in
both persisted save ACKs and a fresh HELLO. CRCs of full valid DFP1 files have a
constant residue and are not accepted as content identity.

Passing host tests does not prove physical flash persistence, OLED output, USB electrical behavior, or power-loss resilience of the hardware filesystem. Verify those on a bot separately before claiming hardware acceptance.

The OLED test exercises the production initialization/frame code at 0x3C and 0x3D, no-response failure, and recovery when a display becomes available again.

The packet-bounds test covers the exact 1044-byte decoded capacity, excess copied
and implicit-zero bytes, malformed source blocks, and recovery into a valid
full-size frame in the same serial batch. It also verifies that a fresh board
boots into its fallback face, malformed input preserves the board identity, and
all-zero/all-one frames and an explicit fallback request remain supported.

The identity test covers the factory record and explicit provisioning command:
model/profile/schema bounds, immutable UID binding, session checks, persisted
readback, duplicate/idempotent requests, truncated/corrupt inactive slots, newest
valid generation, unchanged customer playlists, and refusal to format nonblank
storage. Provisioning restores a visible live face; an absent OLED remains a
separate display error even if identity storage succeeds. Re-loading mock files
is not physical power-cycle acceptance.

`blinking_standby_test.cpp` covers timed eyelids, mouth preservation, factory saved-face recognition, wraparound, unchanged flash data, host ownership, suspension, manual off and custom-image isolation. Build/run it with the same sanitizer flags.
