#![forbid(unsafe_code)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::let_and_return)]
#![allow(clippy::too_many_arguments)]

pub mod cli;
pub mod color;
pub mod conf;
pub mod game;
pub mod log;
pub mod name;
pub mod ui;
pub mod util;


#[cfg(test)]
pub(crate) mod test_support {
    pub(crate) use crate::game::test_support::{
        game1,
        game_tunnel,
        game_two_cities_two_infantry,
    };
}