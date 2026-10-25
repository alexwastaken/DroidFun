# Third-party notices

The root MIT license applies to original DroidFun project source and documentation. It does not replace the licenses of dependencies, copied font data or third-party code incorporated into compiled firmware. Preserve the upstream notices when redistributing those components.

## Desktop dependencies

The exact JavaScript and Rust dependency versions are recorded in [`package-lock.json`](package-lock.json) and [`src-tauri/Cargo.lock`](src-tauri/Cargo.lock). These are downloaded during the build rather than vendored as source in this repository. Each dependency retains its own license; a binary distributor must include the notices required by the components in their actual build.

The desktop stack includes React, Tauri, Rust ecosystem crates and the font8x8 bitmap font. The copied font data and derived widget illustrations retain the existing [font8x8 MIT notice](docs/images/widget-ideas/FONT-LICENSE.txt), copyright 2017 Joaquin Rosales.

## Bundled Mac USB helper

App 0.4.2 includes universal arm64/x86_64 picotool 2.0.0 and dynamically linked libusb 1.0.27, built for macOS 13 or later. Their resources are in [`src-tauri/resources/picotool-macos-universal/`](src-tauri/resources/picotool-macos-universal/), including these notices and complete pinned source archives:

| Component | Corresponding source | License copy |
| --- | --- | --- |
| picotool 2.0.0 | [Source archive](src-tauri/resources/picotool-macos-universal/picotool-2.0.0-source.tar.gz) | [BSD-3-Clause](src-tauri/resources/picotool-macos-universal/LICENSE-picotool.txt) |
| Pico SDK 2.0.0, used by the helper build | [Source archive](src-tauri/resources/picotool-macos-universal/pico-sdk-2.0.0-source.tar.gz) | [BSD-3-Clause](src-tauri/resources/picotool-macos-universal/LICENSE-pico-sdk.txt) |
| libusb 1.0.27 | [Source archive](src-tauri/resources/picotool-macos-universal/libusb-1.0.27-source.tar.bz2) | [LGPL-2.1-or-later](src-tauri/resources/picotool-macos-universal/LICENSE-libusb.txt) |
| clipp, included in the pinned picotool archive | picotool source archive above | [MIT](src-tauri/resources/picotool-macos-universal/LICENSE-clipp.txt) |
| nlohmann/json, included in the pinned picotool archive | picotool source archive above | [MIT](src-tauri/resources/picotool-macos-universal/LICENSE-nlohmann-json.txt) |
| whereami, included in the pinned picotool archive | picotool source archive above | [MIT](src-tauri/resources/picotool-macos-universal/LICENSE-whereami.txt) |

[`SOURCE_SHA256SUMS`](src-tauri/resources/picotool-macos-universal/SOURCE_SHA256SUMS) records the exact source inputs. The [offline build recipe](src-tauri/resources/picotool-macos-universal/build-helper.sh) and [helper build notes](src-tauri/resources/picotool-macos-universal/README.md) describe the build, dependency checks, load-path adjustment and signatures. These source versions are for the host helper; the firmware's separate core/SDK versions are listed below.

Picotool dynamically loads the adjacent `@executable_path/libusb-1.0.0.dylib`; libusb is not statically linked and no Homebrew installation is required. A compatible rebuilt library can replace that file. Replacing it in a private signed app copy requires re-signing the replacement and enclosing app; ad-hoc signing is sufficient for local testing. Keep the source archives, recipe and notices with distributions of these helper binaries. The [firmware/helper rebuild guide](docs/FIRMWARE_SOURCE.md) provides the repository entry points. The remaining helper binary dependencies are Apple system libraries/frameworks.

## Firmware and UF2 binaries

The `.ino` and project headers are original project source under the root MIT license. App 0.4.2 embeds normal firmware 1.2.2 and a separate version-1 display diagnostic. The `.uf2` files are compiled combinations containing third-party runtime/library code; **the entire binary is not MIT licensed**.

The documented firmware toolchain is Arduino CLI 1.5.1 with Arduino-Pico 6.3.0. The following references come from that core's pinned source tree:

| Component | Source reference | License / notice |
| --- | --- | --- |
| Arduino-Pico core | [6.3.0](https://github.com/earlephilhower/arduino-pico/tree/6.3.0) | LGPL-2.1-or-later; [license copy](LICENSES/Arduino-Pico-LGPL-2.1.txt). Core copyright includes Earle F. Philhower, III and the upstream contributors. |
| ArduinoCore-API | [`82928635c893189343cf8eb78569f0c4136fded0`](https://github.com/earlephilhower/ArduinoCore-API/tree/82928635c893189343cf8eb78569f0c4136fded0) | LGPL-2.1; [license copy](LICENSES/ArduinoCore-API-LGPL-2.1.txt). Individual source-file notices also apply. |
| Raspberry Pi Pico SDK | [`079c6f39023649b154152db30f1d781e884879bc`](https://github.com/raspberrypi/pico-sdk/tree/079c6f39023649b154152db30f1d781e884879bc) | BSD-3-Clause; [license copy](LICENSES/Pico-SDK-BSD-3-Clause.txt). |
| littlefs | [`8e251dd675da00342d45dac78b6f627f119aed03`](https://github.com/littlefs-project/littlefs/tree/8e251dd675da00342d45dac78b6f627f119aed03) | BSD-3-Clause; [license copy](LICENSES/LittleFS-BSD-3-Clause.txt). |
| TinyUSB (Pico SDK submodule) | [`86ad6e56c1700e85f1c5678607a762cfe3aa2f47`](https://github.com/hathach/tinyusb/tree/86ad6e56c1700e85f1c5678607a762cfe3aa2f47) | MIT; [license copy](LICENSES/TinyUSB-MIT.txt). |
| Adafruit TinyUSB Arduino (available core submodule) | [`08c7dabed48b5ab92839289f58191b3f04f48fb0`](https://github.com/adafruit/Adafruit_TinyUSB_Arduino/tree/08c7dabed48b5ab92839289f58191b3f04f48fb0) | MIT; [license copy](LICENSES/Adafruit-TinyUSB-MIT.txt). Inclusion in a particular binary depends on the USB build configuration. |

Arduino-Pico contains other optional components and compiler/runtime dependencies. Its [licensing and credits](https://github.com/earlephilhower/arduino-pico/blob/6.3.0/README.md#licensing-and-credits), per-file headers and recursive submodules remain authoritative for those components. The table above identifies the firmware's documented core and principal linked libraries; it is not a claim that optional networking, Bluetooth or other libraries are used by this RP2040-Zero sketch.

The LGPL core remains available under its own license. See [firmware source and rebuild instructions](docs/FIRMWARE_SOURCE.md) for the project source, pinned upstream source, and a path to rebuild/relink with modified libraries. The root MIT license imposes no restriction on modifying the project or reverse engineering a combined work to debug such modifications.

## Services and names

Open-Meteo weather data, Yahoo Finance quotes and optional account integrations are external services with their own terms. Their data and brands are not relicensed by this repository. The app displays Open-Meteo attribution and links to its terms. Claude, Codex, Raspberry Pi, Arduino and other third-party names identify compatibility; they do not imply sponsorship or endorsement.
