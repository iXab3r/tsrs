# Resource ownership patches

Upstream: https://github.com/oxc-project/tsrs.
Base: `66119d706aada74470984b8c1c0595179060a2d9` (v0.19.0).

Only the three changes below remain. Each requires an independent reproduction,
comparison with the exact Go reference pinned in Cargo.toml, and review before upstream submission.
Their presence here does not imply upstream acceptance or a stable embedding API.
Language semantics follow the pinned reference; `alwaysStrict: false` is unsupported.

## Region rollback

A speculative parse cannot rewind over a newly registered destructor. Its values and destructor
entries remain owned until region release, avoiding leaked heap payloads. Dropless speculation
and thread arenas keep their existing rewind behavior. Go allocation and garbage collection
ownership must be compared with this Rust-specific destructor requirement.

## Lifetime-ID indexes

`IdLinkStore` and `InlineIdStore` use the existing `2^24` dense-key ceiling and their sparse maps
above it, instead of extending a fresh dense prefix through earlier programs' global IDs.
Stable addresses and frozen-parent semantics are preserved. This bounds dense indexes; it does
not make allocation traffic flat below the ceiling, reset IDs or remove packed-ID lifetime limits.
Boundary correctness and targeted cost after crossing the threshold are covered below.
Go ID lifetime and link storage must be analyzed before proposing this optimization upstream.

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
