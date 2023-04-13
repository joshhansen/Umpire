use clap::{builder::Str, value_parser, Arg, ArgAction, Command};

use crate::{
    conf::{FOG_OF_WAR, MAP_HEIGHT, MAP_WIDTH},
    game::{ai::AISpec, player::PlayerType},
    util::Wrap2d,
};

/// An item specified by a string on the command line
pub trait Specified: TryFrom<String> {
    /// A description to show up in the command line help
    fn desc(&self) -> String;

    /// A canonicalized string representation of the item
    fn spec(&self) -> String;
}

pub fn players_arg() -> Arg {
    Arg::new("players")
        .short('p')
        .long("players")
        // .default_value("h1233")
        .help(format!(
            "Player type specification string, {}",
            PlayerType::values()
                .iter()
                .map(|player_type| format!("'{}' for {}", player_type.spec(), player_type.desc()))
                .collect::<Vec<String>>()
                .join(", ")
        ))
        .value_parser(|s: &str| parse_player_spec(s))
}

/// A standardized `clap` `App`. Provides uniformity to command line interfaces across binaries.
pub fn app(name: impl Into<Str>, included_flags: &'static str) -> Command {
    let mut app = Command::new(name);

    for c in included_flags.chars() {
        app = app.arg(match c {
            'f' => Arg::new("fog")
                .short('f')
                .long("fog")
                .help("Enable or disable fog of war")
                .default_value(FOG_OF_WAR)
                .value_parser(clap::builder::BoolishValueParser::new()),

            // 'm' => Arg::with_name("ai_model")
            //     .short("m")
            //     .long("model")
            //     .help("AI model file path")
            //     .takes_value(true),
            //     .multiple(true),// 

            'v' => Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::Count)
                .help("Show verbose output"),

            'w' => Arg::new("wrapping")
                .short('w')
                .long("wrapping")
                .help("Whether to wrap horizontally ('h'), vertically ('v'), both ('b'), or neither ('n')") 
                .default_value("b")
                .action(ArgAction::Append)// Multiple so the AI trainer can specify multiple dimensions to train in sequence
                .value_parser(|s:&str| Wrap2d::try_from(s)),

            'H' => Arg::new("map_height")
                .short('H')
                .long("height")
                .help("Map height")
                .default_value(MAP_HEIGHT)
                .action(ArgAction::Append)// Multiple so the AI trainer can specify multiple dimensions to train in sequence
                .value_parser(value_parser!(u16)),

            'W' => Arg::new("map_width")
                .short('W')
                .long("width")
                .help("Map width")
                .default_value(MAP_WIDTH)
                .action(ArgAction::Append)// Multiple so the AI trainer can specify multiple dimensions to train in sequence
                .value_parser(value_parser!(u16)),

            'D' => Arg::new("dnn_learning_rate")
                .short('D')
                .long("dnnlr")
                .help("The learning rate of the neural network (if any)")
                .value_parser(value_parser!(f32))
                .default_value("10e-3"),

            c => panic!("Tried to build CLI with unrecognized flag '{}'", c)
        });
    }

    app
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
pub fn parse_spec<S1: AsRef<str>, S2: AsRef<str>, T: Specified>(
    spec: S1,
    spec_name: S2,
) -> Result<Vec<T>, String> {
    let mut items: Vec<T> = Vec::new();
    for spec2 in spec.as_ref().split(",") {
        match T::try_from(spec2.to_string()) {
            Ok(item) => items.push(item),
            Err(_) => {
                // char by char
                for spec3 in spec2.split_terminator("").skip(1) {
                    items.push(T::try_from(spec3.to_string()).map_err(|_| {
                        format!(
                            "{} is not a valid {} specification",
                            spec2,
                            spec_name.as_ref()
                        )
                    })?);
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
pub fn parse_player_spec<S: AsRef<str>>(spec: S) -> Result<Vec<PlayerType>, String> {
    parse_spec(spec, "player")
}

pub fn parse_ai_spec<S: AsRef<str>>(spec: S) -> Result<Vec<AISpec>, String> {
    parse_spec(spec, "AI")
}

#[cfg(test)]
mod test {
    use super::parse_ai_spec;
    use crate::game::ai::AISpec;

    #[test]
    fn test_parse_ai_spec() {
        assert_eq!(
            parse_ai_spec("rr"),
            Ok(vec![AISpec::Random, AISpec::Random])
        );

        assert_eq!(
            parse_ai_spec("r,r"),
            Ok(vec![AISpec::Random, AISpec::Random])
        );
    }
}
