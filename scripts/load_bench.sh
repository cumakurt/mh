#!/usr/bin/env bash
set -euo pipefail

SIZE="${MH_BENCH_SEARCH_SIZE:-1000000}"
echo "Running search load benchmark with ${SIZE} rows..."
MH_BENCH_SEARCH_SIZE="$SIZE" cargo bench --bench search_bench
