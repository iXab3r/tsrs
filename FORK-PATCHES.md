# Fork patches

Upstream: https://github.com/oxc-project/tsrs.
Base: `66119d706aada74470984b8c1c0595179060a2d9` (workspace version 0.19.0).
Branches: `main` = base + the changes below; `upstream` = mirror of oxc-project/tsrs `main`.
Full diff: [`66119d70...main`](https://github.com/iXab3r/tsrs/compare/66119d706aada74470984b8c1c0595179060a2d9...main).
The history of `main` also contains earlier fork changes that were later reverted; only the diff counts.

Only the three changes below remain. Language semantics follow the Go reference pinned in Cargo.toml;
`alwaysStrict: false` is unsupported. Their presence here does not imply upstream acceptance or a
stable embedding API. Keep the summary list in README.md in sync with this file.

| Change | Commits | Upstream status |
|---|---|---|
| [Region rollback](#region-rollback) | `0d30031d` (arena/ptr part) | Reported upstream as [oxc-project/tsrs#863](https://github.com/oxc-project/tsrs/issues/863) (upstream does not accept external PRs). Draft: closed fork PR [#5](https://github.com/iXab3r/tsrs/pull/5) |
| [Lifetime-ID indexes](#lifetime-id-indexes) | `0d30031d` (links part) | Fork only for now; candidate after an analysis of Go ID lifetime. Draft: closed fork PR [#6](https://github.com/iXab3r/tsrs/pull/6) |
| [Parser stack containment](#parser-stack-containment) | `9fd23966`, `616f25dd`, `8eaaba81` | Fork only: not a port of Go behavior, not upstream-ready. Draft: closed fork PR [#7](https://github.com/iXab3r/tsrs/pull/7) |

GitHub Actions is disabled in this fork; evidence comes from local runs. Last local run, 2026-11-01,
tree of `43954a88`, Rust 1.99.1, Windows: `cargo test -p tsrs_core --lib arena` (8 passed, 1 ignored),
`cargo test -p tsrs_checker --lib links` (14 passed), `cargo test -p tsrs_parser` (7 passed).

## Region rollback

A speculative parse cannot rewind over a newly registered destructor. Its values and destructor
entries remain owned until region release, avoiding leaked heap payloads. Dropless speculation
and thread arenas keep their existing rewind behavior. Go allocation and garbage collection
ownership must be compared with this Rust-specific destructor requirement.

- Why: rewinding over a registered destructor leaked the owned heap payloads of speculative parses,
  64 bytes per TSX compilation in a long-lived host.
- What: `crates/tsrs_core/src/arena.rs` (`rewindable`/checkpoint also compare the destructor count),
  `crates/tsrs_core/src/ptr.rs` (docs). Arena unit tests moved to `crates/tsrs_core/tests/support/arena.rs`.
- Verified: release leak probe and Go comparison below; arena unit tests. On upstream `main` `e18fb617`
  (2026-11-01) the leak reproduces when a host parses into its own region: a `Diagnostic` reported during a
  rewound speculation loses its drop entry, 32 bytes per parse of `let r = (x = <span>hi</span>) => x;` as `.ts`.
  The CLI parses into the thread arena and is not affected. Reproduction and fix are in #863.

## Lifetime-ID indexes

`IdLinkStore` and `InlineIdStore` use the existing `2^24` dense-key ceiling and their sparse maps
above it, instead of extending a fresh dense prefix through earlier programs' global IDs.
Stable addresses and frozen-parent semantics are preserved. This bounds dense indexes; it does
not make allocation traffic flat below the ceiling, reset IDs or remove packed-ID lifetime limits.
Boundary correctness and targeted cost after crossing the threshold are covered below.
Go ID lifetime and link storage must be analyzed before proposing this optimization upstream.

- Why: IDs survive individual programs, so each fresh checker in a long-lived process grew dense
  indexes through the IDs of earlier programs (2.5 MB at `2^24`, 10 MB at `2^26`).
- What: `crates/tsrs_checker/src/links.rs` (`dense_store_id` routes IDs at or above `2^24` to the
  sparse maps). Link-store unit tests moved to `crates/tsrs_checker/tests/support/link_stores.rs`.
- Verified: targeted allocation probe and Go comparison below; link-store unit tests.

## Parser stack containment

Token advancement/rescans query `stacker::remaining_stack`. Where bounds are available, less than
1 MiB remaining panics with `tsrs parser stack limit exceeded`; an embedding host must catch
unwinding on its compiler thread and bound AST depth before recursive consumers. This is a
parser guard, not a compiler-wide recoverable stack-budget API or a literal port of Go's growing
goroutine stacks. An unavailable query preserves upstream parsing rather than rejecting every input;
this guard provides no stack containment on that backend. The threshold still needs upstream-quality review.

Pending source-text registration is unregistered on unwind before a host releases its region.
Successful entrypoints retain upstream's text/region ownership. Region-owning consumers use
the existing owned or embedded parse entrypoints.

- Why: deeply nested input overflowed the native stack and aborted the embedding process.
- What: `crates/tsrs_parser/src/parser_1.rs`, new `stacker` dependency in `crates/tsrs_parser/Cargo.toml`
  (and `Cargo.lock`); `tools/lint/unsafe_sites.tsv` follows the moved tests.
- Verified: deep-input runs, the WASI unsupported-query run and paired parser timings below.

## Comparison with the pinned Go implementation

Reference: microsoft/TypeScript `3e9236361fa97a9e6c685f4833ac457ae2d8c50a`, as pinned in
Cargo.toml. The reference checkout's HEAD and unmodified `tsc/` tree were verified before building
its Go compiler. Both compilers reported TypeScript 7.1.0-beta. Public TS/TSX cases used identical
files, working directory, `--singleThreaded`, target, JSX, declaration and source-map options.

### Region rollback is a Rust ownership correction

Go's [parser mark/rewind and lookAhead](https://github.com/microsoft/TypeScript/blob/3e9236361fa97a9e6c685f4833ac457ae2d8c50a/tsc/internal/parser/parser.go#L350)
restore scanner state and diagnostic/list lengths. Its
[Arena](https://github.com/microsoft/TypeScript/blob/3e9236361fa97a9e6c685f4833ac457ae2d8c50a/tsc/internal/core/arena.go)
holds Go slices; unreachable payloads remain visible to garbage collection. There is no Rust-style
destructor ledger to truncate. The Rust optimization must retain every registered destructor until
its region is freed, without changing the parser's decisions or invoking destructors prematurely.

In a controlled Windows release run, only the old arena rewind behavior was restored. After warmup,
200 repeated TSX compilations retained 12,800 requested heap bytes; the corrected variant retained
zero at checkpoints 20, 100 and 200. The reproduced slope is 64 bytes per compile. This measures
requested live Rust heap bytes after outputs/regions are dropped, not OS residency. Public TSX
diagnostics, JavaScript, declarations and source maps matched the pinned Go compiler exactly.

### Go already bounds paged links and keeps ordinary IDs in maps

Go's [atomic ID counters and block generators](https://github.com/microsoft/TypeScript/blob/3e9236361fa97a9e6c685f4833ac457ae2d8c50a/tsc/internal/ast/utilities.go#L17)
also survive individual programs. Its [node/symbol link stores](https://github.com/microsoft/TypeScript/blob/3e9236361fa97a9e6c685f4833ac457ae2d8c50a/tsc/internal/checker/links.go)
route only the reserved block-ID range below `maxPageLinkCount = 0x100_0000` to
[PagedLinkStore](https://github.com/microsoft/TypeScript/blob/3e9236361fa97a9e6c685f4833ac457ae2d8c50a/tsc/internal/core/linkstore.go#L46).
Ordinary and out-of-range IDs use the map-backed LinkStore. Rust uses different compressed-ID and
small-group layouts; bounding its dense prefix preserves that resource property without changing IDs.

The targeted Windows release probe held 512 active keys in fresh real stores, with equal checksums
and region lifetimes. At ID `2^24`, restoring only old dense routing allocated 2,523,192 requested
heap bytes versus 102,584 with sparse routing. At ID `2^26`, it allocated 9,994,296 versus 102,584.
Keys below the cutoff retain the dense route. Sparse hit/miss lookups cost more than dense lookups;
these microbenchmarks establish the memory bound and its tradeoff, not an end-to-end speedup.
Public generic/type-error cases matched Go's diagnostic text/order, exit code 2 and all emit files.

### Stack containment is not a port of Go stack growth

Go's [prefix-unary parser](https://github.com/microsoft/TypeScript/blob/3e9236361fa97a9e6c685f4833ac457ae2d8c50a/tsc/internal/parser/parser.go#L5170)
recurses without this remaining-stack check. Its runtime grows goroutine stacks; those stacks also
have a finite fatal limit. On the same valid input with 400,000 unary operators, pinned Go completed
checking/emit while the native Rust CLI overflowed its stack. At one million operators both processes
overflowed. The parser guard therefore does not establish compiler-wide stack containment or deep-input
Go parity: later recursive passes need their own resource policy.

With an explicit 8 MiB native thread reserve, the guarded parser rejected the 400,000-operator input
by unwinding, then parsed a normal source successfully on that thread. A host must catch that panic
and bound AST depth before recursive checking/emit. The 1 MiB margin is a conservative internal
headroom choice, not a TypeScript language limit. Do not report this patch as upstream-ready parity.

The unsupported-query correction was executed under WASI: `remaining_stack()` returned `None` and
an ordinary source parsed with one statement and no diagnostics. The previous `is_some_and` check
would reject that input before token scanning. Unsupported backends retain ordinary parsing, with
no promised stack-containment guarantee.

Six interleaved release pairs used the same generated ordinary TypeScript parser workload, with
100 parses per batch. Paired samples varied in both directions; a small overhead is not ruled out.
Both builds used the same host release profile (`opt-level=s`) and counting allocator. This is not
a zero-cost or speedup claim. Builds, source generation, checker work and emit were outside those batches.

These focused Windows measurements and oracle comparisons support internal resource corrections.
Upstream performance readiness additionally requires its release catalog, retired-instruction and
peak-RSS evidence on an appropriate runner; current working set is not a substitute for peak RSS.

## Upstream behavior retained

Source-text slots are not reused. Once the fixed table fills, upstream stores identifier text
in each identifier. Programs are discarded after cancellation rather than renewing cancelled
checkers inside a program. Worker-pool ownership and environment reads follow upstream.
Consumers can select bundled library paths explicitly; no blanket environment-isolation feature
is provided. This does not guarantee independence from every compiler environment switch.

Apache-2.0 (`LICENSE`) and `NOTICE` are unchanged from upstream and accompany redistribution.

## Updating from upstream

This procedure is the same in the tsrs and Boa forks; only the checks differ.

`upstream` mirrors the upstream default branch and never receives our commits. `main` is the
upstream base plus the changes listed in this file. Consumers pin `main` commits, so the history
of `main` only grows: never rebase, squash or force-push it.

1. Refresh the mirror: `git fetch upstream --tags`, then fast-forward the fork's `upstream` branch
   (`git push origin upstream/main:refs/heads/upstream`). If that is not a fast-forward, stop and ask:
   upstream rewrote its history.
2. Pick the target: the newest upstream release tag, or an upstream `main` commit when a needed fix
   is not tagged yet.
3. Branch from `main` (`sync/upstream-<target>`) and `git merge <target>`. Resolve conflicts inside
   our changes; elsewhere keep the upstream code.
4. Review every change in this file. If upstream now contains it (our accepted upstream PR or an
   equivalent fix), drop our version in the merge. Otherwise confirm the reason still holds and the
   change still fits the new code.
5. Check new upstream behavior that matters to an embedding host (see the checks below).
6. Run the checks locally. GitHub Actions is disabled in this fork; put the commands and results in
   the PR description.
7. Update the base, the change list in README.md and the status table in this file.
8. Open a PR into `main` and merge it with a merge commit.
9. Re-pin consumers in a separate change in each consumer repository, with its own tests and
   `--locked` builds. Old pins stay reachable from `main`.

Cadence: when an upstream release contains fixes we need, or monthly, whichever comes first.
Syncing upstream is a routine operation. Adding or changing our own patches needs the owner's
explicit approval in the PR.

Checks for tsrs (toolchain from `rust-toolchain.toml`):

- `cargo test --locked`
- `cargo clippy --all-targets --config 'build.warnings="deny"'`
- `tools/lint/source.mjs`
- New environment reads or process-wide state in library crates since the previous base, for example
  `git diff <old-base> <target> -- crates | grep -E 'env::var|var_os'`.
- Consumer re-pin: the compiler tests pass, the build is `--locked`, and `cargo tree -d` shows one copy
  of the ICU crates.
