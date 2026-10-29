# 0.10.4 — Automatic selection after swapping droids

The app could keep waiting for an unplugged selection even when a different droid was the only connected board. This candidate identifies the sole available replacement, restores its profile and reconciles its factory model before sending screens. A selected board that is still attached stays active. Several possible replacements require a choice through the existing menu or arrows; the four-board limit remains.

The automatic selection transaction captures the selected ID and available candidate, then rechecks both after the read-only USB handshake and any legacy model lookup. Late replies cannot replace a new choice or select a board that has disappeared. Probe history resets when the selection context changes, so returning to a continuously attached board does not stall. Temporary failures remain retryable; a valid unknown legacy board can still ask for its model once.

Non-null malformed factory identity now fails the handshake instead of falling through to a remembered or guessed model. Explicit model choices for new boards also read the current handshake, and a valid factory model takes precedence. Valid factory identities on standard and swapped screen wiring remain supported. No firmware, wiring or database permissions change.

Settings drafts and model pickers close when the selected board or recognized model changes. A new valid identification clears obsolete manual-model prompts. Initial menu-bar Settings requests and unrelated error messages are preserved.

## Validation

- 170 native tests passed, zero failures; one existing live-account test remains ignored. The committed regression tests cover all models, board-specific settings, two boards of the same model, stale replies, ambiguous choices, slot limits, legacy fallback, malformed identity and returning to a still-attached board.
- [20 browser lifecycle scenarios](ui.json) passed at 360/500px using synthetic USB events, with no browser errors or horizontal overflow.
- Production frontend and local arm64 package builds passed. The app's strict ad-hoc signature, bundled helper execution and matching DMG contents were verified.
- Local installation launched as 0.10.4 and preserved every saved settings field. See [local acceptance](mac-acceptance.json) for physical-test status.
- Read-only Workshop source review confirmed preparation checks bind board ID, model, wiring and identity generation to the job and recheck them after the required power cycle before marking a droid prepared.

This candidate is installed locally. Public downloads/updater remain 0.10.0 while Developer ID signing, notarization and public package acceptance are pending. Browser fixtures and native regression tests do not substitute for physical multi-droid, cable-swap or power-cycle acceptance. Evidence contains no real board IDs, account values or local settings.
