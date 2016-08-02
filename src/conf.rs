
/// The name of this application
pub const APP_NAME: &'static str = "umpire";

/// The width of the game map
pub const MAP_WIDTH: u16 = 180;

/// The height of the game map
pub const MAP_HEIGHT: u16 = 90;

/// The height of the header
pub const HEADER_HEIGHT: u16 = 1;

/// The height of the footer
pub const FOOTER_HEIGHT: u16 = 5;

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
pub const NUM_TEAMS: u8 = 4;

// Key mappings
pub const KEY_VIEWPORT_SHIFT_LEFT:       char = 'H';
pub const KEY_VIEWPORT_SHIFT_RIGHT:      char = 'L';
pub const KEY_VIEWPORT_SHIFT_UP:         char = 'K';
pub const KEY_VIEWPORT_SHIFT_DOWN:       char = 'J';
pub const KEY_VIEWPORT_SHIFT_UP_LEFT:    char = 'Y';
pub const KEY_VIEWPORT_SHIFT_UP_RIGHT:   char = 'U';
pub const KEY_VIEWPORT_SHIFT_DOWN_LEFT:  char = 'B';
pub const KEY_VIEWPORT_SHIFT_DOWN_RIGHT: char = 'N';

pub const KEY_QUIT: char = 'q';
