#!/bin/sh
ID=$(sh scripts/id.sh)
./umpire-ai_eval_perf-record.sh $ID
