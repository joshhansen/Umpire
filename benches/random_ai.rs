#[macro_use]
extern crate criterion;

use criterion::Criterion;

use umpire::{
    game::{
        ai::RandomAI,
        player::TurnTaker,
        test_support::game_two_cities_two_infantry_big,
    },
};

fn criterion_benchmark(c: &mut Criterion) {
    let mut game = game_two_cities_two_infantry_big();

    let mut ctrl = game.player_turn_control(0);
    let mut ai = RandomAI::new();
    c.bench_function(
        "random_ai",
        |b| b.iter(|| {
            ai.take_turn(&mut ctrl);
        })
    );
}


criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);