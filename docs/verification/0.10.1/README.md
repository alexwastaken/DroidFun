# 0.10.1 repair candidate — local Mac acceptance

This is a locally installed arm64 repair build, not a public release. The website and updater remain on 0.10.0. Apple Developer ID credentials, notarization, a fresh browser-download Gatekeeper check and public cross-platform package acceptance are still required before publishing 0.10.1. See [Mac signing setup](../../MAC_SIGNING.md).

## Changes

- Busy USB ports explain that another app has control, instead of implying absent firmware. Retrying retains the useful diagnosis, and initial firmware identification allows more startup time.
- An unplugged saved selection is shown honestly. If another droid is attached, a visible selector lets the customer choose it without silently replacing the current droid or its settings.
- Diagnostics distinguish USB contention from missing hardware and disable firmware recovery based on stale connection evidence.
- Mac Finder/Dock reopen restores the controls while the existing USB worker continues. Startup errors such as newer settings show a native explanation instead of returning an error through the native launch callback.
- Public Mac packaging requires Developer ID signing and notarization. Internal ad-hoc builds are explicit and cannot enter public draft uploads or the updater feed. Bundled USB code is signed in supported locations and its library loading is actually exercised.

## Software checks

155 native tests passed, zero failed, one existing live-account test intentionally ignored. Tests include real OS pseudo-terminal ownership errors, delayed firmware startup, retries, identity/wiring preservation, settings and diagnostics. The production frontend build passed. Browser fixtures passed 49 scenarios: [32 customer cases](customer-ui.json), [10 droid-switcher cases](switcher-ui.json), and [7 connection regression cases](connection-ui.json). Release and signing policy tests passed 17 cases.

## Installed Mac and physical Nimbus

The local app was installed with an unchanged settings file. The obsolete 0.4.1 installation was backed up and moved to Trash; 19 previous crash reports belonged to that old copy, whose settings format predates the current saved configuration. The repaired 0.10.1 app automatically recognized a physical Nimbus running factory firmware 1.3.2. Fresh USB diagnostic replies and reported screen-ready status were observed. No firmware, identity or saved standalone screens were rewritten.

A real Cmd+W → Finder Open check reproduced the hidden-controls problem before the fix, then passed after it. The same process remained connected and the controls returned. Final app and DMG signatures, contained files and bundled helper execution were checked. See [installed acceptance](mac-acceptance.json).

Private Workshop 0.3.2 was independently installed and tested with the same Nimbus. It showed “Nimbus already programmed”, selected its existing model and wiring, offered a reset review that was cancelled without erasing, retained “Last checked” details when the customer app took USB, and refreshed them after its explicit quit action. Workshop remains private.

These checks establish detection and the tested handoff on this Mac, not uninterrupted long-term USB stability, physical visibility of every rendered screen, or four-board hardware acceptance. No real Apple signing/notarization could be performed without the owner's Developer Program credentials. Internal ad-hoc signatures do not resolve Gatekeeper warnings for website downloads.
