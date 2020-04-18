#!/bin/sh
ID=$(sh scripts/id.sh)
./scripts/umpire-ai_eval_perf-record.sh $ID
