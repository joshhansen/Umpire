#!/bin/sh
id=`sh scripts/id.sh`
cmd="cargo bench"
benches=""
for filename in ./benches/*.rs; do
    bench=`basename $filename .rs`
    cmd+=" --bench $bench"
    benches+=" $bench"
done
cmd+=" -- --save-baseline $id"
echo $cmd
$cmd
echo "Saved baseline $id for: $benches"
