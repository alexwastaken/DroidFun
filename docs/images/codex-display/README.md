# Codex OLED mark

The reference vector is the Codex mark from the installed official Codex app, `webview/assets/codex_new-f14177b03534.svg`. It identifies the Codex account-usage screen; it is not DroidFun branding.

`core.rs` uses a 24 × 24, one-bit rendering of this vector (alpha threshold 128), placed at (94, 4). The visible shape begins at row 6. The header moves from y=2 to y=7 in both information layouts. A short cursor extension changes only two pixels; the outline, usage value and bar remain stationary. Reduced motion, animation off and pause keep the resting frame.

![Exact renderer preview with sample usage](preview.gif)

The development-only `device_smoke --codex-assets` command exports the exact resting and pulse frames without opening USB. The preview uses sample usage.
