# DroidFun App 0.7.0 verification

All native packages were built from `b6d347d1f977db709ff4379af3294b03f912e5d4`.

- [Package verification](package-verification.json): 131 native tests on Mac, 125 each on Windows x64, Windows ARM64 and Linux; exact package hashes, Mac/Windows updater signatures and native package checks.
- [Mac universal inspection](macos-load-command-verification.json): both architecture slices target macOS 13; strict ad-hoc signature verification passed.
- [Real Mac upgrade](mac-app-upgrade.json): installed 0.6.0 to 0.7.0 through the public updater, automatic restart, exact published executable and unchanged settings.
- [Customer UI fixtures](ui-verification.json): 24 scenarios, including all four 3D models, reduced motion and WebGL fallback.
- [Registry security](registry-security-verification.json): 58 website tests, including nine security tests; live anonymous access denied to inventory routes, model-only public lookup, and actual owner pairing/native Workshop sync.

[Mac CI](https://github.com/alexwastaken/DroidFun/actions/runs/40731707194) and [Windows/Linux CI](https://github.com/alexwastaken/DroidFun/actions/runs/40732069700) built the published prerelease. All five website installers and Guide 2.4 were subsequently downloaded and hash-checked. [Current downloads](../../../distribution/current.json) record their exact files.

No physical droid was attached. USB recognition, readable-screen confirmation, sustained USB stability, power cycling, saved screens on wall power and sleep/wake remain physical acceptance checks. Mac notarization, Windows publisher signing and a real Windows in-app upgrade remain open. Simulated USB/browser fixtures do not establish hardware acceptance.
