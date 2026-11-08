# Mac signing and first-install trust

The Mac pipeline defaults to Developer ID signing and notarization. The owner explicitly authorized a public, unnotarized 0.10.9 release on November 7, 2026. That release uses the separate `public-adhoc` mode below. Ad-hoc signatures and signed updater packages verify integrity but do **not** provide Apple Developer ID trust or a notarization ticket; macOS can block a fresh website download.

## Owner setup

The Apple-trusted distribution path requires an active Apple Developer Program membership and a **Developer ID Application** certificate. Create that certificate in the owner's Apple account, then store the following in the repository's GitHub Actions secrets. Never paste credentials into chat, commit them, or include them in build artifacts.

| Secret | Purpose |
| --- | --- |
| `APPLE_SIGNING_IDENTITY` | Full `Developer ID Application: … (TEAMID)` identity |
| `APPLE_TEAM_ID` | The matching ten-character Apple team ID |
| `APPLE_CERTIFICATE` | Base64 export of the Developer ID certificate and private key as a password-protected `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | Password for that export |
| `APPLE_ID` | Apple account used for notarization |
| `APPLE_PASSWORD` | An **app-specific password**, never the account's normal password |

The existing `TAURI_SIGNING_PRIVATE_KEY` is a separate updater credential. It cannot replace Apple signing. Tauri imports the certificate into a temporary CI keychain and performs its supported signing and notarization workflow. Local builds may instead use an installed Developer ID identity. Local notarization also supports `APPLE_API_KEY`, `APPLE_API_ISSUER` and `APPLE_API_KEY_PATH` as an alternative to the Apple ID pair; do not set both methods.

## Apple-trusted public release

Run **Build DroidFun App for Mac** with `signing_mode: developer-id` (the default), or run:

```sh
npm run release -- --target universal-apple-darwin --mac-signing developer-id --updater
```

Preflight rejects missing/incomplete credentials, an ad-hoc or development identity, and mismatched team IDs. There is no silent downgrade. The USB helper is staged in `Contents/MacOS`, with its library in `Contents/Frameworks`, so Tauri signs both before the application. Source archives and licenses stay in Resources. Staging only changes copied binaries; it does not alter their checked-in originals.

Tauri signs public builds with hardened runtime, submits the app for notarization, waits for acceptance, and staples the result. Verification then requires valid signatures on the app, helper and library; the expected Developer ID team and secure timestamps; hardened runtime on executables; a valid stapled ticket; and a successful Gatekeeper assessment. The helper must load its bundled library. Running its `version` command checks the actual loader without enumerating or opening any USB device.

The updater archive is created only from that verified app. It is unpacked and checked before Tauri's signer authenticates its bytes and app version. Final package collection checks the DMG's contained app, ZIP, and updater archive against the original file contents, modes and symlinks, then repeats signature, staple and Gatekeeper checks on each extracted app. This verifies the app inside each container; the DMG itself does not require a separate notarization submission. No stapling or bundle changes may occur after updater signing. The draft-upload and feed-generation scripts reject internal or incomplete Mac trust evidence.

Before publishing, test the actual website download in a normal browser on a Gatekeeper-enabled Mac and verify a real in-app upgrade. The ordinary first-run confirmation that an app came from the internet may still appear. Do not disable Gatekeeper or strip quarantine as a distribution fix.

## Explicitly approved publication without Apple credentials

The owner may choose `signing_mode: public-adhoc` and `allow_unnotarized: true` in the Mac workflow. Both are required. The equivalent local command is:

```sh
npm run release -- --target universal-apple-darwin --mac-signing public-adhoc --allow-unnotarized --updater
```

This mode removes Apple credentials from the build environment and requires the existing Tauri updater signing key. It verifies strict ad-hoc signatures on the app, USB helper and library, checks helper execution and both CPU architectures, and compares the DMG, ZIP and updater archive contents. The publication evidence requires an explicit acknowledgment and records Developer ID, notarization, stapling and Gatekeeper acceptance as false. The download page and release notes must disclose that the app is not Apple-notarized. It changes no OS security settings.

When rebuilding an already published application commit with newer packaging tools, the workflow overlays only an explicit packaging-script allowlist from its pinned workflow commit. Evidence records the application and packaging commits separately, and CI checks that runtime sources and configuration remain unchanged.

## Private testing without Apple credentials

```sh
npm run release -- --target universal-apple-darwin --mac-signing internal-adhoc
```

The workflow offers the same explicit `internal-adhoc` choice. Its collected filenames end in `-INTERNAL`; it creates no updater package, strips Apple/updater credentials from the build environment, and cannot upload to a public-release draft or enter the public updater feed. These private binaries omit hardened runtime because ad-hoc code has no Apple team identity for library validation; the public Developer ID path requires hardened runtime. This is a build setting and changes no Mac security preferences. Internal packages remain subject to macOS first-install trust warnings. Use the release wrapper for public packaging and its required trust checks. Direct `npm run tauri -- build` or `bundle` also stages the helper in the correct signed-code locations for development packages, while preserving explicit signing configuration. `npm run tauri dev` still uses the source-tree helper; the runtime retains the older Resources lookup for compatibility.

References: [Apple Developer ID](https://developer.apple.com/developer-id/), [Apple notarization requirements](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution), [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/).
