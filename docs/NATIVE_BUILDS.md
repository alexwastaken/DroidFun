# Windows and Linux builds

The **Build Windows and Linux packages** workflow builds Windows x64, Windows ARM64 and Linux x86_64 on their native operating systems. It runs only through **Run workflow** in GitHub Actions. Pushing source, opening a pull request or editing a release does not start it.

## Reproduce a release build

1. Open [the manual build workflow](https://github.com/alexwastaken/DroidFun/actions/workflows/build-desktop.yml).
2. Choose the pipeline branch, normally `main`.
3. Set `source_ref` to the exact application commit or release tag. Set `release_tag` to the matching release version, for example `v0.3.1`. Choose `all` or one platform to retry.
4. Run the workflow. Download each successful job's artifact. Failed tests, installer failures, wrong binary architecture or a missing native window prevent upload of a successful package artifact.
5. Review the verification JSON and checksums before attaching the installer packages to the release. The workflow does not publish releases or need write access to repository contents.

The pipeline is checked out separately from the application. This lets the original tagged application source use newer build tooling without moving a release tag. Each verification record identifies both commits and the Actions run.

`source_ref` defaults to `main`, which includes the Windows mock-server test fix. For an exact rebuild, use the application commit from that platform's verification record. The version check rejects source that does not match `release_tag`.

## Outputs

| Target | Runner | Package |
| --- | --- | --- |
| Windows x64 | `windows-2025` | `DroidFun-App-VERSION-Windows-x64-setup.exe` |
| Windows ARM64 | `windows-11-arm` | `DroidFun-App-VERSION-Windows-ARM64-setup.exe` |
| Linux x86_64 | `ubuntu-24.04` | `DroidFun-App-VERSION-Linux-x86_64.deb`, `.rpm` |

The Windows installers use the current-user NSIS installation mode. The Linux packages require GTK 3, WebKitGTK 4.1, AppIndicator and libudev. Use a current distribution with those libraries; RPM generation alone does not establish compatibility with every RPM distribution. From 0.3.4, the DEB/RPM installs a scoped robot access rule automatically; Connection → Set up robot access repairs it through the desktop authorization dialog. Standard active local logind sessions are supported; see [Linux USB setup](LINUX_USB_ACCESS.md). A desktop with a StatusNotifier/AppIndicator host can display the tray icon; the app keeps its regular window when no host is detected.

## Checks and limits

- Locked frontend dependencies, TypeScript check and production frontend build.
- Rust 1.99.0, locked Cargo dependencies and native Rust behavior tests on each target.
- Actual application executable checked for x64/ARM64 PE or x86_64 ELF architecture.
- Windows: silent installer succeeds, installed executable matches the extracted NSIS payload and has the requested PE architecture, process remains alive for 15 seconds and creates a native window. The installer payload includes Tauri's bundle metadata; the original build executable is restored after bundling.
- Linux: DEB installation succeeds on Ubuntu 24.04, installed executable matches the DEB payload, app stays alive for 15 seconds and creates a window under Xvfb/D-Bus without a tray host. RPM architecture metadata is checked. Tauri embeds bundle-type metadata, then restores the original build executable after each package.
- SHA-256 hashes and byte sizes recorded for every installer package.

These checks do not establish physical USB behavior, human tray interaction, live account authentication, upgrade/uninstall behavior or a 24-hour soak on the new platforms. Windows packages are unsigned development builds. Existing Mac runtime/hardware evidence is separate in [0.2.0.json](verification/0.2.0.json).

The executable comparison uses each installer's payload because [the pinned Tauri bundler patches package metadata and restores the original binary](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.2/crates/tauri-bundler/src/bundle.rs). Comparing an installed executable with the restored build would incorrectly reject a successful installation.

## Build references

The native targets follow [Tauri's distribution pipeline guidance](https://v2.tauri.app/distribute/pipelines/github/). Linux development packages follow [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/). Native Windows ARM64 is supported by [GitHub's hosted runner labels](https://docs.github.com/en/actions/reference/runners/github-hosted-runners); private repositories use the owner's Actions minutes. Installer behavior is described in [Tauri's Windows installer documentation](https://v2.tauri.app/distribute/windows-installer/).

## Version 0.2.0 build records

| Platform | Application source | Build and checks |
| --- | --- | --- |
| Linux-x86_64 | `02af44fd1b36c5317de82ca95c6b48c574b99a44` | [Native run](https://github.com/greensky5656/DroidFun/actions/runs/38219369412), [verification](verification/native-0.2.0/verification-0.2.0-Linux-x86_64.json) |
| Windows-ARM64 | `dc4b0c24abbf6702f863ebfc7c6846fd056d3746` | [Native run](https://github.com/greensky5656/DroidFun/actions/runs/38220668748), [verification](verification/native-0.2.0/verification-0.2.0-Windows-ARM64.json) |
| Windows-x64 | `02af44fd1b36c5317de82ca95c6b48c574b99a44` | [Native run](https://github.com/greensky5656/DroidFun/actions/runs/38219584289), [verification](verification/native-0.2.0/verification-0.2.0-Windows-x64.json) |

The Windows ARM64 source includes a correction confined to `#[cfg(test)]`: the mock HTTP server explicitly uses a blocking accepted socket and waits for complete request headers. The original Windows test could fail with Winsock error 10035 before request bytes arrived. Production Rust, frontend and firmware source remain unchanged. [Microsoft documents accepted sockets inheriting listener properties](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-accept). All 27 behavior checks remain required.

## Version 0.3.0 build records

All three targets use application and pipeline commit `f506d20bf9b8f2fa58fb051b3b59570a1fb015f8`. [Native workflow](https://github.com/greensky5656/DroidFun/actions/runs/38311819047) completed successfully.

| Platform | Behavior tests | Build and installation evidence |
| --- | --- | --- |
| Windows x64 | 33 passed | [Verification](verification/native-0.3.0/verification-0.3.0-Windows-x64.json), matching installed payload/architecture, native window alive 15 seconds |
| Windows ARM64 | 33 passed | [Verification](verification/native-0.3.0/verification-0.3.0-Windows-ARM64.json), matching installed payload/architecture, native window alive 15 seconds |
| Linux x86_64 | 34 passed | [Verification](verification/native-0.3.0/verification-0.3.0-Linux-x86_64.json), installed DEB payload matches, native window alive 15 seconds under Xvfb/D-Bus |

The credential-terminal test is Unix-only, explaining the test-count difference. [RPM payload inspection](verification/native-0.3.0/linux-package-inspection.json) independently confirms x86_64 ELF; Fedora runtime remains untested. [Mac packaging evidence](verification/native-0.3.0/verification-0.3.0-macOS.json) records the universal executable, ad-hoc signature and ZIP/DMG payload checks. The exact release Mac package was not launched during the ongoing reliability run. [All release upload digests](verification/native-0.3.0/release-upload-verification.json) match their local files.

These development packages include the five local widgets and richer animations. The continuous Mac/robot test remains on its unchanged internal executable and has not yet completed; publication is not 24-hour acceptance. Physical Windows/Linux robot use, real account sign-in, human tray interaction, upgrade/uninstall, signing, Fedora runtime and Intel Mac runtime remain open.

## Version 0.3.1 build records

[DroidFun App 0.3.1](https://github.com/greensky5656/DroidFun/releases/tag/v0.3.1) was rebuilt from application and pipeline commit `994f8993e647a7085444c8f7abda3966004e94e7`. [Native Windows/Linux run](https://github.com/greensky5656/DroidFun/actions/runs/38335346212) completed successfully. The Mac universal package was built locally from the same commit.

| Platform | Behavior tests | Build and package evidence |
| --- | --- | --- |
| Windows x64 | 33 passed | [Verification](verification/native-0.3.1/verification-0.3.1-Windows-x64.json), silent installation, payload/architecture match, native window alive 15 seconds |
| Windows ARM64 | 33 passed | [Verification](verification/native-0.3.1/verification-0.3.1-Windows-ARM64.json), silent installation, payload/architecture match, native window alive 15 seconds |
| Linux x86_64 | 34 passed | [Verification](verification/native-0.3.1/verification-0.3.1-Linux-x86_64.json), DEB installation and matching payload, native window alive 15 seconds on Ubuntu 24.04 under Xvfb/D-Bus |
| macOS universal | 34 passed | [Verification](verification/native-0.3.1/verification-0.3.1-macOS.json), Intel/Apple Silicon executable, ad-hoc signature, matching ZIP and mounted read-only DMG payloads |

The Mac bundle is signed before disk-image creation, so both downloads contain the verified app. [Independent RPM payload inspection](verification/native-0.3.1/linux-package-inspection.json) confirms ELF64 x86_64. All 15 release uploads match [GitHub's stored digests](verification/native-0.3.1/release-upload-verification.json). Frontend checks/build passed on every target. The test-count difference is the Unix-only credential-terminal test.

This development release introduces the DroidFun App name, blue theme and icons while preserving compatibility identifiers and settings locations. The exact branded Mac app was not launched during the ongoing robot soak; its launch and upgrade acceptance remain open. Windows installers are unsigned, and Mac packages lack Developer ID signing and notarization. Windows/Linux physical USB, account authentication, human tray interaction, full upgrade/uninstall, Fedora RPM runtime and Intel Mac runtime remain open. The separate Mac/robot run continues unchanged and is not yet a 24-hour pass. Follow [upgrade instructions](BRANDING.md) when replacing Deskbot.

## Version 0.3.2 build records

The approved branding selector and Claude fixes were built from application and pipeline commit `5943daccc8b80eaf6a3129cc6df374813f6a387c`. [Windows/Linux native run](https://github.com/greensky5656/DroidFun/actions/runs/38669361152) and [universal Mac run](https://github.com/greensky5656/DroidFun/actions/runs/38669363368) passed.

| Platform | Behavior tests | Package checks |
| --- | --- | --- |
| Linux-x86_64 | 40 passed | [Verification](verification/native-0.3.2/verification-0.3.2-Linux-x86_64.json), DEB installed/matching payload and native window alive 15 seconds under Xvfb/D-Bus; RPM metadata checked |
| Windows-ARM64 | 38 passed | [Verification](verification/native-0.3.2/verification-0.3.2-Windows-ARM64.json), native installer, matching payload/architecture and native window alive 15 seconds |
| Windows-x64 | 38 passed | [Verification](verification/native-0.3.2/verification-0.3.2-Windows-x64.json), native installer, matching payload/architecture and native window alive 15 seconds |
| macOS | 40 passed | [Verification](verification/native-0.3.2/verification-0.3.2-macOS.json), universal architecture, ad-hoc signature, identical ZIP/read-only DMG payloads; exact app installed/launched on Apple Silicon with saved settings and hub connection |

The two Unix-only credential-terminal/owned-renewal tests explain the platform count difference; the live account-shape diagnostic is intentionally ignored. All 15 release assets match [GitHub stored digests](verification/native-0.3.2/release-upload-verification.json). The installed Mac package showed both skins and a fresh live Claude card; this is not another natural-expiration event or resource soak. Prior natural renewal and 15-minute background results retain their recorded build/time boundaries. Firmware 1.1.0 is unchanged.

These are development packages: Mac lacks Developer ID signing/notarization and Windows installers are unsigned. Windows/Linux physical robot/live-account behavior and human tray interaction, Fedora RPM runtime, Intel Mac runtime, full installation/upgrade/uninstall lifecycle and complete uninterrupted long-term qualification remain open. Windows automatic credential renewal is not claimed.

## Version 0.3.3 build records

The Windows automatic Claude recovery update was built from application and pipeline commit `c6be4fbf70a2989fced3bded7c6f6e1123880194`. [Windows/Linux native run](https://github.com/greensky5656/DroidFun/actions/runs/38674557752) and [universal Mac run](https://github.com/greensky5656/DroidFun/actions/runs/38674559918) passed.

| Platform | Behavior tests | Package checks |
| --- | --- | --- |
| Linux-x86_64 | 40 passed | [Verification](verification/native-0.3.3/verification-0.3.3-Linux-x86_64.json), DEB installed/matching payload and native window alive 15 seconds under Xvfb/D-Bus; RPM metadata checked |
| Windows-ARM64 | 44 passed | [Verification](verification/native-0.3.3/verification-0.3.3-Windows-ARM64.json), native installer, matching payload/architecture and native window alive 15 seconds |
| Windows-x64 | 44 passed | [Verification](verification/native-0.3.3/verification-0.3.3-Windows-x64.json), native installer, matching payload/architecture and native window alive 15 seconds |
| macOS | 40 passed | [Verification](verification/native-0.3.3/verification-0.3.3-macOS.json), universal architecture, ad-hoc signature, identical ZIP/read-only DMG payloads; exact app installed/launched on Apple Silicon with saved settings and fresh Claude status; robot was absent before this update |

Six additional Windows-only ConPTY recovery tests and two Unix-only credential-terminal/owned-renewal tests explain the platform count difference; the live account-shape diagnostic is intentionally ignored. All 15 release assets match [GitHub stored digests](verification/native-0.3.3/release-upload-verification.json). The installed Mac package retained the branding dropdown and showed a fresh live Claude card; this is not another natural-expiration event or resource soak. Prior natural renewal and 15-minute background results retain their recorded build/time boundaries. Firmware 1.1.0 is unchanged.

These are development packages: Mac lacks Developer ID signing/notarization and Windows installers are unsigned. Windows/Linux physical robot/live-account behavior and human tray interaction, Fedora RPM runtime, Intel Mac runtime, full installation/upgrade/uninstall lifecycle and complete uninterrupted long-term qualification remain open. Windows automatic recovery is verified with isolated native fixtures; natural live Windows account expiration remains unverified.

## Version 0.3.4 — Linux USB installer and repair

[Published Linux release](https://github.com/alexwastaken/DroidFun/releases/tag/v0.3.4). Application/package source `bfe2f8b2caf5ef2f16fa3bcda49336387a6a9830`. Mac/Windows downloads remain the previously verified 0.3.3 packages; no older binary is relabeled as 0.3.4.

Built locally in an isolated **Ubuntu 24.04 ARM64 VM** with pinned Rust 1.99.0, a GNU x86_64 cross toolchain, locked dependencies and **Rosetta for Linux** to execute the x86_64 tests and installed app. This is not native x86_64 hardware acceptance. Initial hosted jobs were rejected before starting by GitHub's billing restriction; no further paid jobs were requested.

- **43 behavior tests passed**, zero failed, one intentionally ignored live-account diagnostic. [Receipt](verification/native-0.3.4/verification-0.3.4-Linux-x86_64.json).
- Actual DEB installed, executable matched its own payload, native window created and live for 15 seconds. Both blue and green were explicitly selected/checked in the installed app.
- Package installs/activates a scoped serial access rule without group changes. Real kernel denial/grant/other-user exclusion/seat-change revocation were verified with an ACL-capable character-node fixture and private synthetic seat data. USB parent/tty matching has positive and negative umockdev cases.
- Installed button → real IPC → pkexec → exact root-owned helper restored a missing rule. A temporary caller/action-specific test authorization enabled the headless approval path. The same grant did not authorize a different shell command; extra helper arguments were rejected. After removing it, denied approval displayed the expected error. No test authorization ships.
- Removal/reinstallation preserved the actual saved settings hash, a separate administrator rule and user data.
- RPM x86_64 metadata, runtime/file dependencies, fixed-interpreter install script and all payload file modes checked. Release tooling normalizes modes so a developer's umask cannot ship group-writable privileged files; app is 0755, script/policy/data 0644.
- All five uploaded files match [GitHub's stored SHA-256 digests](verification/native-0.3.4/release-upload-verification.json). Full final logs, screenshots and local build/verification procedures are in the release evidence ZIP.

[Ubuntu setup](LINUX_USB_ACCESS.md), [blue controls](verification/native-0.3.4/linux-usb-blue.png), [green controls](verification/native-0.3.4/linux-usb-green.png). Physical robot use on an Ubuntu laptop, a human graphical authorization interaction, native x86_64-host runtime, Fedora RPM runtime, Linux live accounts/tray behavior and complete long-duration qualification remain open. Packages are unsigned development builds. Existing Claude and firmware evidence keeps its original build/time boundary.
