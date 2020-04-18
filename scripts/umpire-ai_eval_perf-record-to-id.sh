#!/bin/sh
ID=$1
cargo build
set -x
perf record -g -o profiling/perf/record/umpire-ai_eval.$ID target/debug/umpire-ai -e 10 -s 100 eval random ai/10-30_e100_s100000_a__scorefix__turnpenalty.ai
