#[macro_use]
extern crate criterion;

use criterion::Criterion;
use criterion::black_box;

use umpire::{
    util::{Dims,Vec2d,Wrap,Wrap2d},
};

const DIM_WIDTH: u16 = 100;
const DIMS: Dims = Dims::new(100, 100);

fn criterion_benchmark(c: &mut Criterion) {
    for wrap in &[Wrap::Wrapping, Wrap::NonWrapping] {
        for coord in (0..DIM_WIDTH).step_by(25) {
            for inc in &[-30, -1, 1, 30] {
                c.bench_function(
                    format!("{:?}.wrapped_add({}, {}, {})", wrap, DIM_WIDTH, coord, inc).as_ref(),
                    |b| b.iter(|| black_box(wrap.wrapped_add(black_box(DIM_WIDTH), coord, *inc))),
                );
            }
        }
    }

    for (wrap_name,wrap) in &[("both",Wrap2d::BOTH), ("horiz",Wrap2d::HORIZ), ("vert",Wrap2d::VERT), ("neither",Wrap2d::NEITHER)] {
        for loc in DIMS.iter_locs() {
            if loc.x % 25 == 0 && loc.y % 25 == 0 && loc.x == loc.y {
                for inc in &[Vec2d::new(-30, -30), Vec2d::new(-1, -1), Vec2d::new(1, 1), Vec2d::new(30, 30)] {
                    c.bench_function(
                        format!("{}.wrapped_add({}, {}, {})", wrap_name, DIM_WIDTH, loc, inc).as_ref(),
                        |b| b.iter(|| black_box(wrap.wrapped_add(black_box(DIMS), loc, *inc))),
                    );
                }
            }
        }
    }
}


criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
