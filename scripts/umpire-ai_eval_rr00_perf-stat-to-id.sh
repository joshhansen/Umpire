#!/bin/sh
ID=$1
PROFILE=release-dbg
cargo build --profile $PROFILE -p umpire-ai
set -x
perf stat record -d -o profiling/perf/stat/umpire-ai_eval_rr00.$ID target/$PROFILE/umpire-ai -e 10 -s 1000 -W 10 -W 20 -W 30 -W 40 -H 10 -H 20 -H 30 -H 40 eval -S 2938 --detsec r4343 r8989 ./ai/agz/15x15/0.agz.mpk ./ai/agz/15x15/0.agz.mpk
