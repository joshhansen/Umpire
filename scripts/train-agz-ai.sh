#!/bin/sh
set -x

BASE_ID=$(sh scripts/id.sh)

WIDTH=10
HEIGHT=10

ID="${WIDTH}x${HEIGHT}_${BASE_ID}"

AI="cargo run --release -p umpire-ai --"

# Generate self-play data

$AI -e 100000 -s 1000 -W $WIDTH -H $HEIGHT -F -v eval rr -P ai/data/$ID.data -p 0.001

# Train AlphaGo Zero-style action-state model
$AI -e 10 -W $WIDTH -H $HEIGHT -v agztrain -D 0.00001 -o ai/agz/$ID.agz ai/data/$ID.data

# Evaluate the new model against a random model
$AI -e 100 -F -vv -s 1000 -W $WIDTH -H $HEIGHT eval r ./ai/agz/$ID.agz/model.mpk
