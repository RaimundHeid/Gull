# Gull (Rust port)

A Rust port of the [Gull 3](https://github.com/...) chess engine.  The port is
deliberately split into three modules:

| Module | Responsibility |
| --- | --- |
| `src/evaluation.rs` | The hand-tuned evaluation function, PSTs, pawn structure, material and king safety. |
| `src/search.rs` | Alpha/beta search, quiescence search, PVS, null move, IID, singular extensions, SEE, move ordering, transposition/PV hash tables and iterative deepening. |
| `src/communication.rs` | The UCI protocol front end. |

Supporting modules:

* `src/types.rs` — board types, move encoding, score helpers.
* `src/tables.rs` — zobrist keys, magic bitboards, PSTs, KPK tables and all
  static evaluation data.
* `src/position.rs` — board state, FEN parsing, make/unmake, engine state and
  the search entry point.

## Scope

This version is:

* **Lazy SMP multithreading**, configurable from 1 to 256 threads (`Threads`);
* **Syzygy tablebase support** using the pure-Rust `pyrrhic-rs` probe library
  (vendored in `vendor/`, no C/C++ code);
* no network access or external build tools required when building normally.

## Building and running

```sh
cargo build --release
./target/release/gull          # UCI mode
./target/release/gull bench 6 startpos
./target/release/gull bench 8 "r3k2r/... w KQkq - 0 1"
```

The `bench` command uses an 8 MB hash table and prints an
`info`/`bestmove` summary.

Example UCI session:

```
uci
isready
position startpos moves e2e4 e7e5
go depth 8
quit
```

`stop`, `ponderhit`, `ucinewgame`, `setoption name Hash value N`,
`setoption name Threads value N`, `setoption name SyzygyPath value <path>`,
`setoption name SyzygyProbeDepth value N` and
`position startpos|fen ... moves ...` are supported.  Threads share the
transposition, PV and tablebase handles; each worker keeps private position
state, history, refutation and pawn-hash tables.  Syzygy paths are
colon-separated directories.  Positions with castling rights are not probed,
matching Syzygy tablebase semantics.

## Tests

```sh
cargo test --release
```

The test suite includes:

* `tests/evaluation_reference.rs` — 50 positions whose score, PST, material,
  pawn-structure terms and zobrist keys are captured in reference fixtures.
* `tests/search_reference.rs` — 46 positions/depths (including startpos at
  depths 1–8) whose best move, score and node count are captured in reference
  fixtures.  These are exact-search regression tests.
* `tests/communication.rs` — end-to-end UCI process tests, including
  multithreaded start/stop.
* `tests/smp.rs` — Lazy SMP search, shared-TT visibility/concurrency,
  start/stop and 256-thread configuration tests.
* `tests/tablebase.rs` — pure-Rust adapter/attack-table checks, KQvK WDL/DTZ
  root probing, path management and tablebase sharing across workers.
* `tests/position_setup.rs` — applying UCI moves and searching from the
  resulting root.
* Unit tests for move generation, SEE, make/unmake, hashing, evaluation and
  UCI parsing.


## Profile-guided optimization

PGO is optional and is not applied automatically.  On a POSIX host, generate a
local profile with:

```sh
./scripts/build-pgo.sh
```

The script builds an instrumented binary, runs representative searches, merges
the profile into `pgo/gull.profdata`, and rebuilds with `-Cprofile-use`.
Equivalent manual invocation:

```sh
RUSTFLAGS="-C target-cpu=native -C profile-use=pgo/gull.profdata" \
    cargo build --release
```

The profile is platform-specific and intentionally not committed.  A normal
`cargo build --release` does not require it.

## Portability

The crate is Rust-only and builds on Linux, macOS and Windows.  Dependencies
are vendored pure-Rust crates, so no C/C++ compiler is required.  Syzygy paths
are separated with `:` on Unix and `;` on Windows.  The optional PGO helper is a
POSIX shell script; Windows builds use the normal release profile.

## FEN/move conventions

Moves use Gull's internal 16-bit encoding.  `make_move(from, to)`
creates a quiet move; promotion, en-passant and castling flags are added by the
move generator or by the UCI parser.  `move_to_string` renders standard UCI
long algebraic notation.

## License

The original Gull 3 source is public domain.  New Rust code is released under
the MIT License; see `LICENSE`.  Vendored dependencies retain their own
licenses: `pyrrhic-rs` is MIT, `memmap2` is MIT/Apache-2.0 and `libc` is
MIT/Apache-2.0.
