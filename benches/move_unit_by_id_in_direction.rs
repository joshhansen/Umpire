#[macro_use]
extern crate criterion;

use common::util::init_rng;
use criterion::{BatchSize, Criterion};

use rand::Rng;

use umpire_workspace::common::{
    game::{
        map::{terrain::Terrain, MapData},
        unit::UnitType,
        Alignment, Game,
    },
    util::{Dims, Direction, Location, Wrap2d},
};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("move_unit_by_id_in_direction", |b| {
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

                let rng = init_rng(Some(84393483));
                let (game, secrets) =
                    Game::new_with_map(Some(rng), map, 1, true, None, Wrap2d::BOTH);

                let mut rng = init_rng(Some(21371294));
                let dir_idx = rng.gen_range(0, 8);
                let dir = Direction::values()[dir_idx];
                (game, secrets, unit_id, dir)
            },
            |(game, secrets, unit_id, dir)| {
                game.move_unit_by_id_in_direction(secrets[0], *unit_id, *dir)
                    .unwrap()
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
