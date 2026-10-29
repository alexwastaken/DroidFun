# 0.10.3 — Correct remembered factory models

A physical Claw with valid factory firmware could remain displayed as Nimbus when its local profile remembered the wrong model. The USB connection was healthy, but the old profile took precedence and hid the enabled Claw usage screen.

On a fresh connection, the app now checks the selected board’s validated factory identity before sending screens. A different recognized model corrects only that board’s saved model. Friendly names, playlists, enabled flags, account configuration, display settings and other boards’ profiles stay intact. Unrelated, missing, invalid and cached identities cannot trigger correction. Board selection still stays explicit: the droid menu/arrows switch between attached boards, up to four.

Factory-recognized models are explained in Settings and cannot be changed through the model picker while connected. Older boards retain manual model choice with wording that identifies the selected droid. A stale general-settings form cannot overwrite a corrected model.

## Validation

- 158 native tests passed; zero failures and one existing live-account test ignored. Regression tests reproduce the Claw/Nimbus mismatch, preserve all saved fields, restore Nimbus and reject stale/unrelated identities.
- [13 browser scenarios](ui.json) passed at 360/500px, covering Claw views, switching, supported screens and model recognition controls. These use synthetic USB fixtures.
- Production frontend and local arm64 package builds passed; strict ad-hoc app signatures, bundled helper execution and matching DMG contents verified.
- [Physical Claw acceptance](mac-acceptance.json): Workshop read the existing Claw identity and firmware 1.3.2 without reprogramming. Installing 0.10.3 automatically corrected the stale app model, displayed Claw’s 3D view and usage controls, and showed fresh Codex usage over USB. A settings comparison confirmed only the selected model fields changed.

This candidate is installed locally. Public downloads/updater remain 0.10.0 while Developer ID signing, notarization and public package acceptance are pending. The board’s firmware and factory identity were not changed. No real board IDs, account usage values or local settings are included in this evidence.
