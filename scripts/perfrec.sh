#!/bin/sh
set -x
perf record -F 100 --call-graph dwarf --no-buffering $@
