#!/bin/sh
ID=$(sh scripts/id.sh)
scripts/umpire-ai_train_perf-stat-to-id.sh $ID
