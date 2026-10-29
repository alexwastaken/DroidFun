# Desktop app update releases

The desktop updater reads `https://raw.githubusercontent.com/alexwastaken/DroidFun/main/distribution/updater.json`. The feed uses explicit versioned GitHub release URLs and supports published prereleases. It never resolves GitHub's `latest` endpoint.

Supported automatic update packages:

| Feed target | Release asset |
| --- | --- |
| `darwin-aarch64`, `darwin-x86_64` | `DroidFun-App-VERSION-macOS-universal.app.tar.gz` |
| `windows-x86_64` | `DroidFun-App-VERSION-Windows-x64-setup.exe` |
| `windows-aarch64` | `DroidFun-App-VERSION-Windows-ARM64-setup.exe` |

Each updater asset has an adjacent `.sig` release asset. The Mac DMG/ZIP and Linux DEB/RPM downloads remain available; DEB/RPM installation is manual.

## Signing and builds

The application configuration contains the updater public key. Approved release credentials provide `TAURI_SIGNING_PRIVATE_KEY` as a GitHub Actions secret; its password is empty. The private key is injected only into the native packaging step on Mac/Windows. It is never put in source, build arguments, artifacts, evidence, or the feed. Preserve the same signing key for later updates.

`npm run release -- --target universal-apple-darwin --mac-signing developer-id --updater` creates the Mac updater archive after Apple signing, notarization, stapling and an extracted-bundle trust check. Tauri's signer then signs the verified archive and its exact version. Windows uses its native target plus `--updater`. These commands fail before building if required signing credentials or the configured public key are missing. Private Mac packaging requires explicit `--mac-signing internal-adhoc` and cannot produce public updater assets. Linux packaging also disables updater assets. See [Mac signing setup and verification](MAC_SIGNING.md).

Tauri CLI 2.12.2 signs the app version in the signature's authenticated trusted comment. The updater should require that signed version. `scripts/updater-assets.cjs` independently checks the Minisign Ed25519 signature over BLAKE2b-512 of the asset, the trusted-comment signature, the key ID, original package filename, and exact app version. Package evidence records those results and SHA-256 for the updater asset and signature. Draft upload tooling rechecks them before uploading. Platform code signing/notarization remains separate from the updater signature.

`node --test scripts/updater-release.test.cjs` exercises signature and metadata tampering, mismatched source/asset hashes, draft rejection, platform completeness, prerelease support, and local builds without credentials. Tests use public RFC/upstream cryptographic test vectors; they do not generate or read a production key.

## Publishing a feed

1. Build the exact application commit on Mac, Windows x64, Windows ARM64, and Linux using the manual workflows. Existing draft-upload mode may collect the installers, updater packages, signatures, and verification records on the matching draft release.
2. Complete platform checks and publish that exact release with its final assets. Keep `target_commitish` set to the full application commit SHA.
3. Put the Mac and both Windows updater files, `.sig` files, and verification JSON records together in one local directory. Run:

   ```sh
   node scripts/generate-updater-manifest.cjs \
     --version 0.4.2 \
     --source FULL_APPLICATION_COMMIT_SHA \
     --packages /absolute/path/to/verified-packages \
     --output distribution/updater.json \
     --notes /absolute/path/to/update-notes.txt
   ```

   `--notes` is optional. The generator uses a read-only GitHub API request through `gh`. It requires the exact release to be published, all three updater package records to match passing tests/native verification and the source commit, and GitHub's published asset names, byte sizes, SHA-256 digests, and download URLs to match the locally reverified files. Missing platforms, draft releases, duplicate assets, changed signatures, and unsigned version metadata fail closed. The two Mac targets share the verified universal archive. Linux targets are omitted.
4. Review and publish the generated `distribution/updater.json` on `main`. The generator only writes the local file; it never commits, pushes, publishes, or dispatches workflows. Update the feed only after the referenced assets are publicly downloadable.

The first updater-enabled app must be installed normally. Later releases can then be offered in that app. Verify an actual signed newer-version installation, the restarted version, settings preservation and normal USB behavior separately from unit tests and artifact checks. A completed desktop upgrade alone does not establish physical bot acceptance.

