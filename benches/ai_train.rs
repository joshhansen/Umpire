#[macro_use]
extern crate criterion;

use criterion::Criterion;

use umpire::{
    game::{
        ai::rl::trained_agent,
    },
    util::{
        Dims,
    },
};

fn criterion_benchmark(c: &mut Criterion) {

    c
    .bench_function(
        "ai_train",
        |b| b.iter(|| {
        
        //     fn trained_agent(opponent_model_path: Option<String>, dims: Vec<Dims>, episodes: usize, steps: u64, avoid_skip: bool, verbose: bool) ->
        // UmpireAgent<Shared<Shared<LFA<Basis,SGD,VectorFunction>>>,
        //     UmpireEpsilonGreedy<Shared<LFA<Basis, SGD, VectorFunction>>>>{

            let _agent = trained_agent(None, vec![Dims::new(10, 10)], 1, 100, true, false);
            

        })
    );
}


criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);