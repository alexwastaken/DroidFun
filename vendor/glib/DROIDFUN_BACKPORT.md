# GLib 0.18.5 security backport

This is the published `glib` 0.18.5 crate, with the two-line upstream fix for
[RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)
backported into `src/variant_iter.rs`. Its MIT license and copyright notices
remain in `LICENSE` and `COPYRIGHT`.

## Provenance

- Source: <https://static.crates.io/crates/glib/glib-0.18.5.crate>
- Original archive SHA-256: `233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`
- Upstream fix: <https://github.com/gtk-rs/gtk-rs-core/commit/b5a4071e439bef2b5eea76c3aa25e5ae84839e34>
- Change: in `VariantStrIter::impl_get`, make the output pointer binding mutable
  and pass `&mut p` to `g_variant_get_child`, rather than an immutable reference
  to storage the C function writes. No other original package file is changed.

The version remains 0.18.5. A version-only scanner may continue to report the
advisory; the patch and its regression evidence are the disposition, not a claim
that upstream published a fixed 0.18 release. Tauri's GTK3/WebKit dependencies
still require this incompatible older GLib line. Remove this override when the
whole dependency chain accepts a maintained, fixed upstream version (at least
0.20); merely adding a newer GLib alongside the old one does not resolve it.

## Focused regression

The application's `src-tauri/tests/glib_variant_str_iter.rs` exercises `next`,
`next_back`, `nth`, `nth_back`, and `last`, including Unicode and exhausted
iterators, against this same Cargo-resolved patch. On Linux with the normal
desktop build prerequisites installed, run from the repository root:

```sh
cargo test --release --locked --manifest-path src-tauri/Cargo.toml --test glib_variant_str_iter --target x86_64-unknown-linux-gnu
```

The optimized build matters because the original undefined behavior can leave
the returned pointer null after optimization. The native Linux package workflow
runs this command before producing an installer. It does not compile or run the
unrelated upstream GLib test suite.
