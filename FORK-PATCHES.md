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
Boundary correctness is covered; cost after crossing the threshold still needs measurement.
Go ID lifetime and link storage must be analyzed before proposing this optimization upstream.

## Parser stack containment

Token advancement/rescans query `stacker::remaining_stack`. Less than 1 MiB remaining or an
unsupported query panics with `tsrs parser stack limit exceeded`; an embedding host must catch
unwinding on its compiler thread and bound AST depth before recursive consumers. This is a
parser guard, not a compiler-wide recoverable stack-budget API or a literal port of Go's growing
goroutine stacks. The threshold and unsupported-platform behavior need upstream-quality review.

Pending source-text registration is unregistered on unwind before a host releases its region.
Successful entrypoints retain upstream's text/region ownership. Region-owning consumers use
the existing owned or embedded parse entrypoints.

## Upstream behavior retained

Source-text slots are not reused. Once the fixed table fills, upstream stores identifier text
in each identifier. Programs are discarded after cancellation rather than renewing cancelled
checkers inside a program. Worker-pool ownership and environment reads follow upstream.
Consumers can select bundled library paths explicitly; no blanket environment-isolation feature
is provided. This does not guarantee independence from every compiler environment switch.

Apache-2.0 (`LICENSE`) and `NOTICE` are unchanged from upstream and accompany redistribution.
