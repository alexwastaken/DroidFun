# DroidFun direct USB helper for macOS

This bundle contains picotool 2.0.0 and a dynamically linked libusb 1.0.27. Both binaries contain arm64 and x86_64 slices, with a minimum deployment target of macOS 13.0. The app can use the RP2040 ROM vendor USB interface without accessing its mounted filesystem. Binaries were built from the unmodified source archives included here; the only binary changes are the relative dylib install name and ad-hoc signing.

The October 25, 2026 build used Apple Clang 17.0.0 (clang-1700.0.13.3), macOS SDK 15.4, and CMake 3.31.6. Both architecture slices and their minimum OS load commands were inspected. Version/help execution was checked on Apple Silicon. This does not establish native Intel hardware or macOS 13 runtime validation. Ad-hoc signatures do not provide Developer ID signing or Apple notarization.

## Sources, notices, and rebuilding

The accompanying `SOURCE_SHA256SUMS` pins the exact inputs:

- `libusb-1.0.27-source.tar.bz2`: https://github.com/libusb/libusb/releases/download/v1.0.27/libusb-1.0.27.tar.bz2
- `picotool-2.0.0-source.tar.gz`: https://github.com/raspberrypi/picotool/archive/refs/tags/2.0.0.tar.gz
- `pico-sdk-2.0.0-source.tar.gz`: https://github.com/raspberrypi/pico-sdk/archive/refs/tags/2.0.0.tar.gz

`build-helper.sh` is the complete offline build/install recipe. From this directory, run `CMAKE=/path/to/cmake ./build-helper.sh /path/to/output`. In the source repository, `scripts/build-macos-usb-helper.sh` invokes that same recipe; without an output argument it replaces the bundled binaries. It requires macOS, Apple Command Line Tools, and CMake. No source patches or network downloads are performed. CMake 3.31.6 was obtained from the official Kitware macOS universal release and checked against its published SHA-256, `330b9514f5112e5ed4fb08b8b05803b776fd9b539a6ae12927d14dcc0ee2ba8d`.

The build uses universal architecture flags and the macOS 13 deployment target for both dependencies. It uses upstream's included precompiled `xip_ram_perms.elf` as designed by picotool's default build. The SDK archive has no mbedtls submodule, so optional RP2350 signing/hashing commands are absent; DroidFun's RP2040 read/info/load/readback operations do not use them. The recipe verifies both architecture slices and minimum OS commands, applies ad-hoc signatures, and executes only version/help. It never enumerates or opens a USB device. `BINARY_SHA256SUMS` records this build's signed binaries; final application signing can change those hashes. Toolchain updates may change binary hashes even with identical source.

Picotool and the used Pico SDK code are BSD-3-Clause (`LICENSE-picotool.txt`, `LICENSE-pico-sdk.txt`). Its bundled clipp and nlohmann/json dependencies are MIT (`LICENSE-clipp.txt`, `LICENSE-nlohmann-json.txt`). The whereami MIT notice is included as `LICENSE-whereami.txt`, obtained from https://raw.githubusercontent.com/gpakosz/whereami/master/LICENSE.MIT; its source is included in the pinned picotool archive.

libusb is LGPL-2.1-or-later; `LICENSE-libusb.txt` contains the full license. Its complete corresponding source and the build recipe accompany the library. `picotool` dynamically loads the adjacent `@executable_path/libusb-1.0.0.dylib`; there is no static libusb linkage or Homebrew dependency. A rebuilt compatible libusb can replace that file. On a private copy of the application, replacing a signed file requires re-signing that file and the enclosing application (ad-hoc signing is sufficient for local testing). Keep the source archive, recipe, and notices with distributions of these binaries. The remaining binary dependencies are Apple system libraries/frameworks.

## Device identity and verification

The app uses a fixed bundled firmware buffer. Discovery requires the ROM VID/PID, exact `RP2 Boot` product name, and device class 0. Runtime firmware can reuse the ROM VID/PID. ROM USB descriptor serials differ from flash IDs and are not unique. A known-board preflight selects its expected flash ID; initial setup requires exactly one ROM device before reading identity. Preflight confirms exactly one RP2040 and the expected flash identity when known.

The write uses `load -v -x --ser <verified-flash-ID>`. A successful process exit means readback verification succeeded; upstream throws a nonzero verification error for mismatched bytes. Its success message is `OK`, while `Verified OK` belongs to an unrelated OTP operation. See https://github.com/raspberrypi/picotool/blob/2.0.0/main.cpp#L3819-L3855. Explicit VID/PID command-line overrides are omitted because picotool 2.0.0 handles native RP2040 model detection differently when a PID is supplied. The normal app handshake separately verifies the returning firmware version and OLED status.

On erased flash, picotool may report one RP2040 without a flash ID: identity output is gated by a flash-size estimate that returns zero when the first two pages match. The app allows this only for first-time setup with exactly one ROM device before preflight and immediately before loading. It then uses the default single-device `load -v -x`, which independently rejects multiple accessible boot devices, and reports an explicit unknown identity for the parent's post-reboot check. Known-board updates still require their expected flash ID. Missing identity alone does not establish a hardware fault. See https://github.com/raspberrypi/picotool/blob/2.0.0/main.cpp#L2322-L2340 and https://github.com/raspberrypi/picotool/blob/2.0.0/main.cpp#L3053-L3061.
