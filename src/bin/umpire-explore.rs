use umpire::{
    game::unit::orders::test_support::test_explore,
    util::Dims,
};

fn main() {
    test_explore(Dims::new(100, 100));
}
