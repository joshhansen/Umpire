#[macro_use]
extern crate criterion;

use criterion::Criterion;
use criterion::black_box;

use umpire::{
    game::{
        map::{
            LocationGrid,
            Tile,
            terrain::Terrain,
        },
        unit::orders::test_support::test_explore,
    },
    
    util::{Dims},
};

fn criterion_benchmark(c: &mut Criterion) {
    let dims = Dims::new(10, 10);

    c.bench_function(
        format!("explore {}", dims).as_ref(),
        |b| b.iter(|| test_explore(dims))
    );
}


criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);