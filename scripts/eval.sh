#!/bin/bash

PROCS=$1
PERPROC=$2
shift; shift;

>&2 echo "Passing args: $@"

PROFILE="release"

>&2 echo "Processes: $PROCS"
>&2 echo "Games per process: $PERPROC"

set -x

cargo build --profile=$PROFILE -p umpire-ai

# ./ai/agz/15x15/0.agz.mpk ./ai/agz/15x15/0.agz.mpk 
# -W 10 -W 20 -W 30 -W 40 -H 10 -H 20 -H 30 -H 40
exec parallel -j $PROCS --lb ./target/$PROFILE/umpire-ai -v -e $PERPROC eval -W 10 -W 20 -W 30 -W 40 -H 10 -H 20 -H 30 -H 40 $@
