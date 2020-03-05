//! Binary for profiling the wrapped_add function

use umpire::{
    util::Wrap,
};

const DIM_WIDTH: u16 = 100;
// const DIMS: Dims = Dims::new(100, 100);

const ITERATIONS: usize = 10000000;


fn main() {
    for _it in 0..ITERATIONS {
        Wrap::Wrapping.wrapped_add(DIM_WIDTH, 90, 20);
        Wrap::NonWrapping.wrapped_add(DIM_WIDTH, 90, 20);
    }
}
