//! Configuration
//!
//! For now this is just a bunch of const's, but in the future I expect it will be something more
//! sophisticated that allows configuration to be set through a combination of defaults, command
//! line arguments, and configuration files.

// use std::collections::HashMap;
// use std::convert::AsRef;
// use std::env;
// use std::ffi::OsStr;
// use std::str::FromStr;
//
// mod keys {
//     pub const APP_NAME: &'static str = "APP_NAME";
// }
//
// pub struct Config {
//     map: HashMap<&'static str, String>
// }
//
// impl Config {
//     fn new() -> Config {
//         let mut conf = Config {
//             map: HashMap::new()
//         };
//
//
//
//
//
//         conf
//     }
// }
//
// pub fn get<K:AsRef<OsStr>,T:FromStr>(key: K) -> Result<T,()> {
//     match env::var(key) {
//         Ok(val) => match val.parse::<T>() {
//             Ok(parsed_val) => Ok(parsed_val),
//             Err(_) => Err(())
//         },
//         Err(_e) => Err(())
//     }
// }

/// The name of this application
pub const APP_NAME: &'static str = "umpire";

/// The subtitle. You know, for flavor.
pub const APP_SUBTITLE: &'static str = "Combat Quest of the Millennium";

// pub const USER_NAME: &'static str = "Jersh";

/// The width of the game map
pub const MAP_WIDTH: u16 = 180;

/// The height of the game map
pub const MAP_HEIGHT: u16 = 90;

/// The height of the header
pub const HEADER_HEIGHT: u16 = 1;

/// The height of the footer
// pub const FOOTER_HEIGHT: u16 = 5;

/// The number of landmasses to seed during map generation
pub const LANDMASSES:u16 = 150;

/// The number of iterations to grow landmasses during map generation
pub const GROWTH_ITERATIONS : u16 = 5;

/// The degree to which cardinal-direction landmass growth should be discouraged
pub const GROWTH_CARDINAL_LAMBDA : f32 = 2_f32;

/// The degree to which diagonal landmass growth should be discouraged
pub const GROWTH_DIAGONAL_LAMBDA : f32 = 5_f32;

pub const NEUTRAL_CITY_DENSITY : f32 = 0.05;

/// The number of teams playing, including humans and AIs
pub const NUM_PLAYERS: &'static str = "4";

pub const FOG_OF_WAR: &'static str = "on";

pub const USE_ALTERNATE_SCREEN: &'static str = "on";

// pub const HUMAN_PLAYER: PlayerNum = 0;

// Key mappings
pub const KEY_VIEWPORT_SIZE_ROTATE:      char = 'v';
pub const KEY_LEFT:       char = 'h';
pub const KEY_RIGHT:      char = 'l';
pub const KEY_UP:         char = 'k';
pub const KEY_DOWN:       char = 'j';
pub const KEY_UP_LEFT:    char = 'y';
pub const KEY_UP_RIGHT:   char = 'u';
pub const KEY_DOWN_LEFT:  char = 'b';
pub const KEY_DOWN_RIGHT: char = 'n';
pub const KEY_VIEWPORT_SHIFT_LEFT: char = 'H';
pub const KEY_VIEWPORT_SHIFT_RIGHT: char = 'L';
pub const KEY_VIEWPORT_SHIFT_UP: char = 'K';
pub const KEY_VIEWPORT_SHIFT_DOWN: char = 'J';
pub const KEY_VIEWPORT_SHIFT_UP_LEFT: char = 'Y';
pub const KEY_VIEWPORT_SHIFT_UP_RIGHT: char = 'U';
pub const KEY_VIEWPORT_SHIFT_DOWN_LEFT: char = 'B';
pub const KEY_VIEWPORT_SHIFT_DOWN_RIGHT: char = 'N';

pub const KEY_QUIT: char = 'q';

pub const KEY_EXAMINE: char = 'x';
pub const KEY_EXAMINE_SELECT: char = '\n';
