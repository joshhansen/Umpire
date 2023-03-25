//! Color
//!
//! Umpire's approach to color:
//! * Support 256 colors as the main color scheme as this is widely supported and gives us a much nicer set of options
//! * Support 16 colors as a fallback, with the number of players limited by the number of available colors
//! * Support true color (24 bit color) as a secondary option for those with proper support
//!
//! The default is 256 colors, but the color scheme can be selected by a command line flag.
//!
//! Umpire's colors are divided into two categories: normal and paired.
//!
//! Normal colors are single colors. Just like normal.
//!
//! Paired colors have an "active" and a "foggy" color, paired together. This is to support the fog of war effect on the map.
//!
//! There are a further three categories:
//! * general: colors that can apply anywhere
//! * map: colors only used on the map
//! * text: colors used in textual output outside of the map

use common::{colors::Colors, game::PlayerNum};
use crossterm::style::Color;

use pastel::{
    distinct::{distinct_colors, DistanceMetric, IterationStatistics},
    Color as PastelColor, RGBA,
};

pub trait PairColorized {
    fn color_pair(&self, palette: &Palette) -> Option<ColorPair>;
}

pub trait PaletteT {
    fn background(&self) -> Color;
}

pub struct Palette {
    // general
    background: Color,

    // map
    land: ColorPair,
    ocean: ColorPair,
    players: Vec<ColorPair>,
    neutral: ColorPair,

    // text
    text: Color,

    notice: Color,

    cursor: Color,

    combat: Color,

    // other
    scroll_marks: Color,
}

impl Palette {
    pub fn get_single(&self, color: Colors) -> Color {
        match color {
            Colors::Background => self.background,
            Colors::Notice => self.notice,
            Colors::Text => self.text,
            Colors::Combat => self.combat,
            Colors::Cursor => self.cursor,
            Colors::ScrollMarks => self.scroll_marks,
            _ => panic!("Color {:?} is not a single color", color),
        }
    }

    pub fn get(&self, color: Colors, currently_observed: bool) -> Color {
        match color {
            Colors::Background => self.background,
            Colors::Land => self.land.get(currently_observed),
            Colors::Ocean => self.ocean.get(currently_observed),
            Colors::Neutral => self.neutral.get(currently_observed),
            Colors::Player(player_num) => {
                self.players[usize::from(player_num)].get(currently_observed)
            }
            Colors::Notice => self.notice,
            Colors::Text => self.text,
            Colors::Cursor => self.cursor,
            Colors::Combat => self.combat,
            Colors::ScrollMarks => self.scroll_marks,
        }
    }

    pub fn get_pair(&self, color: Colors) -> ColorPair {
        match color {
            Colors::Land => self.land,
            Colors::Ocean => self.ocean,
            Colors::Neutral => self.neutral,
            Colors::Player(player_num) => self.players[usize::from(player_num)],
            _ => panic!("Color {:?} is not a paired color", color),
        }
    }
}

pub fn palette16(num_players: PlayerNum) -> Result<Palette, String> {
    if num_players > 5 {
        Err(format!(
            "Chosen color palette only supports 5 players, but {} were specified",
            num_players
        ))
    } else {
        Ok(Palette {
            background: Color::Reset, // Color::Black,
            land: ColorPair::new(Color::Green, Color::DarkGreen),
            ocean: ColorPair::new(Color::Blue, Color::DarkBlue),
            players: vec![
                ColorPair::new(Color::Red, Color::DarkRed),
                ColorPair::new(Color::White, Color::Grey),
                ColorPair::new(Color::Magenta, Color::DarkMagenta),
                ColorPair::new(Color::Yellow, Color::DarkYellow),
                ColorPair::new(Color::Cyan, Color::DarkCyan),
            ],
            neutral: ColorPair::new(Color::DarkGrey, Color::DarkGrey),
            text: Color::White,
            notice: Color::Cyan,
            cursor: Color::White,
            combat: Color::Red,
            scroll_marks: Color::Yellow,
        })
    }
}

