# Claude allowance recovery

Updated October 13, 2026. This account widget reads the allowance shared by Claude desktop, web and Code through the installed Claude Code sign-in. It displays allowance percentages, not remaining tokens. The account-usage endpoint is a compatibility integration, not a documented public API.

## Window state and sign-in state are different

The interrupted reliability run repeatedly reported “Claude allowance reset time was absent or expired.” The former normalizer required a future reset timestamp for every response, so an inactive or resetting allowance window removed Claude from the cycle even when endpoint access worked. A redacted read before the change found active, future-reset windows again; this upstream recovery must not be attributed to the fix.

The new reader handles these cases explicitly:

| Endpoint response | App and OLED behavior |
| --- | --- |
| Valid usage and future reset | Show the remaining percentage and actual reset time. |
| Explicit zero usage and absent/elapsed reset | Show 100% remaining, identify the allowance as inactive, and show no invented reset countdown. |
| Valid nonzero usage but absent/elapsed reset | Show `WAITING / 5H RESET PENDING` or its seven-day equivalent, keep the status in the cycle and continue polling. No current percentage is inferred from an elapsed window. |
| Missing allowance window, invalid usage, malformed timestamp | Report unavailable data; never turn missing data into zero usage. |
| Known reset before the five-minute poll | Recheck when the current card expires. |

A freshly fetched reset-pending status is not a usable allowance percentage. Diagnostics and reliability telemetry include `allowanceState` (`active`, `inactive`, `reset_pending`) so observers can distinguish them even though a current status card can remain in the cycle.

## Automatic credential recovery

On macOS/Linux, an expired stored access token invokes the installed credential owner in the existing, previously trusted app scratch folder. It sends no model prompt or terminal input, does not approve a trust dialog, and does not implement a separate refresh-token writer. The helper has a twenty-second deadline; credential reads use the remaining time budget. Its own process session and children are cleaned up when it succeeds, fails or times out.

If HTTP returns 401, the reader rechecks the shared local sign-in. A token rotated by another client is used immediately. If the same token remains, the app tries its bounded installed-client renewal path, requires a changed token, and retries the usage request once. Persistent rejection, missing client/trust or a failed renewal is an explicit authentication state with a five-minute automatic retry. HTTP 403 and quota failures do not cause an unbounded refresh loop.

This can renew refreshable access tokens. A revoked sign-in or a login that the credential owner cannot refresh requires the user to sign in again. The current Windows implementation retains its explicit reconnect path; automatic Windows renewal is not claimed by the Mac tests.

Only fixed authentication/window state labels and observation times are added to diagnostics. Credentials, identity, allowance amounts, profile paths and exact credential expiry times are excluded. The bounded event history records `credential_expired`, `credential_rejected`, `renewal_started`, successful renewal and failed/unavailable renewal states.

## Verification

The local Mac candidate passed **39 Rust behavior tests**, with one explicitly ignored live response-shape diagnostic, plus frontend checking/building and an ARM64 app build. The ad-hoc signature was verified. This is a local fix, not a newly published or notarized release.

Meaningful recovery checks include:

- Expired fixture credentials → owned terminal helper → renewed fixture credentials → authenticated local HTTP request → expected valid allowance card; owned helper reaped.
- HTTP rejection with a rotated shared token → one retry → valid card.
- HTTP rejection while local credentials appear valid → one renewal attempt → changed token → one retry → valid card.
- Persistent rejection, missing credentials and failed renewal → explicit error, no fabricated token or allowance.
- Inactive, reset-pending, absent and malformed windows → their distinct expected results.
- Helper success, early failure and timeout → process cleanup.

These use isolated fixture credentials and a local server. They do not expire or overwrite the user's sign-in. Live verification additionally requires the fixed app to fetch a valid account card and return Claude to the physical robot's cycling trace. Natural-expiration verification must record expiry/rejection, app-owned renewal, a subsequent newly fetched valid allowance card and a Claude cycle visit. A currently fresh card alone does not prove renewal.

