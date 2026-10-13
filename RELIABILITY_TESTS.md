# Mac and robot reliability tests

October 12, 2026. This record supplements [spec acceptance](IMPLEMENTATION_SPEC.md) and [completion status](SPEC_COMPLETION.md). A started soak is not a passed soak. The separate 0.3.0 Windows/Linux release packages were built and published on October 12; those native package checks do not replace this Mac/robot run.

## Preflight evidence

Machine: MacBookPro18,2, 32 GiB RAM. Replacement RP2040-Zero: firmware 1.1.0, GP4 SDA / GP5 SCL, OLED address 0x3C. The operator explicitly selects this board; test tools never open another board automatically.

| Check | Result / boundary |
|---|---|
| Physical display | User confirmed enlarged original smiling companion and all widget text remain readable. |
| Damaged USB packets | Independent Python codec: 12 checks passed on the real board, including CRC, truncation, invalid COBS, flags, session/sequence rejection, partial-packet timeout, duplicate handling and valid-frame recovery. Frame CRC and nonzero-byte counts match the independent sent buffer. |
| Orientation, display policy and explicit fallback commands | Board acknowledged these commands. ACKs alone do not establish physical pixel appearance. |
| Build checks | 29 Rust behavior tests, TypeScript check and production frontend build passed. Universal macOS app built with both architectures. This Mac verifies only the Apple Silicon runtime. |
| Closing/reopening controls | Backend, providers and USB remain active. Web Content is released on close; opening controls recreates the panel and reads the existing state. Relaunch activates the same app instance. |
| Account recovery | Installed Claude client refreshed the expired local sign-in without a model prompt or a new browser login. All four live data sources became fresh. Automatic renewal after the next natural expiration is still a soak gate. |
| Initial host stall | User saw weather/blank; not passed. Its frame comparison was also confounded by a live weather refresh on resume. Retained as evidence, not used to assert a static-frame failure. |
| Repeat host stall | Fixed demo screen, 120-second host pause. User confirmed the large happy face appears and dims. After resume, a new frame ACK confirms the exact pre-pause bitmap CRC. Ten-minute blanking and real OS sleep/wake remain open. |
| Physical USB removal | User unplugged/reconnected using a different port. The OS path changed from usbmodem1101 to usbmodem2101; the same selected board reconnected and acknowledged the expected image within an observed upper bound of about 1.3 seconds after the new port appeared. |
| Static resource baseline | Final lazy-panel app: 300.11 seconds, 60/60 healthy samples, average summed CPU 0.1458%, peak summed RSS 73.59375 MiB (77,168,640 bytes), no sampling gaps or issues. Face pinned, blinking disabled, controls closed, all live providers available. Initial older-build memory excess and early collector-start observations are retained; neither is represented as a pass. |

Raw local evidence is under `build/reliability/preflight/` and is ignored by Git. It contains timestamps, redacted source/connection states, frame checksums and process measurements; no account values, credentials, identity, city, stock symbol or model prompts are recorded. The protocol tool's private local report includes the selected board identity.

## Fixes made for this run

- Verify each frame ACK's checksum and OLED health before treating it as delivered.
- Resend display policy and unchanged pixels after a host pause long enough for firmware fallback.
- Release the controls' webview when it closes; keep cycling and data collection in the native backend.
- Create controls only when requested, so a background launch never constructs a hidden webview. Native controls and Settings reopened successfully after the final background baseline.
- Retry authentication failures after five minutes so externally refreshed credentials can be detected.
- For expired Claude credentials on Unix, allow the installed client to renew them in an already trusted Deskbot scratch folder, without submitting a model prompt. The bounded helper is cancelled/reaped; it never answers a trust prompt or writes credentials itself. Missing trust/client and Windows use the existing manual reconnect path. Full natural-expiration acceptance is pending.
- Correct the diagnostic checkerboard, which previously produced stripes.
- Add opt-in local measurements and scoped failure markers. Normal runs without `DESKBOT_RELIABILITY_DIR` do not create these logs or inject faults.

## 24-hour run procedure

The collector observes the existing app and never opens its serial port or restarts a crashed app. A per-user LaunchAgent runs it independently of this conversation. Scoped `caffeinate -i` prevents idle computer sleep for the test and exits with the collector; the display can sleep. Closing the laptop, manual sleep, shutdown, app exit, a disconnected robot or an unavailable volume can invalidate continuous observation.

Planned disruptions affect Deskbot only:

- 20–40 minutes: block its HTTP reader, allowing cache expiry and provider retries.
- 60 minutes: drop its host serial connection for 20 seconds, then allow reconnection.
- 12 hours: repeat the 20-minute HTTP interruption.
- 18 hours: repeat the 20-second host serial interruption.

These exercise real app recovery but do not replace a physical unplug/replug or real operating-system sleep test. The robot remains attached and powered. Markers are removed on normal/interrupted collector exit.

### Completion review

