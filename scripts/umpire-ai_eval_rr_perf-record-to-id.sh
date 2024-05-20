#!/bin/sh
ID=$1
PROFILE=release-dbg
cargo build --profile $PROFILE -p umpire-ai
set -x
perf record --call-graph dwarf,65528 -o profiling/perf/record/umpire-ai_eval_rr.$ID target/$PROFILE/umpire-ai -e 10 -s 1000 -W 10 -W 20 -W 30 -W 40 -H 10 -H 20 -H 30 -H 40 eval rr
