/// A function that is opaque to the optimizer, used to prevent the compiler from
/// optimizing away computations in a benchmark.
///
/// This variant is stable-compatible, but it may cause some performance overhead
/// or fail to prevent code from being eliminated.
#[cfg(not(feature = "real_blackbox"))]
pub fn black_box<T>(dummy: T) -> T {
    unsafe {
        let ret = std::ptr::read_volatile(&dummy);
        std::mem::forget(dummy);
        ret
    }
}

use umpire::{
    util::{Dims,Location,Vec2d,Wrap,Wrap2d},
};

const DIM_WIDTH: u16 = 100;
// const DIMS: Dims = Dims::new(100, 100);

const ITERATIONS: usize = 10000000;


fn main() {
    // for wrap in &[Wrap::Wrapping, Wrap::NonWrapping] {
    //     for coord in (0..DIM_WIDTH).step_by(25) {
    //         for inc in &[-30, -1, 1, 30] {

                for _it in 0..ITERATIONS {
                    // println!("{}", it);
                    black_box(Wrap::Wrapping.wrapped_add(black_box(DIM_WIDTH), 90, 20));
                    black_box(Wrap::NonWrapping.wrapped_add(black_box(DIM_WIDTH), 90, 20));
                }

                // c.bench_function(
                //     format!("{:?}.wrapped_add({}, {}, {})", wrap, DIM_WIDTH, coord, inc).as_ref(),
                //     |b| b.iter(|| black_box(wrap.wrapped_add(black_box(DIM_WIDTH), coord, *inc))),
                // );
    //         }
    //     }
    // }

    // for (wrap_name,wrap) in &[("both",Wrap2d::BOTH), ("horiz",Wrap2d::HORIZ), ("vert",Wrap2d::VERT), ("neither",Wrap2d::NEITHER)] {
    //     for loc in DIMS.iter_locs() {
    //         if loc.x % 25 == 0 && loc.y % 25 == 0 && loc.x == loc.y {
    //             for inc in &[Vec2d::new(-30, -30), Vec2d::new(-1, -1), Vec2d::new(1, 1), Vec2d::new(30, 30)] {
    //                 c.bench_function(
    //                     format!("{}.wrapped_add({}, {}, {})", wrap_name, DIM_WIDTH, loc, inc).as_ref(),
    //                     |b| b.iter(|| black_box(wrap.wrapped_add(black_box(DIMS), loc, *inc))),
    //                 );
    //             }
    //         }
    //     }
    // }
}