1. Require at least 86,400 seconds, fresh measurements throughout, no unexplained gaps or process exits, and all configured widgets cycling outside unavailable-source intervals.
2. Review every connection event, provider-state change and fault clear. Correlate frame ACKs with current frame CRC; verify reconnection and a restored frame within five seconds of the measured return of access. Physical unplug timing requires separate OS-enumeration evidence. During animation, separately sampled current and acknowledged frames can be adjacent animation phases; a single snapshot mismatch alone is not failure. The connection worker strictly validates the CRC of every actual sent frame.
3. Confirm unavailable sources never display invented zeroes and return to fresh values after interruption within their documented cache/backoff schedules. Confirm Claude survives its next natural token expiration or explicitly report a failure.
4. Review process memory over the entire run, comparing settled early/recent medians and inspecting peaks for growing trends. Include native app, named WebKit services and sampled helper descendants; only the optional Codex child subtree is excluded and reported separately. Summed RSS can double count shared pages and brief helpers can fall between samples.
5. Static CPU/memory gates use a separate five-minute pinned, animation-disabled, closed-panel baseline: average CPU below 1%, summed resident memory below 100,000,000 bytes. Cold startup transients remain visible in the record.
6. Keep physical acceptance separate: user-observed readable pixels and fallback/dimming are evidence; CRC/ACK is transport evidence. Ten-minute blanking, physical USB removal/path change, OLED disconnection, rotated patterns, supply/pull-up voltages and real OS suspend remain open until actually exercised.

If a defect requires changing the running code, preserve the interrupted evidence and start a new full 24-hour run after the fix. Do not silently join partial runs or label elapsed time alone as a pass.

### Live run

**First run interrupted for the user-authorized feature update after about 81 minutes; not a 24-hour pass.** It started October 12, 2026 at 11:49:14 AM Pacific. Its original deadline was October 13 at 11:49:14 AM Pacific; the replacement run below has a new deadline. The normal five-widget playlist and original blinking/companion animation settings were restored. At the launch check, all four live sources were fresh, all five kinds appeared in the transition trace, and the first 21 observations were healthy with no gaps or issues.

- Internal directory: `/Users/hansparsons/.cache/deskbot/reliability/2026-10-12-24h`.
- App PID at launch: 17120. Collector PID: 17124. Scoped idle-sleep prevention runs with the collector.
- Former per-user LaunchAgent: `com.deskbot.reliability.20261012`, unloaded and removed when the run was stopped.
- Internal app executable SHA-256: `7809accf981999156b6e2f92e14e58759121f121e08667a4d9457905e86c70b3`, identical to the final local `build/Deskbot.app` executable. App and collector run from the internal disk, independently of the external source volume and conversation.
- Heartbeat: `deskbot-24-hour-reliability-test`, hourly, active. It stays quiet while healthy and unchanged; it reviews failures/completion, preserves evidence, cleans up this run's LaunchAgent/markers, then pauses itself.
- `progress.json` reports running observations. `report.json` is created only when the collector ends. Source/binary hashes and preflight measurements are in [verification](docs/verification/reliability-2026-10-12.json).

After completion or a terminal failure, unload only this run's LaunchAgent and remove its exact plist. Clear its owned fault markers. Preserve all reports. To stop ongoing diagnostic logging, gracefully relaunch only the measured app without `DESKBOT_RELIABILITY_DIR`, retaining saved preferences and whether controls were open. Leave the robot cycling normally.

## Tools

- `scripts/robot-reliability.py`: exclusive selected-board protocol and pattern checks; requires pyserial 3.5 and Deskbot quit.
- `scripts/app-stall-test.py`: bounded pause/resume of the selected app, static-frame ACK comparison; always resumes on normal exit or interruption.
- `scripts/reliability-soak.py`: local observer; optional scoped faults/idle-sleep prevention. Existing run directories cannot be overwritten.

Additional widget proposals: [30 ideas](WIDGET_IDEAS.md).


## Replacement run: local widgets and richer companion

**Running, result pending.** Started October 12 at 02:01 PM Pacific; due October 13 at 02:01 PM Pacific. Directory: `/Users/hansparsons/.cache/deskbot/reliability/2026-10-12-local-widgets-24h`. App PID at launch 46472; collector PID 46479. LaunchAgent `com.deskbot.reliability.20261012.widgets` runs the one-shot collector from the internal disk; its exact plist is `/Users/hansparsons/Library/LaunchAgents/com.deskbot.reliability.20261012.widgets.plist`. The hourly heartbeat follows this replacement run. The shared internal `active-run.json` identifies it.

The original four live sources plus clock, Pomodoro, CPU and memory are enabled alongside the face (nine active widgets). The manually entered test meeting was cleared and disabled; an actual event must be configured by the user. Review enabled kinds per sample rather than requiring a disabled meeting to cycle. Normal animation is restored, with lively companion energy, glances and mouth shapes. Pomodoro is paused at 25 minutes, with 5-minute short breaks, 15-minute long breaks and four focus sessions. The timer controls and natural phases were tested before launch.

Physical review: on October 12, the user confirmed the clock, timer, meeting, CPU and memory screens are readable and the quicker blinks, glances and open-mouth movements are visible on the connected robot. This confirms the new visual behavior; the continuous 24-hour run remains pending.

