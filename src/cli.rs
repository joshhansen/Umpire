use clap::{Arg, App};

use crate::conf::{
    FOG_OF_WAR,
    MAP_HEIGHT,
    MAP_WIDTH,
};

/// A standardized `clap` `App`. Provides uniformity to command line interfaces across binaries.
pub fn app<S:Into<String>>(name: S, included_flags: &'static str) -> App {
    let mut app = App::new(name);

    for c in included_flags.chars() {
        app = app.arg(match c {
            'f' => Arg::with_name("fog")
                .short("f")
                .long("fog")
                .help("Enable or disable fog of war")
                .takes_value(true)
                .default_value(FOG_OF_WAR)
                .possible_values(&["on","off"]),

            'm' => Arg::with_name("ai_model")
                .short("m")
                .long("model")
                .help("AI model file path")
                .takes_value(true),

            'v' => Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Show verbose output"),

            'w' => Arg::with_name("wrapping")
                .short("w")
                .long("wrapping")
                .help("Whether to wrap horizontally ('h'), vertically ('v'), both ('b'), or neither ('n')")
                .takes_value(true)
                .default_value("b")
                .validator(|s| {
                    match s.as_ref() {
                        "h" | "v" | "b" | "n" => Ok(()),
                        x => Err(format!("{} is not a supported wrapping type", x))
                    }
                }),

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

            c => panic!("Tried to build CLI with unrecognized flag '{}'", c)
        });
    }

    app
}