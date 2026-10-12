# Configurable providers: implementation and acceptance

Updated October 11, 2026. This records the macOS development build and observed behavior, not cross-platform release approval. The earlier API-key design is retained in `build/archive/provider-implementation-before-free-stock.md`.

## What is implemented

| Widget | In-app configuration | Data used |
| --- | --- | --- |
| Weather | City search and region choice, Fahrenheit/Celsius, name | Open-Meteo conditions and provider observation time |
| Stocks | Yahoo symbol/exchange suffix, name; separate instances | Free public regular-session quote, provider currency and quote date; no account/API key |
| Codex | Returned account bucket and allowance window, name | Signed-in Codex CLI account allowance, using the actual returned duration |
| Claude | Shared account or optional Code report; five-hour/seven-day window, name; browser connect control | Account allowance shared with desktop/web/Code, or an explicitly selected local status-line report |

Every instance has independent settings. Demo mode remains explicit. Unavailable live cards are skipped; missing values never become zero. Source changes invalidate prior cards, and an in-flight refresh cannot replace a newer configuration. Settings persist with an atomic write and backup.

## Free stock quotes

The former Alpha Vantage key, credential-entry controls, and request ledger are no longer used. Deskbot requests Yahoo Finance's public chart metadata. Enter a symbol such as MSFT; an exchange suffix such as SHOP.TO identifies a different listing. Currency is returned by the provider, rather than selected manually. GBp remains pence and is rendered as GBX on the monochrome display.

The adapter validates symbol identity, finite price, quote time, and currency. Quotes more than seven days old or with future/invalid times are unavailable. The renderer shows the price and a dated QUOTE/LAST footer. Details identify the last regular-session quote and warn it may be delayed. Older quote age is expressed in days/hours. There is no guarantee of real-time or after-hours coverage. [Yahoo Finance exchange/delay information](https://help.yahoo.com/kb/finance/article-exchanges-data-delays-sln2310.html).

Identical configurations share runtime cache entries. Successful stock requests refresh every 15 minutes. Transient failures follow 30-second, 2-minute and 10-minute backoff with jitter; HTTP 429 blocks requests to that provider for at least ten minutes or a longer Retry-After. Explicit refresh observes these limits. Calls have a 10-second timeout, a five-second connection timeout, bounded responses, cancellation, and shared in-flight request handling. On failure, an eligible prior stock/weather card is shown as stale until expiry. Stock responses become stale after 30 minutes without a successful fetch and expire at the earlier of three days after fetch or seven days after the quote.

This is a public endpoint compatibility integration, without a service guarantee. Endpoint changes, rate limits, and commercial redistribution/licensing remain release concerns. No paid subscription is required for the implemented personal prototype.

## Shared Claude account allowance

The default source now reads the account allowance, including desktop activity, without waiting for an interactive Claude Code status line. Claude confirms its web, Code, and Desktop surfaces share account limits. [Claude usage limits](https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work).

Deskbot uses the installed Claude Code sign-in. **Connect Claude account** runs its subscription login flow and lets the user finish authentication in the browser. It starts no model conversation. Duplicate connections are guarded and the owned login process has a ten-minute deadline. Existing credentials are read in memory from the specific macOS credential-store account or the client credential file on other platforms. Deskbot does not copy them into settings, evidence, or logs.

The reader requests `/api/oauth/usage` with the existing client session. This is an internal client compatibility endpoint, not an advertised public allowance API. Separate five-hour and seven-day utilization/reset fields are validated. Missing windows and expired resets become unavailable. Account requests refresh every five minutes. Rejected/expired sign-ins explain how to reconnect; completion invalidates the account cache. Percent remaining means account allowance, not a remaining token count or context size.

The optional Code status-line source is still available. Its native helper writes only allowlisted allowance fields, a local session ID and receipt time, preserving the original command's output. Its integration is backed up/restorable and works with Deskbot closed. It does not represent a standalone desktop-only feed. [Claude status-line input](https://code.claude.com/docs/en/statusline).

## Verification completed

| Check | Expected and observed result |
| --- | --- |
| Frontend/native build | TypeScript and production frontend build passed; Apple Silicon app bundled successfully |
| Rust behavior checks | 17 passed: scheduler/eligibility, protocol corruption recovery, source independence, units, matching quote identity/time/currency, pence preservation, separate allowance windows and missing/expired fields |
| Free stock live comparison | Public MSFT response and native preview both show 535.07 USD, last quote October 9. This also matched the [Yahoo historical page](https://finance.yahoo.com/quote/MSFT/history/?ltr=1). No key or stock account was used |
| Claude login | User completed the browser flow opened by Deskbot; native editor reported account connected |
| Claude independent comparison | Separate read-only account response returned 6% five-hour usage and 61% weekly usage: 94% and 39% remaining. Native five-hour preview showed 94% and the matching October 12, 2:59 AM Pacific reset |
| App update/restart | Final bundle relaunched, retained brightness 88 and the saved five enabled widgets, reconnected to replacement board 505431651938581C with firmware 1.0.2 |
| Previous live checks | Weather city search and Fahrenheit/Celsius; authenticated Codex returned seven-day window; local status-line helper output preservation and allowlisting passed |

No paid Claude model turn was started to verify this repair. Screenshots: [Claude account preview](build/claude-account-live.png), [free quote preview](build/stock-free-live.png). Artifact hashes and acceptance boundaries: [verification record](build/verification-2026-10-11.json).

## Version 0.2.0 update, October 11–12, 2026

Weather daily high/low and optional stock change layouts are implemented. Native previews showed San Diego 64 F / H71 L64 F, and MSFT 535.07 / USD +2.4% / October 9. The independent MSFT response had comparable previous close 522.61, giving +2.384% before display rounding. These are dated observations, not current market claims. Original user layouts were restored after testing.

The four data widgets now retain the original happy face at half size in a dedicated upper-right corner. Animated frames and the local preview share the Rust renderer. The first small-face implementation was physically confirmed to blink without obscuring text; its enlarged matching-face revision is recorded separately in the completion record.

Independent workers prevent a slow provider from stopping other sources. Changing or disabling a widget cancels pending work. Shared HTTP requests and provider-level rate limits are tested against a local HTTP server. Weather/stock cache survives restart only for matching configuration and valid expiry; allowance cards remain in memory. Optional Claude profile folders and local report directories/profile selectors are available in its advanced editor.

27 Rust behavior tests pass, including missing/null/reset allowances, public cache restart/expiry, cancellation, deduplication/quota, and corner-only animation. The native app reconnects to firmware 1.1.0. The user confirmed all five live screens are physically complete and readable.

## Remaining boundaries

Windows/Linux native builds, credentials/login, tray behavior and USB permissions require native machines. Claude account switching across multiple real profiles remains untested; default signed-in account behavior is verified. Exchange-calendar and redistribution/retention policy review remain release work. Claude account and public Yahoo endpoints are compatibility integrations and may change. Signing, full accessibility and the soak gates are listed in [the completion record](SPEC_COMPLETION.md).
