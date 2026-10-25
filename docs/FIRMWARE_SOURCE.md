# Firmware source and rebuilding

DroidFun App 0.4.2 embeds `firmware/releases/droidfun-rp2040-zero-gp4-gp5-v1.2.2.uf2` for explicit USB setup/update and **Export firmware**. It separately embeds `firmware/releases/droidfun-display-test-v1.uf2` for the temporary screen diagnostic. These files are compiled works containing both DroidFun source and third-party components. See [third-party notices](../THIRD_PARTY_NOTICES.md) for their separate licenses.

## Source for the current firmware

- [Production sketch](../firmware/deskbot_firmware/deskbot_firmware.ino)
- [Fallback face](../firmware/deskbot_firmware/fallback_face.h)
- [Playlist encoding](../firmware/deskbot_firmware/standalone_playlist.h)
- [Persistent storage](../firmware/deskbot_firmware/standalone_storage.h)
- [Host tests and hardware substitutes](../firmware/tests/)
- [Standalone diagnostic sketch and report format](../firmware/display_test/README.md)
- [Recorded checksums](../firmware/releases/checksums.json)

Firmware 1.2.2 retains saved playlists and the 1.2.1 OLED address compatibility check. It fixes a decoded-packet capacity error that allowed an implicit COBS zero to overwrite one byte beyond the receive buffer. Its host regression test covers the malformed input, board identity, recovery into the next valid frame, full-size frame boundaries and a fresh-board fallback face. This is not a claim that the change fixes physical USB re-enumeration.

Firmware 1.2.0 entered the project in [commit `2da685d`](https://github.com/alexwastaken/DroidFun/tree/2da685d/firmware/deskbot_firmware). The current source and UF2 are retained in this repository. Historical versions and diagnostics have separate records under `firmware/archives/` and `firmware/releases/`; they should not be substituted for the current build. See [USB setup acceptance](USB_SETUP.md) for the 1.2.1 diagnostic history, confirmed 1.2.2 in-app update and physical screen, and remaining USB stability checks.

## Obtain the pinned core and library source

The release records identify **Arduino CLI 1.5.1** and **Arduino-Pico 6.3.0**. This command retrieves the core source plus its exact recursive submodules, including ArduinoCore-API, Pico SDK, TinyUSB and littlefs:

```sh
git clone --branch 6.3.0 --recurse-submodules https://github.com/earlephilhower/arduino-pico.git arduino-pico-6.3.0
git -C arduino-pico-6.3.0 submodule status --recursive
```

The 6.3.0 core tree is [`0a3b802122dabc3135f010fe55367af63b20ee0f`](https://github.com/earlephilhower/arduino-pico/tree/6.3.0). Component commits are listed in [third-party notices](../THIRD_PARTY_NOTICES.md). Preserve the core's license files and source notices when copying it.

The upstream [installation instructions](https://github.com/earlephilhower/arduino-pico/tree/6.3.0#installing-via-git) describe installing that checkout as an Arduino hardware core and fetching its matching toolchain with `tools/get.py`. A Git installation is the route for modifying the core or its libraries and relinking the sketch. Keep this separate from a Boards Manager installation to avoid selecting the wrong core.

## Compile without flashing

With Arduino CLI 1.5.1 installed, the standard Boards Manager route is:

```sh
arduino-cli core update-index --additional-urls https://github.com/earlephilhower/arduino-pico/releases/download/global/package_rp2040_index.json
arduino-cli core install rp2040:rp2040@6.3.0 --additional-urls https://github.com/earlephilhower/arduino-pico/releases/download/global/package_rp2040_index.json
arduino-cli compile --fqbn rp2040:rp2040:waveshare_rp2040_zero:flash=2097152_262144 --output-dir build/firmware firmware/deskbot_firmware
arduino-cli compile --fqbn rp2040:rp2040:waveshare_rp2040_zero:flash=2097152_262144 --output-dir build/display-test firmware/display_test
```

For a locally modified Git core, select the equivalent Waveshare RP2040-Zero board from that installation and retain the **2 MiB flash / 256 KiB filesystem** partition. Confirm the FQBN with `arduino-cli board listall`; the platform prefix can differ for a manually installed core. Compile the same project sketch with that FQBN.

The production hardware profile is SSD1306 128 × 64, I²C address `0x3C` or `0x3D`, SDA GPIO 4 and SCL GPIO 5. The diagnostic also tries SH1106 initialization using those same pins. Changing the flash partition on an existing bot can invalidate its saved playlist. These commands build UF2 files and do not upload them to a board.

To build the desktop app with your newly compiled firmware, replace the corresponding embedded UF2, record its new checksum in `firmware/releases/checksums.json`, and update the embedded-firmware hash check in `src-tauri/src/firmware_setup.rs` before rebuilding the app. Versioned filenames and the firmware's reported version must agree with the source being distributed. Keep the source, toolchain choices, notices and binary together when distributing a modified build. A successful compile is not physical hardware acceptance, and reproduction of a byte-identical historical artifact has not been asserted here.

## Rebuild the Mac USB helper

The desktop's USB installer is a separate host executable, not part of the UF2. App 0.4.2 bundles universal arm64/x86_64 picotool 2.0.0 and dynamically linked libusb 1.0.27 for macOS 13+. Their [resource directory](../src-tauri/resources/picotool-macos-universal/) includes pinned complete source archives, source and binary SHA-256 manifests, license copies and an offline build recipe.

On macOS with Apple Command Line Tools and CMake available, run from the repository root:

```sh
CMAKE=/absolute/path/to/cmake scripts/build-macos-usb-helper.sh /absolute/path/to/helper-output
```

The output argument keeps a rebuilt copy separate from the app's bundled files; omitting it replaces the bundled binaries. The recipe verifies source hashes, builds both architectures with a macOS 13 deployment target, sets the adjacent libusb load path, checks load commands/dependencies, ad-hoc signs the results, and runs version/help only. It never opens a USB device. See the [helper README](../src-tauri/resources/picotool-macos-universal/README.md) for the recorded build toolchain and how to replace/re-sign a compatible rebuilt libusb in a private app copy.

Helper architecture and build checks do not establish a successful device installation on Intel hardware or macOS 13. Native CI and final installer acceptance remain separate release checks.
