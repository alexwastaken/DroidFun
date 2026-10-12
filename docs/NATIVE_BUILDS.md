# Windows and Linux builds

The **Build Windows and Linux packages** workflow builds Windows x64, Windows ARM64 and Linux x86_64 on their native operating systems. It runs only through **Run workflow** in GitHub Actions. Pushing source, opening a pull request or editing a release does not start it.

## Reproduce a release build

1. Open [the manual build workflow](https://github.com/greensky5656/DroidFun/actions/workflows/build-desktop.yml).
2. Choose the pipeline branch, normally `main`.
3. Set `source_ref` to the exact application commit or release tag. Set `release_tag` to the matching release version, for example `v0.3.0`. Choose `all` or one platform to retry.
4. Run the workflow. Download each successful job's artifact. Failed tests, installer failures, wrong binary architecture or a missing native window prevent upload of a successful package artifact.
5. Review the verification JSON and checksums before attaching the installer packages to the release. The workflow does not publish releases or need write access to repository contents.

The pipeline is checked out separately from the application. This lets the original tagged application source use newer build tooling without moving a release tag. Each verification record identifies both commits and the Actions run.

`source_ref` defaults to `main`, which includes the Windows mock-server test fix. For an exact rebuild, use the application commit from that platform's verification record. The version check rejects source that does not match `release_tag`.

## Outputs

| Target | Runner | Package |
| --- | --- | --- |
| Windows x64 | `windows-2025` | `Deskbot-VERSION-Windows-x64-setup.exe` |
| Windows ARM64 | `windows-11-arm` | `Deskbot-VERSION-Windows-ARM64-setup.exe` |
| Linux x86_64 | `ubuntu-24.04` | `Deskbot-VERSION-Linux-x86_64.deb`, `.rpm` |

The Windows installers use the current-user NSIS installation mode. The Linux packages require GTK 3, WebKitGTK 4.1, AppIndicator and libudev. Use a current distribution with those libraries; RPM generation alone does not establish compatibility with every RPM distribution. Linux USB serial access may require the distribution's serial-device permissions. A desktop with a StatusNotifier/AppIndicator host can display the tray icon; the app keeps its regular window when no host is detected.

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
