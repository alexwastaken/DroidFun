# DroidFun App 0.10.7 screen update

Source: `106b0128229214647c754fa3dffdf81f035908a3`.

Clock uses the same simple time-only renderer as Workshop. Calendar is separately addable for Nimbus, Boo and Meep, with month/day and optional weekday or year. Time zone, inversion, screen enablement, duration and ordering remain independent. It is stored as a clock layout, preserving schema-3 compatibility; older versions read the playlist but may render Calendar as a clock. Existing playlists are not automatically extended. Claw remains usage-only.

The shared elapsed-time marquee renderer now ships in this companion candidate, with an editor preview refreshed up to 30 FPS. USB pacing still respects each board’s negotiated limit, and pause/reduced motion remain supported. No permissions, endpoints, credentials or factory firmware were added to the public app.

The [verification record](screen-update.json) includes exact source, package hashes, native installation checks and updater signature results. [Screen UI checks](screen-ui.json) exercised the production React editors with simulated native/USB calls for the three general models and checked Claw exclusions. [Layout preview](screen-layouts.png) was generated from actual native clock/calendar renderer output at a fixed timestamp, not from the browser fixture.

The owner’s installed Apple Silicon app was replaced with 0.10.7. Strict bundle/helper verification and matching DMG contents passed; the real native UI showed 0.10.7 and the new Calendar option. The original settings file remained byte-for-byte identical after replacement, launch and menu inspection. No physical board was connected or flashed. The previous app bundle is preserved locally.

Public publication is not complete: the website and automatic updater remain on 0.10.0. Mac packages are explicit internal ad-hoc builds, not Apple Developer ID signed/notarized. The release tooling correctly excludes these from the public updater. Workshop 0.4.5 and its factory firmware 1.3.3 are a separate published release; the new wall-power blink requires reprogramming through Workshop.

## Physical Meep acceptance — November 2, 2026

In a subsequent owner-authorized installation, Workshop was updated from 0.4.1 to 0.4.5 without replacing its settings or pairing credentials. Workshop identified one standard-wired Meep on 1.3.2 and installed 1.3.3 through Reprogram. Firmware, board identity and screen communication passed. The user removed all power, reconnected the board and confirmed the face blinked; Workshop verified retained identity and startup face. The private inventory shows 1.3.3. DroidFun App 0.10.7 then recognized Meep automatically, reported 1.3.3, and passed USB, fresh-reply and screen-controller diagnostics. The app was left playing the existing playlist. See [acceptance record](meep-install.json); physical IDs and credentials are excluded.
