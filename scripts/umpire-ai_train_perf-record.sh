#!/bin/sh
ID=$(sh scripts/id.sh)
./scripts/umpire-ai_train_perf-record-to-id.sh $ID
