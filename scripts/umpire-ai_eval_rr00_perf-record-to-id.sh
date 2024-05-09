#!/bin/sh
ID=$1
PROFILE=debug
cargo build -p umpire-ai
set -x
perf record --call-graph dwarf -o profiling/perf/record/umpire-ai_eval_rr00.$ID target/$PROFILE/umpire-ai -e 10 -s 1000 -W 10 -W 20 -W 30 -W 40 -H 10 -H 20 -H 30 -H 40 eval r r ./ai/agz/15x15/0.agz.mpk ./ai/agz/15x15/0.agz.mpk