The controls-closed resource run uses the normal nine-widget playlist and existing animations, not the earlier pinned static face. Its final evidence is recorded in [Claude and background verification](verification/claude-background-2026-10-13.json). Earlier partial reliability runs remain separate.

## Sources and boundaries

- [Anthropic: account usage limits](https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work) describes shared usage across Claude surfaces and the session/weekly limits.
- [Claude Code authentication](https://code.claude.com/docs/en/authentication) describes credential storage and the need to sign in again when a login cannot be refreshed.

Neither source documents the private account-usage response schema. Window handling above is compatibility behavior verified with explicit fixtures and redacted live states; further provider changes can require maintenance.

## Completed controls-closed check

The final local app completed a 900.20-second normal animated cycle with controls absent: 175/175 healthy samples, no gaps/missing measurements, 0.410% average app CPU, 65.05 MiB median app RSS and 69.66 MiB app peak. A captured temporary Codex reader raised combined owned-process peak to 183.23 MiB; its complete CPU cost and very brief helper peaks are not established by five-second samples. Settled memory decreased slightly. Claude was fresh/active throughout, with four fetch timestamps and nineteen cycle visits. [Full background report](verification/claude-background-2026-10-13.md).

The separate natural-expiry monitor completed successfully on October 13. Expiration did not occur during the fifteen-minute resource interval; it occurred later in the green-theme build. The evidence and its observation limits are recorded below.

## Natural renewal verified — October 13, 2026

The unchanged Claude backend in the green-theme build recorded `credential_expired` and `renewal_started` at **7:20:12 PM Pacific**, followed by `credential_renewed` at **7:20:14 PM**. A new valid `fresh` / `active` card was verified afterward, including a fetch at **8:15:18 PM** and a Claude cycle transition at **8:16:00 PM**. The earlier heartbeat already observed another fresh post-renewal card at 8:10:19 PM. Exactly one live Claude widget was enabled; app identity/hash and the renewal source hashes matched the pinned manifest. No other client, manual sign-in, artificial expiry or model prompt was invoked by the monitor.

This verifies natural automatic renewal and subsequent live-card/cycle recovery in this Mac app instance. The two-second interval is between integer event timestamps, not an exact measurement of helper runtime or first fresh-card latency. The first post-renewal cycle at 7:20:16 PM could still have used the previous usable card; first new-card latency and uninterrupted availability were not continuously measured. Credential contents, identity, allowance amounts and exact token expiry were not inspected. Earlier runs remain separate.

Private redacted proof: `/Users/hansparsons/.cache/deskbot/verification/2026-10-13-claude-recovery-final/natural-renewal-2026-10-13.json`, summarized in [verification](verification/claude-background-2026-10-13.json). The heartbeat is now **PAUSED** and temporary telemetry logging has stopped. The app was restarted without logging, preserving saved preferences and closed controls. Its former PID is still in an exiting kernel state with no executable path; no broad or forceful process kill was used.

**USB follow-up:** after restart, macOS reports neither the RP2040 USB device nor a USB serial node. The new app is alive, but physical reconnection is not yet verified; the user was asked to unplug/reconnect the robot. The cause is unestablished. This cleanup issue does not erase the prior natural-renewal/card/cycle evidence and does not establish Windows renewal or a full-day qualification.

## Publication follow-up — October 14, 2026

The user approved the blue/green branding selector with these Claude fixes for v0.3.2. The unchanged Claude/backend source hashes remain recorded in the verification manifest. Fresh native release builds and package evidence are separate from the October 13 resource interval and natural-renewal observation. The subsequent USB investigation restored normal firmware 1.1.0 and verified app connection, independent frame/health checks and user-confirmed readable cycling through the hub using the working phone cable/port; it did not modify the Claude code.

The v0.3.2 release is now [published](https://github.com/greensky5656/DroidFun/releases/tag/v0.3.2). Native behavior tests, installer/window checks and package hashes are recorded in [native build evidence](NATIVE_BUILDS.md#version-032-build-records). The exact delivered Mac app retained settings, connected to the hub robot and showed a fresh live Claude card; the earlier natural renewal and resource evidence remain separate.
