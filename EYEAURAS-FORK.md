# EyeAuras fork of tsrs

This fork is the tsrs build embedded in the EyeAuras scripting compiler (EyeAuras.Scripting, PoeBane).
Upstream: https://github.com/oxc-project/tsrs. Base: 66119d706aada74470984b8c1c0595179060a2d9 (v0.19.0).
Fork branch: `eyeauras/v0.19`. Consumers pin an exact commit of this branch.

## Changes against upstream

1. **Environment isolation (`tsrs_core` feature `embedded`).** Every `TSRS_*` tuning or diagnostic switch and
   `RAYON_NUM_THREADS` is read through `tsrs_core::hostenv`. With `embedded` these reads report "not present",
   so the process environment of the host cannot change compiler behavior; the host configures the compiler
   through its API. Without the feature the behavior is unchanged. Not covered on purpose: the CLI system
   object (`tsrs_execute::sys`, `tsrs_wasm::sys`: `FORCE_COLOR`, `NO_COLOR`, `TS_WATCH_DEBUG`), the OS user
   cache directory for typings (`tsrs_vfs::osvfs`), mimalloc's own `MIMALLOC_*` options, build scripts and tests.
   An embedding host uses none of them.
2. **ICU range.** `icu_normalizer` accepts `>=2.0.1, <2.4` instead of `=2.3.0`, so a host on Boa 0.21 (icu ~2.0)
   links one copy of each ICU crate. Only NFD (`tsrs_core::stringutil::norm`) uses it. With icu 2.0 the data is
   Unicode 16; NFD of characters added in Unicode 17 differs from upstream, and the parity test
   `normalization_matches_pinned_tables_for_every_scalar` fails under icu 2.0. Drop this change when the host
   moves to Boa 0.22 (icu ~2.3).

## Updating the base

Rebase this branch onto the new upstream tag through a pull request in this fork. The PR must list which changes
above still apply, and pass the EyeAuras.Scripting compiler tests with the new pin before a consumer moves to it.

## License

Apache-2.0 (LICENSE) with NOTICE, unchanged from upstream. Both ship with the product.
