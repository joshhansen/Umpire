#[macro_use]
extern crate criterion;

use criterion::black_box;
use criterion::Criterion;

use umpire_workspace::common::{
    game::{
        map::{
            dijkstra::nearest_adjacent_unobserved_reachable_without_attacking, terrain::Terrain,
            LocationGrid, Tile,
        },
        obs::Obs,
        unit::{Unit, UnitID, UnitType},
        Alignment,
    },
    util::{Dims, Location, Wrap2d},
};

fn bench_nearest_adjacent_unobserved_reachable_without_attacking(
    dims: Dims,
    src: Location,
    wrapping: Wrap2d,
) {
    let dest = Location::new(dims.width - 1, dims.height - 1);

    let grid = LocationGrid::new(dims, |loc| {
        if loc == dest {
            Obs::Unobserved
        } else {
            Obs::Observed {
                tile: Tile::new(Terrain::Land, loc),
                turn: 0,
                current: false,
                action_count: 0,
            }
        }
    });

    let unit = Unit::new(
        UnitID::new(0),
        src,
        UnitType::Infantry,
        Alignment::Belligerent { player: 0 },
        "Juan de Fuca",
    );
    let nearest =
        nearest_adjacent_unobserved_reachable_without_attacking(&grid, src, &unit, wrapping);
    black_box(nearest);
}

fn criterion_benchmark(c: &mut Criterion) {
    for src in [Location::new(0, 0), Location::new(4, 4)].iter() {
        for dims in [Dims::new(10, 10), Dims::new(11, 11)].iter() {
            for (wrap_name, wrapping) in [
                ("both", Wrap2d::BOTH),
                ("horiz", Wrap2d::HORIZ),
                ("vert", Wrap2d::VERT),
                ("neither", Wrap2d::NEITHER),
            ]
            .iter()
            {
                c.bench_function(
                    format!(
                        "nearest_adjacent_unobserved {} src={} {}",
                        dims, src, wrap_name
                    )
                    .as_ref(),
                    |b| {
                        b.iter(|| {
                            bench_nearest_adjacent_unobserved_reachable_without_attacking(
                                *dims, *src, *wrapping,
                            )
                        })
                    },
                );
            }
        }
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
