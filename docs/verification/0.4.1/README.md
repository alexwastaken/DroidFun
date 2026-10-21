# DroidFun App 0.4.1 verification

This release gives Boo, Claw and Meep matching controls, settings, screen editors, avatars and preview housings. The blue Droid Fun wordmark remains shared. Guide 1.5 describes automatic themes; firmware remains 1.2.0.

`source-commits.json` identifies the exact source used for all packages. Native platform records describe tests, installer checks and limits. `theme-verification.json` records 42 color-contrast comparisons and CSS parser results; this is not a complete accessibility audit. `mac-theme-verification.json` and the screenshots record isolated native light/dark UI checks. The final idle-caption correction was checked by the Linux native smoke test after the Mac screenshots were captured.

All five installers and the PDF were downloaded again from the public website and matched their recorded SHA-256 checksums. Website product and store settings were unchanged. The source repository is public; website updates remain manual. These release records describe the original 0.4.1 validation, before the October 21 open-source publication.

No physical robot was connected or flashed for this visual update. Save-to-bot USB upload and wall-power reboot acceptance remain outstanding. macOS packages are ad-hoc signed, not notarized; Windows installers are unsigned. Intel Mac and Fedora RPM runtime are untested. The installed copy on Alex’s Mac was not replaced by this isolated preview.
