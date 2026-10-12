# Clock, focus timer, meeting and computer widgets

These five widgets run locally in the desktop app. They use the existing OLED frame protocol and firmware 1.1.0; the robot needs no new wiring or firmware for them. They can be added more than once, enabled/disabled, reordered, previewed, pinned and given their own playlist duration.

## Customize them

Choose **Add widget**, select the type, then use **Edit** beside it:

| Widget | Settings | What the OLED shows |
|---|---|---|
| Clock | Name, computer/IANA time zone, 12/24-hour format, seconds, weekday/date/no date | Large hours/minutes, zone and AM/PM, optional seconds beneath, date footer |
| Pomodoro | Name, focus/short-break/long-break minutes, sessions before long break | Remaining minutes/seconds, phase/session, progress and next phase |
| Meeting countdown | Meeting name, date/time in the computer's time zone, duration | Time until start; NOW during the meeting; DONE afterward. An unset time says SET TIME. This version uses manual entry. |
| CPU | Name, 1/2/5/10/30-second sampling, history on/off | Whole-computer CPU percentage and a last-minute history graph, or a percentage bar |
| Memory | Name, sampling, used/available, GB/GiB units | Amount, total capacity, selected percentage and a bar |

### Pomodoro controls

**Start timer**, **Pause timer**, **Reset timer** and **Skip phase** appear beside each Pomodoro in the playlist. They operate on that timer, independently of playlist Pause/Resume. A running timer continues through other screens, quiet hours, sleep and app closure, using a saved elapsed-time anchor. On reopening, its phase is calculated from the saved start time. Break transitions happen automatically; the OLED has no sound hardware. Changing phase lengths or session count resets the timer to paused. Renaming it preserves its current timer state.

Clock/meeting/Pomodoro values update from actual time. CPU/RAM use operating-system readings, with no network calls or process enumeration. CPU gets a warm-up sample before a percentage is shown. Disabled CPU/RAM widgets stop sampling; settings changes trigger a new reading. CPU history covers the available last-minute samples, so it fills after startup. Timers follow the computer's elapsed wall time, so an intentional system-clock change can adjust a running countdown.

## Same bot, more movement

**Settings → Display** contains **Animate the companion**, **Companion energy**, **Glance toward the information**, and **Playful Ahh and Ooo mouth shapes**. Lively is the default for the new companion settings.

The resting expression is the unchanged original happy face. Brief pupils look down/left toward the data and then return to the original hollow eyes. Ahh uses a wider open mouth; Ooo uses a narrower round mouth; both return to the original smile. A small one-pixel movement adds life. Blinks now last one 200 ms animation step, with lively/normal/relaxed periods of 2.4/3.6/6 seconds. The USB sender continues to honor the firmware's five-frame-per-second limit; actual observed blink duration also includes transfer time. The full happy-face widget shares these gestures, while its own Blink/speed controls remain available. Other selected expressions are preserved.

All companion movements stay in the reserved upper-right area and never change data pixels. Animation rests when the playlist is paused, during quiet/off periods, or when system reduced motion is enabled. Turning companion animation off restores its static familiar smile. The full face widget also has its own Blink switch.

## Verification record

**34 Rust behavior tests passed**, including saved timer anchors, pause/resume/skip/reset semantics, long breaks, timezone conversion, meeting boundaries, memory units and preservation of every data pixel during all companion gestures. TypeScript and production frontend builds passed; the final universal Mac executable contains Apple Silicon and Intel architectures.

Native Mac checks exercised Add/Edit for all five types, 24-hour clock with seconds, Pomodoro Start/Pause/Skip and length-change reset, natural focus/short-break/long-break transitions with short test intervals, editing a meeting time and observing NOW/DONE, live CPU history, and available/used RAM. The RAM total matched the independent system value of 34,359,738,368 bytes (32 GiB). Selected-board ACKs confirm delivered frame CRC and healthy OLED; On October 12, the user confirmed the clock, timer, meeting, CPU and memory screens are readable on the physical OLED, and the quicker blinks, glances and open-mouth movements are visible.

Normal Pomodoro lengths (25/5/15 minutes, four sessions) were restored and left paused. The test meeting was cleared and disabled so the user can enter an actual event and enable it. Original weather/stock/account settings, five original widget durations, brightness and orientation were retained. The new widgets are added at eight seconds each; clock is set to 24-hour time with seconds, CPU/RAM sample every two seconds, RAM shows used GiB.

The widgets and animation are published in [the 0.3.0 development release](https://github.com/greensky5656/DroidFun/releases/tag/v0.3.0), including Mac universal, Windows x64/ARM64 and Linux x86_64 DEB/RPM packages. Windows passed 33 behavior tests per architecture; Linux passed 34. Native installation/window checks passed on Windows and Ubuntu, while physical Windows/Linux robot and desktop acceptance remain open. The older 0.2.0 release remains available. The previous 24-hour run must be retained as an interrupted run when replacing its binary; its elapsed time is not added to the new run.

Technical sources: CPU sampling requires two separated readings, and disabling sysinfo's multithread feature avoids extra thread memory ([sysinfo documentation](https://docs.rs/sysinfo/latest/sysinfo/)); named-zone conversion uses the pinned timezone data and parser ([chrono-tz documentation](https://docs.rs/chrono-tz/latest/chrono_tz/)).

The final static baseline passed at **0.2221% average app CPU**, **69,468,160 bytes peak summed resident memory**, and 60/60 healthy checks. A replacement 24-hour run is active from October 12 at 2:01 PM Pacific until October 13 at 2:01 PM Pacific. Its result is pending. [Reliability record](RELIABILITY_TESTS.md), [exact new build verification](docs/verification/local-widgets-2026-10-12.json).
