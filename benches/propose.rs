#[macro_use]
extern crate criterion;

use criterion::Criterion;

use umpire::{
    game::{
        test_support::game_two_cities_two_infantry,
        unit::UnitID,
    },
    util::Location,
};

fn criterion_benchmark(c: &mut Criterion) {
    // let src = Location{x:0, y:0};
    let dest = Location{x:1, y:0};

    let game = game_two_cities_two_infantry();

    let unit_id: UnitID = game.unit_orders_requests().next().unwrap();

    // {
    //     let unit = game.current_player_unit_by_id(unit_id).unwrap();
    //     assert_eq!(unit.loc, src);
    // }

    // let proposed_move = game.propose_move_unit_by_id(unit_id, dest).unwrap();
    c.bench_function(
        "test_propose_move_unit_by_id",
        |b| b.iter(|| {
            game.propose_move_unit_by_id(unit_id, dest).delta.unwrap()
        })
    );
}


criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);