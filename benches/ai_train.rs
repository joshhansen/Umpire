#[macro_use]
extern crate criterion;

use criterion::Criterion;

use umpire::{
    game::{
        ai::{
            AISpec,
            rl::trained_agent,
        }
    },
    util::{
        Dims,
    },
};

fn criterion_benchmark(c: &mut Criterion) {
    for dims in vec![Dims::new(10, 10), Dims::new(20, 20), Dims::new(30, 30)] {
        c
        .bench_function(
            format!("ai_train_{}", dims).as_str(),
            |b| b.iter(|| {
                let _agent = trained_agent(false, vec![AISpec::Random], vec![dims], 1, 100, true, false, true, 0);
            })
        );
    }
}


criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);