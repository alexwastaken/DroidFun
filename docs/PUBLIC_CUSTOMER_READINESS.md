# Public customer readiness

Assessment date: 2026-10-29. This records source review, local checks and hosted-runner verification for the 0.10.6 internal candidate. It does not certify a published release or physical-device acceptance on every operating system.

## Fresh customer setup

A factory-prepared Nimbus, Boo, Meep or Claw reports its model over USB. Recognition and the first screen require no Workshop installation, owner credentials, customer account or internet connection. Nimbus, Boo and Meep start with a face; Claw starts with clearly labeled sample usage. Saving app preferences before connecting the first droid must preserve this starter behavior after restarting.

The customer app stores its own local profiles and preferences. Optional live Codex and Claude allowance screens use the customer's own supported software and sign-in. Missing software or credentials produce an explanation without blocking USB controls or sample screens. “Set up live usage” opens a draft; Apply saves it, and account connection requires an explicit action.

An older board without firmware identity may use a remembered model, a bounded public model hint, or the customer's one-time model choice. The public hint is not inventory access or proof of ownership. See [Droid identification and private inventory](DROID_REGISTRY.md).

## Customer security boundary

- The local main window receives an explicit customer command allowlist and event-listening permissions. There are no Workshop, owner, inventory-list or database-query commands, and remote pages receive no native permissions.
- Production controls stay on the app's bundled origin. CSP excludes development servers, remote scripts, frames and embedded objects. Reviewed public help links open in the system browser; arbitrary navigation and new privileged windows are denied.
- Native commands remain privileged operations and require their own input validation. The allowlist is not a claim that renderer compromise is harmless. Source guards check command/permission parity and configuration; they do not replace runtime IPC testing or package inspection.
- Customer distribution must exclude owner sessions, Workshop pairing tokens, local customer settings and account credentials. The registry service, rather than hidden UI controls, enforces owner authorization.
- Models and fallback posters load locally. The build includes firmware and platform helper resources for supported recovery operations. Every produced installer still needs its package-content and helper-runtime checks; source configuration alone does not prove a complete installer.

## Checks recorded locally

| Check | Recorded result and scope |
| --- | --- |
| Fresh customer UI | 21 browser scenarios passed at 360/500 px: empty profiles, no USB, all four factory models online/offline, update network failure, absent account software, explicit Apply and cancellation. Native replies and account failures were simulated. |
| Bundled visuals | All four models retained usable controls and local posters with WebGL disabled. No external presentation asset requests, unhandled page errors or horizontal overflow occurred in the fixtures. |
| Native regression suite | 177 passed, one existing live-account test ignored. Includes fresh factory recognition with both wiring variants, independent customer profiles, legacy offline selection, and saving preferences before first connection. |
| Release and boundary guards | 23 tests passed, covering release evidence and source-level customer permission/CSP/registry restrictions. |
| Frontend build | TypeScript and production frontend build passed. |
| Installed Apple Silicon app | Internal 0.10.6 passed bundle, helper and read-only DMG checks. The installed app rendered its local 3D model, received state updates, opened/canceled Settings, and opened help in the system browser while retaining its local controls. All saved settings remained identical. This was an existing user profile, not a fresh Mac account. |

The UI fixture report identifies its base version as 0.10.5; it exercised the candidate's two UI changes before the 0.10.6 version bump. Fixtures used synthetic IDs and no real accounts or hardware. These results do not demonstrate another customer's clean-machine installation.

## Native installer verification

