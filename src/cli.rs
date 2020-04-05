use clap::{Arg, App};

use crate::conf::{
    MAP_HEIGHT,
    MAP_WIDTH,
};

/// A standardized `clap` `App`. Provides uniformity to command line interfaces across binaries.
pub fn app<S:Into<String>>(name: S, included_flags: &'static str) -> App {
    let mut app = App::new(name);

    for c in included_flags.chars() {
        app = app.arg(match c {
            'H' => Arg::with_name("map_height")
                .short("H")
                .long("height")
                .help("Map height")
                .takes_value(true)
                .default_value(MAP_HEIGHT)
                .validator(|s| {
                    let width: Result<u16,_> = s.trim().parse();
                    width.map(|_n| ()).map_err(|_e| format!("Invalid map height '{}'", s))
                }),
            'W' => Arg::with_name("map_width")
                .short("W")
                .long("width")
                .help("Map width")
                .takes_value(true)
                .default_value(MAP_WIDTH)
                .validator(|s| {
                    let width: Result<u16,_> = s.trim().parse();
                    width.map(|_n| ()).map_err(|_e| format!("Invalid map width '{}'", s))
                }),
            'v' => Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Show verbose output"),
            c => panic!("Tried to build CLI with unrecognized flag '{}'", c)
        });
    }

    app
}