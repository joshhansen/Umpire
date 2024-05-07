#!/bin/bash

GAMES=$1
PROCS=$2
DEST=$3
shift; shift; shift;

PERPROC=`expr $GAMES / $PROCS`

echo "Games: $GAMES"
echo "Processes: $PROCS"
echo "Games per process: $PERPROC"
echo "Dest dir: $DEST"

set -x
seq $PROCS | parallel --lb -j $PROCS cargo run --release -p umpire-ai -- -v -e 10000 -s 4000 -W 10 -W 20 -W 30 -W 40 -H 10 -H 20 -H 30 -H 40 eval rr -P $DEST/10-40_rr_e$PERPROC_s4000_p0.0001.{}.data -p 0.0001
