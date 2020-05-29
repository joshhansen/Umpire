#!/bin/sh
ID=$(sh scripts/id.sh)
scripts/umpire-ai_train_deep_perf-stat-to-id.sh $ID
