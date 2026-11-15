#!/bin/sh
# Generate pgo/gull.profdata.  Once present, `cargo build --release` uses it
# automatically through .cargo/rustc-wrapper.sh.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

profdir="$root/target/pgo-data"
rm -rf "$profdir"
mkdir -p "$profdir" "$root/pgo"

# Do not let an existing profile influence the instrumented build.
GULL_PGO_GENERATE=1 RUSTFLAGS="-C target-cpu=native -C profile-generate=$profdir" \
    cargo build --release

# A few representative searches are enough to guide branch layout/inlining.
./target/release/gull bench 16 startpos >/dev/null
./target/release/gull bench 13 \
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1" >/dev/null
./target/release/gull bench 13 \
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1" >/dev/null

sysroot=$(rustc --print sysroot)
profdata=$(find "$sysroot" -type f -name llvm-profdata 2>/dev/null | head -n 1)
if [ -z "$profdata" ]; then
    echo "llvm-profdata not found in the Rust toolchain" >&2
    exit 1
fi
"$profdata" merge -o "$root/pgo/gull.profdata" "$profdir"/*.profraw

# Force a rebuild so the newly generated profile is consumed.
touch src/lib.rs
cargo build --release
echo "PGO profile written to pgo/gull.profdata"
