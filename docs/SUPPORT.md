# Customer diagnostics and support

Use **Connection → Run diagnostics** (or Settings → Droid/App → Run diagnostics). Keep the case closed. Choose the symptom and run checks with one droid connected through a USB data cable.

The app observes the existing connection for six seconds and performs a bounded USB/boot-drive search. It checks current replies, reported display health, repeated state changes, screen-off/quiet/sleep policy, brightness, enabled screens, pause state and unavailable online sources. A short check can miss intermittent faults; run it while the fault occurs. A missing device cannot distinguish a power-only cable, computer USB problem or faulty hardware.

**Show test pattern** uses the existing live FRAME/DISPLAY protocol for eight seconds. It temporarily uses visible brightness and screen-on, preserving orientation. It does not install a sketch, write firmware, save a playlist or change settings. It is bound to the report’s connected board and expires automatically. Normal output resumes when the connection is available. The customer records whether the checkerboard was actually visible; an ACK alone does not prove visibility. Claw can use this temporary diagnostic pattern without adding a text/image widget.

Choose **Save support report** after reviewing its contents, then attach the JSON file to a message through the contact link. The app does not email, upload or store reports on the website. The report includes the hardware ID (which may be a remembered ID when disconnected), selected/reported model, app/OS/architecture, firmware and wiring, check codes and physical observation. `modelSource` distinguishes an app selection from a droid’s last reported model. Credentials, account details, user names, screen labels, custom text/images, locations, stock symbols, raw errors and file paths are excluded. The report is allowlisted rather than a copy of logs; Unix exports are owner-only. A new run replaces the in-memory report; saving requires its matching report ID.

## Manual firmware recovery

A current, identity-matched standard legacy droid through 1.2.4 on a supported Mac may offer **Review firmware recovery → Reinstall droid software**. This reuses the native identity-bound recovery preflight and verified direct-USB installer. The computer’s playlist is kept; wall-power screens may need saving again. Review alone never starts recovery.

Factory-prepared 1.3.2, 1.3.2-mistake, newer/custom firmware, unsupported hosts and unidentified bootloaders do not get a generic reinstall. A failed reply is not proof that firmware is absent. Use the report to identify the droid in private Workshop and choose the correct model/wiring before owner servicing. Do not use the old temporary firmware screen-test sketch on factory or swapped-wiring boards. Desktop app updates remain separate under Settings → App.

## Owner triage

1. Read the report ID, symptom, droid ID, firmware/wiring and check codes. Match the hardware ID to Workshop → Droid list or the owner Droids dashboard. A remembered ID is not proof that the device was attached during the check; use the USB/reply checks.
2. Follow the reported next steps before any firmware service. Ask for the saved JSON and the customer’s description/photo if the physical screen result is unclear. Reports cannot diagnose a specific bad solder joint or prove a cable/power fault.
3. For firmware service in Workshop, retain the reported model and actual pin profile. A Mistake board uses GP5 SDA / GP4 SCL. Reinstall/wipe only as an explicit owner service operation; a hardware screen fault may need repair/replacement.

| Codes | Meaning and first action |
| --- | --- |
| DF-USB-01 / 04 | No current data device or selected device absent: compare data cable/direct port and select the attached droid. |
| DF-USB-02 / 03 | Recovery mode or ambiguous selection: reconnect normally; test one droid at a time. |
| DF-USB-05 | Enumeration did not finish: reconnect and repeat; native driver timeout does not imply missing firmware. |
| DF-LINK-01 / 02 | Missing fresh replies / Linux permissions: close competing software or use the USB access flow. |
| DF-LINK-04 / 05 | Selection changed / repeated transitions: repeat with one selection and compare cable/port. |
| DF-SCREEN-01 | Firmware reports screen-controller fault: inspect correct wiring and hardware in Workshop. |
| DF-DISPLAY-01 / 02 | Display off/resting or brightness low: review Display settings; temporary test bypasses these policies. |
| DF-PLAY-01 / 02 | No enabled screens or paused cycle: enable/resume. |
| DF-DATA-01 / 02 | Online data paused or unavailable: inspect screen source/sign-in; firmware replacement is not the remedy. |
| DF-VISUAL-01 | Customer could not see the checkerboard: combine this with ACK/health results; may require hardware service. |

Software tests cover report privacy, decision codes, protected recovery, observation/ACK separation, pattern geometry/expiry and UI flows. Physical screen visibility, cable comparison and actual firmware service require a connected board; do not infer those from fixture tests.