## Recorded 0.4.2 acceptance

The [public 0.4.2 release](https://github.com/alexwastaken/DroidFun/releases/tag/v0.4.2) was built from application commit `701a261bc26a66705b016cc85473a1cabf71b25a`. [Mac CI](https://github.com/alexwastaken/DroidFun/actions/runs/40215307155) passed 97 native tests; [Windows x64, Windows ARM64 and Linux x86_64 CI](https://github.com/alexwastaken/DroidFun/actions/runs/40215309478) passed 91 on each target. Each recorded zero failures and one intentionally ignored live-account test. Frontend and updater release-tooling checks passed, along with universal Mac package checks and Windows/Linux installed-package smoke checks. Mac and both Windows updater signatures were cryptographically verified for the exact 0.4.2 assets before upload.

On October 25, 2026, a writable, user-owned Mac test copy at **0.4.2-rc.1** completed Settings → App → **Check → Download → Restart** against the public 0.4.2 feed. It installed the downloaded update and restarted automatically. Verification after restart confirmed:

- The installed executable matched the published universal Mac executable.
- Strict ad-hoc code-signature verification passed.
- The saved settings file's SHA-256 was exactly unchanged.
- A subsequent in-app check reported that the app was up to date.

The physical board was absent during this upgrade check, so successful USB operation after the desktop upgrade remains unverified. A separate 90-second passive observation with the app closed and no DroidFun process recorded 46 valid samples, five USB registry IDs and the board absent after 24 seconds. The observed disappearance does not require an active app; its cause remains unresolved. Earlier firmware 1.2.2 installation and a readable screen were confirmed on this Meep, as recorded in the [USB acceptance history](USB_SETUP.md).

This establishes the Mac desktop upgrade and settings-preservation flow. It does not establish a Windows in-app upgrade, sustained physical USB stability, saved-playlist wall-power behavior, or Intel Mac/macOS 13/Fedora runtime acceptance. Updater signatures are separate from platform trust: the Mac app remains ad-hoc signed without Developer ID notarization, and Windows installers lack platform code signing. Consumers still need hardware that has passed readable-screen and power-cycle acceptance.

For a protected Mac installation, the pinned updater uses a graphical administrator authorization prompt after a permission-denied replacement; canceling or failing authorization returns an error and DroidFun does not restart. That path was reviewed in the updater source, but the recorded upgrade used a writable test bundle and did not exercise administrator authorization.

## Recorded 0.6.0 acceptance

The [public 0.6.0 release](https://github.com/alexwastaken/DroidFun/releases/tag/v0.6.0) was built from `a5593ec9e68696e9e43dac1cb51c3c85361ed8f0`. [Mac CI](https://github.com/alexwastaken/DroidFun/actions/runs/40626592307) passed 127 native tests; [Windows/Linux CI](https://github.com/alexwastaken/DroidFun/actions/runs/40626594859) passed 121 per target. Package hashes and Mac/Windows updater signatures were independently verified for the exact version and bytes. Windows/Linux installed-package smoke checks passed. The 18 customer UI browser fixtures included update progress surviving closed Settings, settings applied before restart when edited, and offline mode blocking update checks.

On October 27, 2026, an installed, user-owned Mac app completed a real **0.5.5 → 0.6.0** upgrade using its in-app **Check → Download → Restart** controls against the published feed. The updated process restarted automatically before any manual launch. A follow-up check used Settings → App in 0.6.0. The [acceptance record](verification/0.6.0/mac-app-upgrade.json) confirms:

- The installed executable matched the verified universal archive: SHA-256 `00959c662e62c07b159a0f98e64807feb1cf4d4f9645072ca6d11a891ef96252`.
- Strict ad-hoc code-signature verification passed.
- The saved settings file was unchanged: SHA-256 `3452f1ad4ba8801388f97a510ff3e12d0f032a49c3e3aee2e3af456307562c05` before and after.
- A follow-up manual update check reported the app up to date.

This confirms the Mac desktop update and settings-preservation flow. No physical droid was available, so USB reconnection, real-device model recognition, screen visibility, power cycling, wall-power saved screens and sleep/wake behavior were not verified by this upgrade. Windows in-app upgrade acceptance remains separate from successful installer smoke checks and updater-signature verification. Mac packages remain ad-hoc signed without Developer ID notarization, and Windows installers lack platform code signing.

## Recorded 0.7.0 acceptance

The [public 0.7.0 prerelease](https://github.com/alexwastaken/DroidFun/releases/tag/v0.7.0) was built from `b6d347d1f977db709ff4379af3294b03f912e5d4`. [Mac CI](https://github.com/alexwastaken/DroidFun/actions/runs/40731707194) passed 131 native tests; [Windows/Linux CI](https://github.com/alexwastaken/DroidFun/actions/runs/40732069700) passed 125 per target. Package hashes, Mac/Windows updater signatures and native installer smoke checks passed.

On October 28, 2026, the installed Mac app completed **0.6.0 → 0.7.0** through Settings → App → Check → Download → Restart using the public feed. The process restarted automatically before any manual launch. The [acceptance record](verification/0.7.0/mac-app-upgrade.json) confirms the installed executable matched the published universal package (SHA-256 `61734b83deefdb3cbda2fa7bb701ec9d126746a57e7cc32555fec388539fe261`), strict ad-hoc signature verification passed, and saved settings were unchanged. A subsequent update check reported the app up to date, and the installed app rendered its bundled 3D preview.

No physical board was attached, so this verifies the Mac desktop upgrade rather than USB, display, power-cycle or sleep/wake acceptance. A Windows in-app upgrade remains separate from package smoke and signature checks. Mac Developer ID notarization and Windows publisher signing remain open.

## Recorded 0.7.1 acceptance

The installed Mac app completed 0.7.0 → 0.7.1 through Settings → App → Check → Download → Restart using the public feed. Automatic restart was observed before any manual launch. The installed executable matched the verified universal package, strict ad-hoc signing passed and saved settings were unchanged. The corrected screen proportions and enlarge/close control were verified in the native app. A follow-up manual check reported the app up to date. See the [acceptance record](verification/0.7.1/mac-app-upgrade.json) and [release evidence](verification/0.7.1/README.md). Firmware is unchanged; no physical USB acceptance is claimed. Existing platform-signing and Windows in-app upgrade limits remain.

## 0.8.0 custom screens acceptance

The [0.8.0 records](verification/0.8.0/README.md) cover all four platform packages, exact-source hashes and signed updater checks. The installed Mac 0.7.1 app fetched the public feed, downloaded 0.8.0, restarted automatically and matched the published executable with a strict code-signature check. Existing settings remained byte for byte unchanged. Native Add a screen offered Custom text and Upload an image, and a fresh update check reported up to date. Schema-3 migration and the private original-settings archive have separate native tests.

## 0.9.0 diagnostics acceptance

The [0.9.0 evidence](verification/0.9.0/README.md) records exact-source packages and signed updater checks on all platforms. The installed Mac 0.8.0 app updated through the public feed, restarted automatically and matched the published 0.9.0 executable with strict signature verification. Settings were unchanged byte for byte. A real no-device diagnostic ran for six seconds, correctly withheld physical-test/reinstall actions, saved the reviewed JSON through the native file dialog with owner-only permissions, and opened the fixed public support page in Chrome. A fresh update check reported up to date. The private exported report and its droid ID are not included in public evidence.

## 0.10.0 droid-switching acceptance

The [0.10.0 evidence](verification/0.10.0/README.md) covers all four native platforms, exact-source packages and updater signatures. The installed Mac completed 0.9.0 → 0.10.0 through the public feed, restarted automatically before any manual launch, matched the verified universal executable and passed strict code-signature validation. Saved settings were unchanged byte for byte. The native home screen showed the top droid menu, 0/4 attached, and disabled arrows with no boards connected. A fresh manual update check reported up to date. The four-droid behavior has native slot/profile tests and simulated browser checks; physical multi-USB acceptance remains open.
