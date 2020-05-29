#!/bin/sh
ID=$1
cargo build
set -x
perf stat -o profiling/perf/stat/umpire-ai_train_deep.$ID target/debug/umpire-ai -v -W 10 -H 10 -e 1 -s 1000 train -a -d /tmp/profiling_10x10_e1_s1000_a.ai random
