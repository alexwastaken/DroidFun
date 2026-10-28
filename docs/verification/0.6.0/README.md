# DroidFun App 0.6.0 verification

All native packages were built from `a5593ec9e68696e9e43dac1cb51c3c85361ed8f0`.

- [CI results](ci-verification.json): 127 native tests passed on Mac; 121 each on Windows x64, Windows ARM64 and Linux x86_64.
- [Package verification](package-verification.json): exact artifact sizes, hashes and Mac/Windows updater signatures; installed-package checks on Windows/Linux.
- [Mac universal inspection](macos-load-command-verification.json): both architecture slices target macOS 13; strict ad-hoc signature verification passed.
- [Real Mac upgrade](mac-app-upgrade.json): 0.5.5 to 0.6.0 through the public updater, automatic restart, exact published executable, unchanged settings and a successful follow-up check.

The CI record describes verification while the release was still a draft. The release was subsequently published, the updater feed was updated, and all five website installers plus Guide 2.3 were downloaded and hash-checked. Current downloads are listed in [the distribution manifest](../../../distribution/current.json).

No physical droid was available for this acceptance run. Real-device recognition, readable-screen confirmation, USB stability, wall-power saved screens and sleep/wake remain separate checks. Mac notarization, Windows publisher signing and a real Windows in-app upgrade also remain open.
