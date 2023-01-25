#[macro_use]
extern crate criterion;

use criterion::black_box;
use criterion::Criterion;

use umpire::util::{Dims, Location, Vec2d, Wrap, Wrap2d};

const DIM_WIDTH: u16 = 90;
const DIMS: Dims = Dims::new(90, 90);

fn criterion_benchmark(c: &mut Criterion) {
    for wrap in &[Wrap::Wrapping, Wrap::NonWrapping] {
        for coord in (0..DIM_WIDTH).step_by(30) {
            for inc in &[-40, 40] {
                c.bench_function(
                    format!("{:?}.wrapped_add({}, {}, {})", wrap, DIM_WIDTH, coord, inc).as_ref(),
                    |b| b.iter(|| black_box(wrap.wrapped_add(black_box(DIM_WIDTH), coord, *inc))),
                );
            }
        }
    }

    for (wrap_name, wrap) in &[("both", Wrap2d::BOTH), ("neither", Wrap2d::NEITHER)] {
        for coord in (0..DIM_WIDTH).step_by(30) {
            let loc = Location::new(coord, coord);
            for inc in &[Vec2d::new(-40, -40), Vec2d::new(40, 40)] {
                c.bench_function(
                    format!("{}.wrapped_add({}, {}, {})", wrap_name, DIM_WIDTH, loc, inc).as_ref(),
                    |b| b.iter(|| black_box(wrap.wrapped_add(black_box(DIMS), loc, *inc))),
                );
            }
        }
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
