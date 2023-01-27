#[macro_use]
extern crate criterion;

use criterion::{BatchSize, Criterion};

use common::{
    game::{
        map::{terrain::Terrain, MapData},
        unit::UnitType,
        Alignment, Game,
    },
    util::{Dims, Location, Vec2d, Wrap2d},
};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("move_unit_by_id", |b| {
        b.iter_batched_ref(
            || {
                let mut map = MapData::new(Dims::new(180, 90), |_| Terrain::Water);
                let unit_id = map
                    .new_unit(
                        Location::new(0, 0),
                        UnitType::Fighter,
                        Alignment::Belligerent { player: 0 },
                        "Han Solo",
                    )
                    .unwrap();

                let game = Game::new_with_map(map, 1, true, None, Wrap2d::BOTH);

                let unit_loc = game.current_player_unit_by_id(unit_id).unwrap().loc;
                let dest = game
                    .wrapping()
                    .wrapped_add(game.dims(), unit_loc, Vec2d::new(5, 5))
                    .unwrap();
                (game, unit_id, dest)
            },
            |(game, unit_id, dest)| game.move_unit_by_id(*unit_id, *dest).unwrap(),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
