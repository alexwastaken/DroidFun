# Claude authentication recovery — 0.10.9

Published November 7, 2026 (Pacific): [Windows and Linux release](https://github.com/alexwastaken/DroidFun/releases/tag/v0.10.9), built from application source `819a4162f3371f8b6f3af53a2b7fdb2962c25fae`. The four Windows/Linux installers are available on [droidfun.io/downloads](https://droidfun.io/downloads). The public Mac download remains 0.10.0.

## Change

The Unix renewal path previously required a scratch folder that had already been trusted in Claude Code, although ordinary sign-in did not establish that prerequisite. The old `--safe-mode` flag also caused older Claude Code clients to exit before refreshing credentials. Recovery now prepares a private scratch folder, starts the installed credential owner with compatible isolated settings, and requires renewed credentials before reporting success. It does not write trust approval, send keyboard input or request model inference. Authentication overrides are removed from the helper environment, while the selected Claude profile is preserved.

The account editor checks renewal before requesting another sign-in, refreshes status after each completed check at a bounded interval, and ignores responses for obsolete profiles. A revoked or otherwise unrefreshable login can still require manual sign-in. Allowance resets remain separate from authentication expiry. Saved droid settings and firmware are not migrated or flashed by this repair.

## Validation

| Scope | Result | Evidence |
| --- | --- | --- |
| Windows x64 | 176 native tests passed; installer, architecture, native startup and updater signature checks passed | [Native record](verification-0.10.9-Windows-x64.json) |
| Windows ARM64 | 176 native tests passed; installer, architecture, native startup and updater signature checks passed | [Native record](verification-0.10.9-Windows-ARM64.json) |
| Linux x86_64 | 176 native tests passed; DEB installation/native checks and RPM metadata checks passed | [Native record](verification-0.10.9-Linux-x86_64.json) |
| Local Mac ARM64 | 183 native tests passed; internal DMG and ZIP verified | [Source and validation](source-and-validation.json) |
| Frontend and publication tooling | Frontend build/checks, 25 release-tooling tests and 73 website tests passed | [Source and validation](source-and-validation.json) |

Each target recorded zero failing tests and one intentionally ignored live-account test. The three published platform jobs passed in [native workflow 42453653736](https://github.com/alexwastaken/DroidFun/actions/runs/42453653736). Recovery tests use isolated fixture credentials and local HTTP responses, including clean-start renewal, token rotation, bounded rejection, cancellation, owned-process cleanup and no-input checks.

## Publication

The [live download verification](live-download-verification.json) records HTTP 200 and matching byte sizes/SHA-256 hashes for all four updated installers. The Mac download, quick-start guide, legacy updater feed, store settings and products remained unchanged.

The independent Windows feed and distribution catalog were published on `main` at `f8988163d76fac1378af7115ceda79b52f18ca24` and checked through the GitHub API; see [feed publication](feed-publication.json). The [public raw-feed check](live-feed-verification.json) also matched the exact published feed and confirmed the legacy feed unchanged. [The Windows feed](../../../distribution/windows-updater.json) references the exact versioned release assets and their verified signatures. Windows 0.10.8 and later use that feed. Windows 0.10.0–0.10.7 needs one manual installer upgrade because those clients retain the legacy feed URL. Linux installation remains manual.

## Limits

The local Mac ARM64 DMG and ZIP are internal packages only. They were not published or installed; Apple Developer ID signing and notarization credentials remain unavailable. No private Mac package is included in this evidence directory or advertised through the public feed.

No new natural live-account expiry/recovery event or physical droid test was observed. Existing historical observations are not relabeled as 0.10.9 acceptance. Native installer smoke checks and updater signatures do not establish a real Windows in-app upgrade or a user's settings-preservation result. Windows publisher signing remains unavailable. No live Claude credentials were inspected and no model inference was requested during this validation.
