#!/bin/bash

GAMES=$1
PROCS=$2
DEST=$3
shift; shift; shift;

echo "Passing args: $@"

PROFILE="release"

PERPROC=`expr $GAMES / $PROCS`

echo "Games: $GAMES"
echo "Processes: $PROCS"
echo "Games per process: $PERPROC"
echo "Dest dir: $DEST"

set -x

mkdir -p $DEST

cargo build --profile=$PROFILE -p umpire-ai

# ./ai/agz/15x15/0.agz.mpk ./ai/agz/15x15/0.agz.mpk 
# -W 10 -W 20 -W 30 -W 40 -H 10 -H 20 -H 30 -H 40
seq $PROCS | parallel -j $PROCS --lb ./target/$PROFILE/umpire-ai -v -e $PERPROC eval -w v -w h -w v -w n -M c -M t -M r -W 10 -W 20 -W 30 -W 40 -H 10 -H 20 -H 30 -H 40 -P $DEST/{}.data $@ rr
