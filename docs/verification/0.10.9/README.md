# Claude authentication recovery — 0.10.9

Published November 7, 2026 (Pacific): [Mac, Windows and Linux release](https://github.com/alexwastaken/DroidFun/releases/tag/v0.10.9), built from application source `819a4162f3371f8b6f3af53a2b7fdb2962c25fae`. All five installers are available on [droidfun.io/downloads](https://droidfun.io/downloads). The universal Mac package uses packaging tools pinned at `6735f72c5c154f2fb7f2d94dd0dbfe7cc4f89fb7`; the application source and configuration are unchanged. The owner explicitly authorized publication without Developer ID signing or Apple notarization.

## Change

The Unix renewal path previously required a scratch folder that had already been trusted in Claude Code, although ordinary sign-in did not establish that prerequisite. The old `--safe-mode` flag also caused older Claude Code clients to exit before refreshing credentials. Recovery now prepares a private scratch folder, starts the installed credential owner with compatible isolated settings, and requires renewed credentials before reporting success. It does not write trust approval, send keyboard input or request model inference. Authentication overrides are removed from the helper environment, while the selected Claude profile is preserved.

The account editor checks renewal before requesting another sign-in, refreshes status after each completed check at a bounded interval, and ignores responses for obsolete profiles. A revoked or otherwise unrefreshable login can still require manual sign-in. Allowance resets remain separate from authentication expiry. Saved droid settings and firmware are not migrated or flashed by this repair.

## Validation

| Scope | Result | Evidence |
| --- | --- | --- |
| Windows x64 | 176 native tests passed; installer, architecture, native startup and updater signature checks passed | [Native record](verification-0.10.9-Windows-x64.json) |
| Windows ARM64 | 176 native tests passed; installer, architecture, native startup and updater signature checks passed | [Native record](verification-0.10.9-Windows-ARM64.json) |
| Linux x86_64 | 176 native tests passed; DEB installation/native checks and RPM metadata checks passed | [Native record](verification-0.10.9-Linux-x86_64.json) |
| Universal Mac | 183 native tests passed; strict ad-hoc app/helper/library signatures, universal architectures, DMG/ZIP/updater contents and updater signature verified | [Native record](macos/verification-0.10.9-macOS.json) |
| Frontend and publication tooling | Frontend build/checks, 29 release-tooling/security tests and 73 website tests passed | [Source and validation](source-and-validation.json) |

Each target recorded zero failing tests and one intentionally ignored live-account test. Windows/Linux jobs passed in [native workflow 42453653736](https://github.com/alexwastaken/DroidFun/actions/runs/42453653736); Mac passed in [workflow 42455552936](https://github.com/alexwastaken/DroidFun/actions/runs/42455552936). Recovery tests use isolated fixture credentials and local HTTP responses, including clean-start renewal, token rotation, bounded rejection, cancellation, owned-process cleanup and no-input checks.

## Publication

The initial [Windows/Linux download verification](live-download-verification.json) records HTTP 200 and matching byte sizes/SHA-256 hashes for those four installers. Subsequent [Mac download verification](macos/live-download-verification.json) confirms the exact 23,475,687-byte universal DMG from the public website, while preserving the other downloads, store settings and products. The page displays version 0.10.9 and the lack of Apple notarization beside the Mac download button. The Worker deployment is `0669b6f4-16bc-4e04-aa93-1d4de536b680`.

The shared updater feed and catalog were published on `main` at `25cab84c4a0f2d077c25a858ac9a942913454fa3`; [publication evidence](macos/feed-publication.json) verifies exact public raw bytes and preservation of all unrelated Git files. It serves authenticated Mac ARM64/Intel and Windows x64/ARM64 0.10.9 packages. Earlier Windows clients can now upgrade through that shared feed; Windows 0.10.8 and later retain their independent channel. Linux installation remains manual.

A subsequent [notes-only change](macos/windows-notes-publication.json) at `b12032ce8738ab791ec617cbff52c31778c2326c` corrects the independent Windows feed's former Mac-pending description. Windows version, package URLs, signatures and release timestamp remain unchanged. The [final feed check](macos/final-feed-verification.json) verifies all exact immutable feed bytes and the live shared feed/catalog; the Windows main-branch CDN still served its earlier release-note text at that check, with the correct 0.10.9 version and payload signatures. All pre-existing Windows/Linux GitHub assets were preserved when six verified Mac files were added; see [upload evidence](macos/github-upload-verification.json).

## Limits

The public Mac package is expressly approved ad-hoc distribution, not Apple-trusted distribution: Developer ID signing, notarization, stapling and Gatekeeper acceptance are all false in its evidence. No OS security preferences or quarantine attributes were changed. The earlier ARM64-only internal package was not published. Local verification repeated all three bundled binaries’ architectures and strict signatures and ran only the USB helper version command; a complete Mac app launch, real in-app upgrade, Intel runtime and hardware connection remain unverified. The owner’s installed 0.10.7 app was not replaced.

No new natural live-account expiry/recovery event or physical droid test was observed. Existing historical observations are not relabeled as 0.10.9 acceptance. Native installer smoke checks and updater signatures do not establish a real Windows in-app upgrade or a user's settings-preservation result. Windows publisher signing remains unavailable. No live Claude credentials were inspected and no model inference was requested during this validation.
