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
2. **Long-lived host support** (POE-1299). Found by the EyeAuras memory spike (EyeAuras.Scripting
   `docs/tsrs-long-lived-memory.md`); unconditional, because each replaces a dead end (exhaustion, panic) or an
   unconfigurable default:
   - **Source-text slot reuse** (`tsrs_ast` identifier.rs). `unregister_source_text` returns the slot to a free list
     that `register_source_text` takes from first, so a host that frees its files (regions) never fills the 2^20-slot
     table (before: one slot per parsed file version, never reused). The list is linked through the free slots and
     allocates nothing. `source_text_slots()` reports high-water mark and free slots. Nothing may read an index after
     unregistering it (unchanged rule; the file's identifiers and flow nodes are freed with its text).
     `tsrs_parser::parse_source_file` itself returns the slot when the region it copied the text into is freed
     (before, only the `_owned`/`_embedded` variants and the language server's parse cache did; a tsconfig parsed
     in a build region kept its slot forever, one per one-shot build).
     Unregister exactly once per registration: after the slot is reused, a second call would free another file's
     slot (debug builds assert on a second call before reuse).
   - **Cancellation without rebuild** (`tsrs_compiler` checkerpool.rs, program.rs). A checker of the built-in pool
     that saw a cancelled context is replaced by a fresh one the next time it is acquired; the old one is kept until
     the pool is dropped with its program, since diagnostics already handed out may live in its region. The
     per-file diagnostics loops stop taking checkers once the context is cancelled. Before, the next file panicked
     with "Checker was previously cancelled" (`check_not_canceled`), and a host had to rebuild the whole program
     family. Go's compiler pool has no cancellation; `tsrs_project`'s pool already disposed such checkers.
     **Bound:** each replaced checker stays allocated until its program is freed (freeing it earlier read freed
     memory in a test), so a host rebuilds the program family after a bounded number of cancellations on one
     program. `canceled_checker_replacements()` counts replacements. Measured cost per replaced checker:
     see EyeAuras.Scripting `docs/tsrs-long-lived-memory.md`.
   - **Worker pool from code** (`tsrs_core::workerpool`, re-exported by `tsrs_compiler`). The rayon pool moved from
     `tsrs_compiler::program` to `tsrs_core`; `configure_worker_pool(WorkerPoolOptions { threads, stack_size })`
     sets it before first use (`Err(WorkerPoolStarted)` afterwards). Threads are named `tsrs-worker-N`. Defaults
     are unchanged (`RAYON_NUM_THREADS` via hostenv, else min(cores, 32); 256 MiB stacks). The include-glob directory
     walk (`tsrs_vfs::vfsmatch`) now runs on this pool (`tsrs_core::workerpool::scope`) instead of rayon's implicit
     global pool, so no compiler work starts the global pool.

Side effect for the host: tsrs pins `unicode-id-start = "=1.1.2"` (Unicode 15.1 identifier tables), so the host
lock resolves OXC, which accepts `^1`, to the same version until OXC is removed.

Known environment reads left in library code: the OS file system reads `LocalAppData`/`HOME`/`XDG_CACHE_HOME` and
the current directory; a configured content mapper reads `PATH`.

Process-wide state an embedding host cannot release (relevant to a per-service heap): the worker pool (threads,
stacks, rayon registry) lives until the process exits and should be built before such a heap is entered; the
source-text table is a static array; arenas of exited threads go to a static spare pool (`tsrs_core::ptr`).

## Updating the base

Rebase this branch onto the new upstream tag through a pull request in this fork. The PR must list which changes
above still apply, and pass the EyeAuras.Scripting compiler tests with the new pin before a consumer moves to it.
Before opening it, check that upstream added no direct reads in library code; every hit must be a test, example,
build script or CLI crate:

```
rg -n "std::env::var|env::var_os|env::var\(" crates --glob "*.rs" --glob "!**/tests/**" --glob "!**/examples/**" --glob "!build.rs"
```

## License

Apache-2.0 (LICENSE) with NOTICE, unchanged from upstream. Both ship with the product.
