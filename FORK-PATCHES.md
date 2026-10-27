# Embedding patches

Upstream: https://github.com/oxc-project/tsrs.
Base: `66119d706aada74470984b8c1c0595179060a2d9` (v0.19.0).

These patches address resource ownership and configuration for long-lived embedding hosts.
Language semantics follow the pinned TypeScript reference; `alwaysStrict: false` is unsupported.
Each patch requires its own reproduction and review before submission upstream. This list does
not imply upstream acceptance or a stable embedding API.

## Environment isolation

The `tsrs_core/embedded` feature routes `TSRS_*` tuning/diagnostic switches and `RAYON_NUM_THREADS`
through `tsrs_core::hostenv`, reporting them absent. Without the feature, environment reads retain
their upstream behavior. This is not a replacement API for every environment switch.

CLI color/debug options, OS cache locations and current directory, a content mapper's `PATH`,
mimalloc options, build scripts and tests remain outside this isolation.

## Source-text lifetime

`unregister_source_text` returns its slot to a free list linked through free slots. Registration
reuses a free slot before extending the `2^20`-entry table. `source_text_slots()` exposes the
high-water mark and free count. Upstream falls back to storing identifier text when the table
fills; reuse avoids permanently exhausting the compact-text fast path.

`parse_source_file` unregisters copied text when its owning region is freed, including calls
outside the owned/embedded entrypoints and parser cache. Nothing may read an index after
unregistration. Unregister exactly once: after reuse, a second call would free another file's slot.

## Cancellation

The compiler pool replaces a checker that observed cancellation before its next acquisition.
Diagnostics loops skip remaining files once cancellation is observed. Split declaration-file
pieces are marked finished without using a cancelled checker; after a cancelled split pass,
every participating checker is marked cancelled to discard partial state.

Old checkers remain allocated until their program is freed because live diagnostics/program
state can refer into their regions. This fixes reuse panics, not unbounded cancellation memory:
a host must rebuild the program after a bounded number of cancellations. One cancelled split
pass can replace up to the pool's checker count. `canceled_checker_replacements()` reports the
process-wide replacement count.

## Worker pool configuration

`tsrs_core::workerpool::configure_worker_pool(WorkerPoolOptions { threads, stack_size })`, also
exported by `tsrs_compiler`, configures the process-wide pool before first use. Later calls return
`WorkerPoolStarted`. Defaults remain min(cores, 32) threads and 256 MiB stacks, with environment
overrides when not isolated. Threads are named `tsrs-worker-N`.

The include-glob directory walk uses this pool instead of implicitly creating rayon's global
pool. Inside another rayon pool, its scope remains there. The compiler pool and its stacks live
until process exit; the static source table and spare thread arenas also outlive individual builds.

## Parser stack containment

Token advancement/rescans query `stacker::remaining_stack`. Less than 1 MiB remaining or an
unsupported query panics with `tsrs parser stack limit exceeded`; an embedding host must catch
unwinding on its compiler thread and bound AST depth before recursive consumers. This is a
parser guard, not a compiler-wide recoverable stack-budget API.

Pending source-text registration is unregistered on unwind before a host releases its region.
Successful entrypoints transfer registration to their existing text/region lifetime owner.

## Lifetime-ID indexes

`IdLinkStore` and `InlineIdStore` use the existing `2^24` dense-key ceiling and their sparse maps
above it, instead of extending a fresh dense prefix through all earlier programs' global IDs.
Stable addresses and frozen-parent semantics are preserved. This bounds dense indexes; it does
not make allocation traffic flat below the ceiling, reset IDs or remove packed-ID lifetime limits.

## Region rollback

A speculative parse cannot rewind over a newly registered destructor. Its values and destructor
entries remain owned until region release, avoiding leaked heap payloads. Dropless speculation
and thread arenas keep their existing rewind behavior.

## Updating the base

For each upstream update, identify which patches remain necessary, reproduce their failure and
verify the changed compiler/embedding contracts. Recheck direct environment reads in library code:

```sh
rg -n "std::env::var|env::var_os|env::var\(" crates --glob "*.rs" --glob "!**/tests/**" --glob "!**/examples/**" --glob "!build.rs"
```

Apache-2.0 (`LICENSE`) and `NOTICE` are unchanged from upstream and accompany redistribution.
