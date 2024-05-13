#![forbid(unsafe_code)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::let_and_return)]
#![allow(clippy::too_many_arguments)]

#[macro_use]
extern crate lazy_static;

pub mod cli;
pub mod colors;
pub mod conf;
pub mod game;
pub mod log;
pub mod name;
pub mod rpc;
pub mod util;
