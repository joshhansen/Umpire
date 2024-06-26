#[macro_use]
extern crate criterion;

use criterion::black_box;
use criterion::Criterion;

use umpire_workspace::common::{
    game::map::{terrain::Terrain, LocationGrid, Tile},
    util::Dims,
};

fn iterate_grid(grid: &LocationGrid<Tile>) {
    // for loc in grid.dims().iter_locs() {
    //     let tile = grid.get(loc);
    //     black_box(tile);
    // }

    for tile in grid.iter() {
        black_box(tile);
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let dims = Dims::new(100, 100);
    let grid = LocationGrid::new(dims, |loc| Tile::new(Terrain::Land, loc));

    c.bench_function(format!("grid iterate tile {}", dims).as_ref(), |b| {
        b.iter(|| iterate_grid(&grid))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
