# Firmware source and rebuilding

The desktop app embeds `firmware/releases/droidfun-rp2040-zero-gp4-gp5-v1.2.0.uf2` for the explicit **Export firmware** action. This file is a compiled work containing both DroidFun source and third-party components. See [third-party notices](../THIRD_PARTY_NOTICES.md) for their separate licenses.

## Source for the current firmware

- [Production sketch](../firmware/deskbot_firmware/deskbot_firmware.ino)
- [Fallback face](../firmware/deskbot_firmware/fallback_face.h)
- [Playlist encoding](../firmware/deskbot_firmware/standalone_playlist.h)
- [Persistent storage](../firmware/deskbot_firmware/standalone_storage.h)
- [Host tests and hardware substitutes](../firmware/tests/)
- [Recorded checksums](../firmware/releases/checksums.json)

Firmware 1.2.0 entered the project in [commit `2da685d`](https://github.com/alexwastaken/DroidFun/tree/2da685d/firmware/deskbot_firmware). The firmware source and current UF2 are retained in this repository. Historical versions and diagnostics have separate records under `firmware/archives/` and `firmware/releases/`; they should not be substituted for the current build.

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
```

For a locally modified Git core, select the equivalent Waveshare RP2040-Zero board from that installation and retain the **2 MiB flash / 256 KiB filesystem** partition. Confirm the FQBN with `arduino-cli board listall`; the platform prefix can differ for a manually installed core. Compile the same project sketch with that FQBN.

The hardware profile is SSD1306 128 × 64, I²C address `0x3C`, SDA GPIO 4 and SCL GPIO 5. Changing the flash partition on an existing bot can invalidate its saved playlist. These commands build a UF2 and do not upload it to a board.

To build the desktop app with your newly compiled firmware, replace the embedded UF2 at the path at the top of this document, record its new checksum, and rebuild the desktop app. Keep the source, toolchain choices, notices and binary together when distributing a modified build. A successful compile is not physical hardware acceptance, and reproduction of a byte-identical historical artifact has not been asserted here.
