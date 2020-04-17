#!/bin/sh
echo "Comparing to baseline: $1"
cmd="cargo bench"
benches=""
for filename in ./benches/*.rs; do
    bench=`basename $filename .rs`
    cmd+=" --bench $bench"
    benches+=" $bench"
done
cmd+=" -- -b $1"
echo $cmd
$cmd
echo "Compared to baseline $1 for: $benches"
