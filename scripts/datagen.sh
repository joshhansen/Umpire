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

mkdir -p $DEST

seq $PROCS | parallel --lb -j $PROCS cargo run --release -p umpire-ai -- -v -e $PERPROC -s 4000 -W 10 -W 20 -W 30 -W 40 -H 10 -H 20 -H 30 -H 40 eval r r ./ai/agz/15x15/0.agz.mpk ./ai/agz/15x15/0.agz.mpk -P $DEST/10-40_rr00_e${PERPROC}_s4000_p0.0001.{}.data -p 0.0001
