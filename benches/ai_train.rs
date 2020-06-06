#[macro_use]
extern crate criterion;

use criterion::Criterion;

use umpire::{
    game::{
        ai::{
            AI,
            rl::trained_agent,
        }
    },
    util::{
        Dims, Wrap2d,
    },
};

fn criterion_benchmark(c: &mut Criterion) {
    for dims in vec![Dims::new(10, 10), Dims::new(20, 20), Dims::new(30, 30)] {
        c
        .bench_function(
            format!("ai_train_{}", dims).as_str(),
            |b| b.iter(|| {
                let _agent = trained_agent(AI::random(0, false), false, 2, vec![dims], vec![Wrap2d::BOTH], 1, 100, 0.01, 0.9, 0.05, 0.001, true, false, true, 0, None, 0.0);
            })
        );
    }
}


criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);