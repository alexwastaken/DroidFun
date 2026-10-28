# DroidFun App 0.8.0 verification

Application source: `0ad2d9eea69b9a12b2b31da1556b6b524c272760`.

Nimbus, Boo and Meep can add stationary or scrolling custom text and locally converted image screens. Claw retains its Claude/Codex screens. Firmware is unchanged, including swapped-wiring firmware.

- [Custom screen UI](custom-screens-ui.json): text motion/speed, live preview refresh, real PNG conversion, saved-image editing, invalid-file rejection, enabled/listed images, static-save counts, and Claw restrictions across all four models.
- [Image conversion](image-conversion.json): ten checks for PNG/JPG/WebP decoding, exact OLED packing, pre-decode file/dimension limits, dithering and transparent black/white icons.
- [UI regression](ui-verification.json): 32 existing browser fixture scenarios, including model recognition, screen proportions, connection/recovery states and update controls.
- [Package verification](package-verification.json): exact-source installer hashes, native tests, signed updater verification, and native installed-package smoke checks.
- [Mac inspection](macos-load-command-verification.json): universal arm64/x86_64 slices, minimum macOS 13 and strict signature verification.
- [Website downloads](website-downloads.json): all five public installers and unchanged Guide 2.4 matched expected filenames, byte sizes and SHA-256 hashes.
- [Real Mac update](mac-app-upgrade.json): installed 0.7.1 → 0.8.0 updater acceptance.

Native workflows: [Mac](https://github.com/alexwastaken/DroidFun/actions/runs/40757898973), [Windows and Linux](https://github.com/alexwastaken/DroidFun/actions/runs/40757904330).

Settings migrate to schema 3 with an owner-only archive of the original schema-2 file before first replacement. Older apps preserve and refuse newer settings. Still images, static text and faces use the existing saved-playlist protocol; scrolling is excluded because it needs desktop rendering. Browser fixtures mock native commands and USB; native rendering, storage and saved-frame tests run separately. No physical USB display transfer or wall-power acceptance is asserted. Mac notarization, Windows publisher signing and a real Windows updater test remain open.
