# Windows Claude automatic sign-in recovery

October 14, 2026 — v0.3.3 candidate; native Windows acceptance pending.

The app reuses its existing expired-token and HTTP 401 recovery flow. An expired local sign-in starts the installed native `claude.exe` in a hidden Windows pseudoconsole, reads back a valid local credential and fetches a current account card. Server rejection first checks for another client's rotated token; if unchanged, it attempts owned renewal and retries HTTP once. Persistent rejection stays an explicit reconnect state with the normal five-minute retry.

## Terminal and process lifetime

The app supplies the existing no-prompt safe-mode arguments, disables updater/nonessential traffic for that child and keeps the requested Claude profile. It removes inherited API-key/token overrides from the child so they cannot redirect recovery to another sign-in. It launches the exact executable with Unicode arguments directly, without a command shell. No keyboard input, inference prompt, trust approval or credential writer is implemented.

The helper runs in an empty app-owned folder. It never records a workspace-trust decision. Authentication can refresh during installed-client startup; a client requiring interactive onboarding/trust, a missing native client or a revoked/unrefreshable login returns the explicit reconnect state. Initial installation/sign-in must be completed normally. The app does not conceal a setup prompt or claim success from a successful process launch alone: it requires a newly valid credential, and requires a changed credential after server rejection.

A private job is attached atomically at process creation, before the first thread runs. Closing that job kills its owned descendants, including on app exit. The helper has a twenty-second operation budget; termination waits at most two additional seconds for its root. Only that owned job is terminated. Terminal output is discarded with bounded reads, never saved. Output is closed before pseudoconsole shutdown to avoid the older-Windows final-frame deadlock. No generic process-name kill is used.

This path requires Windows 10 version 1809 or later / Windows Server 2019 or later. The release still uses the native installed `claude.exe`; WSL-only and npm command-script installations without that executable are not newly supported.

## Native verification plan

Six Windows-only tests compile an isolated console fixture with the native Rust toolchain. The fixture requires real console handles and the exact safe-mode arguments/environment. Credentials and HTTP responses are fixtures; no real account store or external model endpoint is used.

1. Expired fixture → real ConPTY → fixture credential owner rotates the selected Unicode/space-containing profile → authenticated localhost request → expected fresh inactive allowance card → Claude becomes the only enabled engine cycle widget. Owned helper and child stop; an unrelated decoy remains alive.
2. HTTP 401 with a locally valid rejected fixture → real ConPTY owner rotates it → exactly one authenticated retry → fresh card and cycle eligibility.
3. Timeout under sustained terminal-output flooding → bounded error; owned tree stopped; no sign-in fabricated.
4. Early client failure → owned descendant stopped; no fabricated sign-in.
5. Source-change cancellation → owned tree stopped.
6. Missing executable → explicit bounded error; no client created.

Windows x64 and ARM64 execute these alongside the existing native suite, followed by installer payload/architecture and native-window checks. Mac/Linux retain their existing renewal and behavior tests. Build and fixture checks are not a natural live Windows sign-in-expiration observation. That remaining account acceptance must be recorded separately with redacted expired/rejected → renewal → new valid card → cycle events, without inspecting credentials or allowance amounts.

## Primary sources

- [Microsoft pseudoconsole lifecycle](https://learn.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session) describes console attachment to a child process and pipe lifetime.
- [Microsoft ClosePseudoConsole](https://learn.microsoft.com/en-us/windows/console/closepseudoconsole) documents closing/draining output before shutdown, older-Windows deadlock behavior and minimum OS versions.
- [Microsoft job assignment at process creation](https://devblogs.microsoft.com/oldnewthing/20230209-00/?p=107812/) describes atomic job assignment with `PROC_THREAD_ATTRIBUTE_JOB_LIST`.
- [Microsoft job objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) documents job ownership and kill-on-close.
- [Claude Code CLI reference](https://code.claude.com/docs/en/cli-reference) documents native authentication commands and safe mode. Automatic local credential renewal during startup remains client compatibility behavior, not a documented public refresh API.
