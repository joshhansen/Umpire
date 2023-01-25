#[macro_use]
extern crate criterion;

use criterion::Criterion;

use rsrl::DerefVec;

use umpire::{
    game::Game,
    name::IntNamer,
    util::{Dims, Wrap2d},
};

fn criterion_benchmark(c: &mut Criterion) {
    let city_namer = IntNamer::new("city");
    let game = Game::new(Dims::new(190, 80), city_namer, 4, false, None, Wrap2d::BOTH);

    c.bench_function("deref_vec", |b| {
        b.iter(|| {
            game.deref_vec();
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