pub fn palette256(num_players: PlayerNum) -> Result<Palette, String> {
    palette16(num_players) //These are the same for now
                           // Palette {
                           //     background: AnsiValue(0),
                           //     land: ColorPair::new_ansi(10, 2),
                           //     ocean: ColorPair::new_ansi(12, 4),
                           //     players: vec![
                           //         ColorPair::new_ansi(9, 1),// red
                           //         ColorPair::new_ansi(15, 7),// white
                           //         ColorPair::new_ansi(13, 5),// purple
                           //         ColorPair::new_ansi(11, 3),// yellow
                           //         ColorPair::new_ansi(14, 6),// cyan
                           //     ],
                           //     neutral: ColorPair::new_ansi(8, 8),// gray
                           //     text: AnsiValue(15),
                           //     notice: AnsiValue(14),
                           //     cursor: AnsiValue(15),
                           //     combat: AnsiValue(9),
                           //     scroll_marks: AnsiValue(11),
                           // }
}

fn pastel_color_to_rgb(pastel_color: &PastelColor) -> Color {
    let rgba: RGBA<u8> = pastel_color.to_rgba();
    Color::Rgb {
        r: rgba.r,
        g: rgba.g,
        b: rgba.b,
    }
}

fn pastel_color_to_dim_rgb(pastel_color: &PastelColor, darken_percent: f64) -> Color {
    let pastel_color = pastel_color.darken(darken_percent);
    pastel_color_to_rgb(&pastel_color)
}

fn pastel_color_to_rgb_pair(pastel_color: &PastelColor, darken_percent: f64) -> ColorPair {
    ColorPair::new(
        pastel_color_to_rgb(pastel_color),
        pastel_color_to_dim_rgb(pastel_color, darken_percent),
    )
}

fn color_to_rgb_pair(color: Color, darken_percent: f64) -> ColorPair {
    let pastel_color = color_to_pastel_color(color);
    ColorPair::new(
        pastel_color_to_rgb(&pastel_color),
        pastel_color_to_dim_rgb(&pastel_color, darken_percent),
    )
}

fn color_to_pastel_color(color: Color) -> PastelColor {
    if let Color::Rgb { r, g, b } = color {
        PastelColor::from_rgb(r, g, b)
    } else {
        panic!("Unsupported color type to convert to pastel color format")
    }
}

pub fn palette24(num_players: PlayerNum, darken_percent: f64) -> Palette {
    let land = color_to_rgb_pair(
        Color::Rgb {
            r: 24,
            g: 216,
            b: 67,
        },
        darken_percent,
    );
    let ocean = color_to_rgb_pair(
        Color::Rgb {
            r: 60,
            g: 27,
            b: 225,
        },
        darken_percent,
    );
    let neutral = color_to_rgb_pair(
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
        darken_percent,
    );

    let preexisting = vec![
        color_to_pastel_color(land.active),
        color_to_pastel_color(ocean.active),
        color_to_pastel_color(neutral.active),
    ];

    let num_preexisting = preexisting.len();

    let mut callback = |_stats: &IterationStatistics| {};

    let distinct: Vec<PastelColor> = distinct_colors(
        usize::from(num_players) + num_preexisting,
        // DistanceMetric::CIE76,
        DistanceMetric::CIEDE2000,
        preexisting,
        &mut callback,
    )
    .0
    .iter()
    .skip(num_preexisting)
    .cloned()
    .collect();

    let players: Vec<ColorPair> = distinct
        .iter()
        .map(|pastel_color| pastel_color_to_rgb_pair(pastel_color, darken_percent))
        .collect();

    debug_assert_eq!(players.len(), num_players);

    Palette {
        background: Color::Rgb { r: 0, g: 0, b: 0 },
        land,
        ocean,
        players,
        neutral,
        text: Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
        notice: Color::Rgb {
            r: 232,
            g: 128,
            b: 56,
        },
        combat: Color::Rgb {
            r: 237,
            g: 89,
            b: 66,
        },
        cursor: Color::Rgb {
            r: 255,
            g: 154,
            b: 71,
        },
        scroll_marks: Color::Rgb {
            r: 248,
            g: 202,
            b: 0,
        },
    }
}

#[derive(Copy, Clone)]
pub struct ColorPair {
    active: Color,
    foggy: Color,
}

impl ColorPair {
    pub fn new(active: Color, foggy: Color) -> Self {
        Self { active, foggy }
    }

    pub fn get(self, currently_observed: bool) -> Color {
        if currently_observed {
            self.active
        } else {
            self.foggy
        }
    }
}