The final five-minute closed-panel static baseline passed: average app CPU **0.2221%**, peak summed RSS **66.25 MiB** (69,468,160 bytes), 60/60 healthy observations and no issues/gaps. This baseline includes the new enabled monitors and local widgets. [New build/source verification](docs/verification/local-widgets-2026-10-12.json) records the exact executable. Old reports remain separate.

The collector now gives the deliberate host-link return its five-second recovery window, avoiding an immediate false alert from the telemetry snapshot written just before fault removal. Actual recovery timestamps and all raw snapshots are retained; this grace does not establish a five-second recovery pass by itself.

Cleanup must use the replacement run directory and replacement LaunchAgent label/plist, and only its measured app PID. Never remove unrelated files or restart another process sharing a name. The previous first-run collector/plist and app binary are archived. Do not join elapsed times from different binaries.

### Release publication during observation

Version 0.3.0 was published on October 12 with the same widget/animation production source and a package version bump. The exact measured internal executable, selected robot, saved preferences, collector and fault schedule were left unchanged. The release package was not launched during the run. [Native build evidence](docs/NATIVE_BUILDS.md#version-030-build-records) is separate from continuous observation and physical acceptance.

### First-hour recovery review (October 12, 3:09 PM Pacific)

The app and collector remain alive on the exact measured executable; measurements are fresh, with no sampling gaps or unexplained issues. All nine enabled widget kinds appear in the transition trace. The three nonhealthy snapshots fall inside the deliberate host-link interruption/return grace. HTTP access returned and weather, stocks and Claude became fresh again within their bounded retry schedules (first observed fresh approximately 484, 513 and 709 seconds after return). Codex and the local widgets remained fresh. Claude later had a short unavailable interval followed by fresh data; the redacted card expiry is an allowance-window boundary and does not establish natural credential expiration. That gate remains pending. No credentials, identity or allowance values were inspected.

The deliberate host link reconnected with OLED ready in the same recorded second as marker removal. The next fresh telemetry snapshot has 12 validated frame ACKs and matching current/acknowledged CRC. Its enforced 200 ms minimum frame spacing gives a **derived first-frame upper bound below 3.8 seconds** after access returned; the first frame timestamp itself was not captured. Recovery grace alone was not used as evidence. This is host-link recovery, separate from the earlier physical unplug test.

The controls were opened during the run. Early background versus recent open-panel memory totals have different scopes and are not treated as a growth failure; the startup WebKit peak subsequently decreased. The separate five-minute static resource test remains the static gate. Full-run memory review and the 24-hour result are still pending.

### Second-hour observation (October 12, 4:13 PM Pacific)

App, collector, LaunchAgent and scoped idle-sleep prevention remain active. The latest hour has no unexplained unhealthy observations or sampling gaps, at most one pending request, all enabled live sources fresh throughout, and all nine enabled kinds cycling in the transition trace. Controls remain open; memory trends must be compared with the same panel state. No additional faults occurred. Natural Claude credential expiration is not established from the redacted states; the complete 24-hour result remains pending.

### Third-hour observation (October 12, 5:15 PM Pacific)

The app/collector and scoped idle-sleep prevention remain active. The latest hour is connected throughout, with all enabled live sources fresh, all nine kinds cycling, no unexplained unhealthy observations or gaps, and at most one pending request. Controls remain open; summed app RSS has a latest-hour median of 189.72 MiB, comparable to the preceding open-panel hour (189.61 MiB). No new fault events or required changes occurred. Natural credential-expiration proof and full 24-hour acceptance remain pending.

### Fourth-hour observation (October 12, 6:18 PM Pacific)

The app, collector, LaunchAgent and scoped idle-sleep prevention remain active. Since the previous review, connection stayed ready, all enabled live sources stayed fresh and all nine widget kinds cycled, with no unexplained unhealthy measurements or gaps and at most one pending request. Controls remain open; summed app RSS median is 189.69 MiB, compared with 189.72 MiB in the preceding open-panel hour. No additional faults or required changes occurred. Natural credential-expiration proof and the complete 24-hour result remain pending.

### Claude renewal acceptance requirement

The user reaffirmed that the run should continue and notify only on completion/issues, with explicit verification of Claude after natural sign-in expiration. Require redacted evidence of expiration or the Deskbot-owned renewal helper, followed by a newly fetched fresh Claude card and return to cycling. A usage-window reset or fresh data alone is not credential-renewal proof. Inspect authentication failure persisting beyond the five-minute retry, 20-second helper and sampling margin. Do not manually renew credentials or invoke another client to conceal an app failure. If available redacted evidence cannot establish expiration, leave this check unverified and provide a follow-up verification plan at completion.

### DroidFun App 0.3.1 rebuild during observation

The branded 0.3.1 development release was rebuilt and published without replacing or launching over the measured internal app. Its executable hash, app/collector identities and fresh progress were verified unchanged. The robot run retains its October 13, 2:01 PM Pacific deadline; its 24-hour result and natural Claude credential-expiration proof remain pending. Native package evidence is separate in [0.3.1 build records](docs/NATIVE_BUILDS.md#version-031-build-records).
