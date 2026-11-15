#!/bin/sh
# Cargo rustc wrapper: optionally apply a local PGO profile for release builds.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
profile="$script_dir/../pgo/gull.profdata"
rustc=$1
shift

# The profile must only be applied to optimized builds, and never while the
# profile itself is being generated.
if [ "${GULL_PGO_GENERATE:-0}" != "1" ] && [ -f "$profile" ]; then
    release=0
    for arg in "$@"; do
        case "$arg" in
            opt-level=3|-Copt-level=3) release=1 ;;
        esac
    done
    if [ "$release" -eq 1 ]; then
        exec "$rustc" -C "profile-use=$profile" "$@"
    fi
fi

exec "$rustc" "$@"
