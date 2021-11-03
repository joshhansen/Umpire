use clap::{Arg, App};

use crate::{
    conf::{
        FOG_OF_WAR,
        MAP_HEIGHT,
        MAP_WIDTH,
    },
    game::player::PlayerType,
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

            // 'm' => Arg::with_name("ai_model")
            //     .short("m")
            //     .long("model")
            //     .help("AI model file path")
            //     .takes_value(true),
            //     .multiple(true),// 

            'v' => Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("Show verbose output"),

            'w' => Arg::with_name("wrapping")
                .short("w")
                .long("wrapping")
                .help("Whether to wrap horizontally ('h'), vertically ('v'), both ('b'), or neither ('n')")
                .multiple(true)// Multiple so the AI trainer can specify multiple dimensions to train in sequence
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
                .multiple(true)// Multiple so the AI trainer can specify multiple dimensions to train in sequence
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
                .multiple(true)// Multiple so the AI trainer can specify multiple dimensions to train in sequence
                .validator(|s| {
                    let width: Result<u16,_> = s.trim().parse();
                    width.map(|_n| ()).map_err(|_e| format!("Invalid map width '{}'", s))
                }),

            c => panic!("Tried to build CLI with unrecognized flag '{}'", c)
        });
    }

    app
}

/// An item specified by a string on the command line
pub trait Specified : TryFrom<String> {

    /// A description to show up in the command line help
    fn desc(&self) -> String;

    /// A canonicalized string representation of the item
    fn spec(&self) -> String;
}

/// Parse a our little specification sub-language
/// 
/// Comma-separated and (for tokens that aren't file paths or otherwise already map to an item) split into individual
/// characters.
/// 
/// Examples:
/// * r
/// * r123
/// * r123,ai/model.ai,ai/tf_model
pub fn parse_spec<S1:AsRef<str>, S2:AsRef<str>, T:Specified>(spec: S1, spec_name: S2) -> Result<Vec<T>,String> {
    let mut items: Vec<T> = Vec::new();
    for spec2 in spec.as_ref().split(",") {
        match T::try_from(spec2.to_string()) {
            Ok(item) => items.push(item),
            Err(_) => {
                // char by char
                for spec3 in spec2.split_terminator("").skip(1) {
                    items.push(
                        T::try_from(spec3.to_string())
                        .map_err(|_| format!("{} is not a valid {} specification", spec2, spec_name.as_ref()))?
                    );
                }
            }
        }
    }
    Ok(items)
}

/// Parse the player specification
/// 
/// Examples:
/// * hr
/// * hr123
/// * hhhh
/// * hr123,ai/model.ai,ai/tf_model
pub fn parse_player_spec<S:AsRef<str>>(spec: S) -> Result<Vec<PlayerType>,String> {
    parse_spec(spec, "player")
}