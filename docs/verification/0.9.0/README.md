# DroidFun App 0.9.0 verification

Application source: `2ac10739f795f17bdbde2636267abc736f0ea7e4`.

Customer diagnostics observes USB/replies for six seconds, shows allowlisted findings and next steps, offers a temporary live checkerboard when a matching droid replies, records physical visibility separately, and exports a reviewed local support report. The report includes a droid ID for owner Workshop matching. No reports are emailed/uploaded, no database access is added, and diagnostics does not flash or erase firmware. Generic manual recovery remains limited to identity-verified compatible standard legacy firmware.

- [Support UI](support-ui.json): ten cases cover first-launch welcome, Claw, no USB, factory, swapped wiring, legacy recovery, bootloader, Linux permission, offline and a failed check. Review alone does not start recovery; native preflight failure does not proceed to installation. Exports use the currently reviewed report ID and physical observation. Contact support requests only a fixed public help URL, with no customer or report parameters.
- [Regression UI](ui-verification.json): 32 connection, model/preview, recovery and app-update scenarios.
- [Custom screens](custom-screens-ui.json) and [image conversion](image-conversion.json): four existing feature flows and ten production raster checks.
- [Package verification](package-verification.json): exact-source hashes, native tests, signed updater checks and native installed-package smoke results.
- [Mac inspection](macos-load-command-verification.json): universal arm64/x86_64, minimum macOS 13 and strict signature verification.
- [Real Mac acceptance](mac-app-upgrade.json): installed upgrade, fresh no-device diagnostic and a reviewed report saved through the native file dialog.
- [Website downloads](website-downloads.json): six public files matched expected filenames, byte sizes and SHA-256 hashes.

Native tests cover code decisions, report privacy, protected firmware recovery, fresh ACK evidence across reconnect counter resets, observation/ACK separation, pattern geometry/expiry and normal protocol behavior. Existing firmware and settings schema 3 are unchanged. Native workflows: [Mac](https://github.com/alexwastaken/DroidFun/actions/runs/40766088173), [Windows/Linux](https://github.com/alexwastaken/DroidFun/actions/runs/40766093224).

The real Mac acceptance uses the existing user playlist and does not publish its private report contents or droid ID. Browser tests mock native commands. No physical screen-pattern visibility, USB cable comparison, firmware recovery or power-cycle acceptance is asserted. Mac notarization, Windows publisher signing and real Windows updater acceptance remain open.
