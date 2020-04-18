#!/bin/sh
ID=$1
cargo build
set -x
perf stat -o profiling/perf/stat/umpire-ai_train.$ID target/debug/umpire-ai -e 10 -s 100 -W 10 -H 10 train -a /tmp/profiling_10x10_e100_s100_a.ai
