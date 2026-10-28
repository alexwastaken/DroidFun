# DroidFun App 0.10.0 verification

Application source: `1722ba3b56ab5a25225d86969fa234f661070c46`.

The top-of-app switcher provides previous/next arrows and a named droid menu. Native admission permits four attached droids, keeps their slots stable across selection and USB enumeration changes, and admits a waiting fifth only after a slot becomes free. Remembered history remains compatible; unplugging never deletes a profile. The selected droid receives live frames; others show saved screens or their fallback face.

- [Switcher browser checks](switcher-ui.json): ten scenarios cover four models, arrows and wraparound, keyboard activation, duplicate names, fifth-droid blocking, unplugged selection, first setup, long names/dark mode, failed selection and diagnostic exclusion. Native/USB calls are simulated.
- [Regression UI](ui-verification.json), [custom screens](custom-screens-ui.json), [image conversion](image-conversion.json) and [support UI](support-ui.json): 56 existing checks passed.
- [Package verification](package-verification.json): exact-source package hashes, native tests, installed-package smoke checks and signed updater verification.
- [Mac inspection](macos-load-command-verification.json): universal arm64/x86_64, minimum macOS 13 and strict code-signature verification.
- [Installed Mac acceptance](mac-app-upgrade.json): real 0.9.0 → 0.10.0 update, automatic relaunch, exact executable, unchanged settings, top switcher with no boards, and fresh update check.
- [Website downloads](website-downloads.json): six public downloads matched expected filenames, sizes and SHA-256 hashes.

Native tests cover slot stability, releasing/reusing slots, case-insensitive USB identities/aliases, preserving the initial selected board, all four models restoring their own screens/display settings, and older profile history with more than four saved droids. Firmware and settings schema 3 are unchanged. Native workflows: [Mac](https://github.com/alexwastaken/DroidFun/actions/runs/40772611169), [Windows/Linux](https://github.com/alexwastaken/DroidFun/actions/runs/40770881293).

No physical board was attached during this acceptance. Four-droid USB switching, physical screen output and wall-power playback need hardware acceptance. Mac notarization, Windows publisher signing and a real Windows updater run remain open.
