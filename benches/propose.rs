#[macro_use]
extern crate criterion;

use criterion::Criterion;

use umpire::{
    game::{
        test_support::test_propose_move_unit_by_id,
    },
};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function(
        "test_propose_move_unit_by_id",
        |b| b.iter(|| test_propose_move_unit_by_id())
    );
}


criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);