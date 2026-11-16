#!/bin/sh
# Generate pgo/gull.profdata and rebuild with it.  PGO is optional; a normal
# `cargo build --release` does not depend on a generated profile.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

profdir="$root/target/pgo-data"
rm -rf "$profdir"
mkdir -p "$profdir" "$root/pgo"

# Build an instrumented binary without using any old profile.
RUSTFLAGS="-C target-cpu=native -C profile-generate=$profdir" \
    cargo build --release

# Representative searches covering the opening, middlegame, tactics and
# endgames.  These runs only collect profile counters.
./target/release/gull bench 16 startpos >/dev/null
./target/release/gull bench 13 \
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1" >/dev/null
./target/release/gull bench 13 \
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1" >/dev/null
./target/release/gull bench 13 \
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1" >/dev/null
./target/release/gull bench 12 \
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10" >/dev/null
./target/release/gull bench 12 \
    "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4" >/dev/null
./target/release/gull bench 12 \
    "rnbqkb1r/pp3ppp/3p1n2/2pPp3/2P5/2N2N2/PP2PPPP/R1BQKB1R w KQkq - 0 6" >/dev/null
./target/release/gull bench 12 \
    "2r3k1/pp3pp1/4p2p/3n4/3P4/2NB1P2/PP3P1P/2R3K1 w - - 0 1" >/dev/null

sysroot=$(rustc --print sysroot)
profdata=$(find "$sysroot" -type f -name llvm-profdata 2>/dev/null | head -n 1)
if [ -z "$profdata" ]; then
    echo "llvm-profdata not found in the Rust toolchain" >&2
    exit 1
fi
"$profdata" merge -o "$root/pgo/gull.profdata" "$profdir"/*.profraw

# Force a rebuild so the newly generated profile is consumed.
touch src/lib.rs
RUSTFLAGS="-C target-cpu=native -C profile-use=$root/pgo/gull.profdata" \
    cargo build --release
echo "PGO profile written to pgo/gull.profdata"