Application and pipeline source: `a23ef5635687481142e496b726fcba3472ec1555`.
The [Windows/Linux run](https://github.com/alexwastaken/DroidFun/actions/runs/40972606529)
passed on Windows x64, Windows ARM64 and Ubuntu 24.04 x86_64. Each package was
installed and launched on its native runner. Linux also passed the optimized
GLib backport regression and installed-app IPC, external-browser and scoped USB
permission checks. These hosted runners had no physical droids or owner accounts.

The [universal Mac run](https://github.com/alexwastaken/DroidFun/actions/runs/40973815077)
passed on `68795c116a0e845c2ad3654e66582088ebbf2db8`; only Mac packaging code/tests
changed after the verified Windows/Linux source. Both CPU architectures, ad-hoc
code signatures, helper execution, and matching DMG/ZIP contents were verified.
The staging step supplies the helper under every target filename required by
Tauri. These packages remain internal and are not Developer ID signed or notarized.

| Native target | Behavior tests | Additional verification |
| --- | --- | --- |
| Windows x64 | 169 passed, one ignored | Fresh installer, matching payload/CPU, native window, updater signature |
| Windows ARM64 | 169 passed, one ignored | Fresh installer, matching payload/CPU, native window, updater signature |
| Linux x86_64 | 170 passed, one ignored | Two optimized GLib regressions; fresh DEB install, actual IPC/browser restrictions, scoped USB rules, removal/reinstall preservation; RPM architecture metadata |
| macOS universal | 177 passed, one ignored | Intel/Apple Silicon build, helper runtime and internal DMG/ZIP verification; local Apple Silicon acceptance is recorded separately |

[Compact verification record and package hashes](verification/0.10.6/public-customer.json)
preserve the exact commits, runs, checks and limitations. No installers were
uploaded to a release and the website/updater still point to v0.10.0.

## Remaining release gates

- macOS Developer ID signing and notarization remain blocked on owner enrollment and credentials. An internal ad-hoc build does not remove Apple's trust warning. See [Mac signing](MAC_SIGNING.md).
- Windows installers currently lack a publisher code-signing certificate. Updater signatures do not provide Windows publisher trust or guarantee that SmartScreen will accept a fresh download.
- Hosted Windows/Linux installation and universal Mac packaging checks passed as recorded above. Fresh-machine hardware, human trust prompts, update installation, and Intel Mac runtime still require release acceptance.
- Real four-droid operation, prolonged reconnect behavior, and clean-machine hardware acceptance across supported operating systems are not verified by this audit. Earlier single-board checks are not a substitute.
- Publishing installers, the website catalog and the updater feed requires separate release evidence. This document does not authorize or claim publication.

## Dependency advisory disposition

The lockfile audit prompted compatible updates to Tokio 1.53.3, rustls 0.23.46,
rustls-webpki 0.103.16 and webpki-roots 1.0.10. The remaining findings require
source and platform review; a nonzero version-only advisory count is not by
itself evidence of an unfixed runtime vulnerability or a clean bill of health.

| Finding | Disposition and remaining limit |
| --- | --- |
| `glib` 0.18.5 — [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) / GHSA-wrw7-89jp-8q8g | Affected Linux GTK dependency. Backported the exact two-line upstream fix for the mutable C output pointer in `VariantStrIter`. The published version number remains 0.18.5, so version-only tools may still flag it. [Provenance, archive checksum and regression command](../vendor/glib/DROIDFUN_BACKPORT.md) document the source fix. The optimized Linux regression must pass on the exact release commit before accepting Linux packages. The supported macOS and Windows graphs do not compile this GTK dependency. |
| `block-buffer` 0.10.4 — GHSA-qwgh-2vcv-g2f7 | The aggregate advisory range starts at zero, but the [upstream advisory](https://github.com/RustCrypto/utils/security/advisories/GHSA-qwgh-2vcv-g2f7) explicitly affects `>=0.11.0, <0.12.1`. Reviewed 0.10.4 stores its cursor separately, rather than in the buffer bytes implicated by the bug. Treat this specific version/range finding as not affected; do not suppress future versions or force an unrelated breaking digest upgrade. |
| `proc-macro-error` 1.0.4 — [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html) | Informational unmaintained dependency, with no patched release. Brought in at build time by `glib-macros` and `gtk3-macros` for the Linux GTK stack. This advisory does not identify a runtime exploit. Maintenance risk remains until the GTK dependency chain migrates or receives a separately reviewed replacement. |
| `zvariant` 5.15.0, `zvariant_derive` 5.15.0 and `zbus_names` 4.3.4 — [RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370.html), [0372](https://rustsec.org/advisories/RUSTSEC-2026-0372.html), [0371](https://rustsec.org/advisories/RUSTSEC-2026-0371.html) | Informational maintenance notices: these crates were [merged into zbus 6](https://github.com/z-galaxy/zbus/issues/2043). They enter the Linux build through `tauri-plugin-single-instance` → zbus 5. The final old-line patch releases document the migration but do not resolve the unmaintained status. Even plugin 2.5.5 still requires zbus 5, so removing these findings requires a tested upstream/plugin migration. Retain single-instance protection while tracking this dependency maintenance risk. |

GLib's fixed upstream line starts at 0.20, but the currently used Tauri,
WebKitGTK and GTK3 dependencies require 0.18. Adding 0.20 alongside it would not
repair the older copy. The small vendored backport is temporary and must be
removed when the complete dependency chain supports a fixed maintained release.
Neither the backport nor these scoped dispositions replace release-specific CI,
package review or future advisory checks.
