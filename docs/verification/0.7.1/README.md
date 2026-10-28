# DroidFun App 0.7.1 verification

Application source: `386967a5491ecc0a77dfa05cce0891e21eb876ff`.

This release corrects the screen-to-body proportions in the illustrated live previews and picker portraits for Nimbus, Boo, Meep and Claw. The figure is more compact, and Enlarge screen opens a separate readable view of the same 128 × 64 pixels. Firmware and saved settings are unchanged.

- [UI verification](ui-verification.json): 32 browser fixture scenarios, including every model at 360/500 px, screen containment, 2:1 canvas aspect, exact enlarged-frame pixels and no overflow. No physical USB device is asserted by these fixtures.
- [Package verification](package-verification.json): source-matched native package hashes, passing native tests, Mac/Windows updater signatures and native installer checks.
- [Mac inspection](macos-load-command-verification.json): both universal architecture slices, minimum macOS 13 and strict ad-hoc signature verification.

Native workflows: [Mac](https://github.com/alexwastaken/DroidFun/actions/runs/40738454863), [Windows and Linux](https://github.com/alexwastaken/DroidFun/actions/runs/40738459891). The existing Guide 2.4 remains accurate and is unchanged.

[Real Mac update](mac-app-upgrade.json): the installed 0.7.0 app updated to 0.7.1 through the public feed, restarted automatically, matched the published executable and preserved settings. The native proportional preview, enlarge/close control and follow-up update check passed. All five website installers and unchanged Guide 2.4 were downloaded and hash-checked.

Mac notarization, Windows publisher signing and hardware acceptance remain separate from this visual correction.
