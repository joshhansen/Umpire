#[macro_use]
extern crate criterion;

use common::game::player::PlayerControl;
use criterion::Criterion;

use umpire_workspace::common::game::test_support::game_two_cities_two_infantry_big;

fn criterion_benchmark(c: &mut Criterion) {
    let (mut game, secrets) = game_two_cities_two_infantry_big();

    let mut ctrl = PlayerControl::new(game, 0, secrets[0]);

    let mut ctrl = game.player_turn_control(0);
    let mut ai = RandomAI::new(0, false);
    c.bench_function("random_ai", |b| {
        b.iter(|| {
            ai.take_turn(&mut ctrl, false);
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
