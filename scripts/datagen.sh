#!/bin/bash

GAMES=$1
PROCS=$2
DEST=$3
shift; shift; shift;

PROFILE="debug"

PERPROC=`expr $GAMES / $PROCS`

echo "Games: $GAMES"
echo "Processes: $PROCS"
echo "Games per process: $PERPROC"
echo "Dest dir: $DEST"

set -x

mkdir -p $DEST

# ./ai/agz/15x15/0.agz.mpk ./ai/agz/15x15/0.agz.mpk 
# -W 10 -W 20 -W 30 -W 40 -H 10 -H 20 -H 30 -H 40
seq $PROCS | parallel -j $PROCS --lb -n0 cargo run -p umpire-ai -- -v -e $PERPROC -s 4000 -W 90 -H 45 eval -p 0.0001 rr
