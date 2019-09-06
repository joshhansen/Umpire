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

use std::fmt;

use pastel::Color as PastelColor;
use pastel::RGBA;
use pastel::distinct::{DistanceMetric,distinct_colors};
use pastel::random::RandomizationStrategy;
use pastel::random::strategies::Vivid;

use termion::color::AnsiValue;
use termion::color::Color;
use termion::color::Rgb;

use game::PlayerNum;

pub const BLACK: Rgb = Rgb(0, 0, 0);
pub const WHITE: Rgb = Rgb(255, 255, 255);

pub const NOTICE: Rgb = Rgb(255,140,0);

pub trait Colorized<C:Color+Copy> {
    fn color(&self, palette: &Palette<C>) -> C;
}

pub trait PairColorized<C:Color+Copy> {
    fn color_pair(&self, palette: &Palette<C>) -> Option<ColorPair<C>>;
}

pub trait PaletteT<C:Color> {
    fn background(&self) -> C;
}

pub struct Palette<C:Color> {
    // general
    background: C,
    

    // map
    pub land: ColorPair<C>,
    pub ocean: ColorPair<C>,
    pub players: Vec<ColorPair<C>>,
    pub neutral: ColorPair<C>,

    // text
    text: C,
    notice: C,
}

// impl <C:Color> Palette<C> {
//     fn new(
//         background: ColorPair<C>,
//         text: C,
//         land: ColorPair<C>,
//         ocean: ColorPair<C>,
//         notice: C,
//         player_colors: Vec<ColorPair<C>>) -> Self {
//             Self {
//                 background,
//                 text,
//                 land,
//                 ocean,
//                 notice,
//                 player_colors
//             }
//         }
// }

pub fn palette16() -> Palette<AnsiValue> {
    Palette {
        background: AnsiValue(0),
        land: ColorPair::new_ansi(10, 2),
        ocean: ColorPair::new_ansi(12, 4),
        players: vec![
            ColorPair::new_ansi(9, 1),// red
            ColorPair::new_ansi(11, 3),// yellow
            ColorPair::new_ansi(13, 5),// purple
            ColorPair::new_ansi(14, 6),// cyan
            ColorPair::new_ansi(15, 7),// white
        ],
        neutral: ColorPair::new_ansi(8, 8),// gray
        text: AnsiValue(15),
        notice: AnsiValue(14)
    }
}

pub fn palette256() -> Palette<AnsiValue> {
    Palette {
        background: AnsiValue(0),
        land: ColorPair::new_ansi(10, 2),
        ocean: ColorPair::new_ansi(12, 4),
        players: vec![
            ColorPair::new_ansi(9, 1),// red
            ColorPair::new_ansi(11, 3),// yellow
            ColorPair::new_ansi(13, 5),// purple
            ColorPair::new_ansi(14, 6),// cyan
            ColorPair::new_ansi(15, 7),// white
        ],
        neutral: ColorPair::new_ansi(8, 8),// gray
        text: AnsiValue(15),
        notice: AnsiValue(14)
    }
}

fn pastel_color_to_rgb(pastel_color: &PastelColor) -> Rgb {
    let rgba: RGBA<u8> = pastel_color.to_rgba();
    Rgb(rgba.r, rgba.g, rgba.b)
}

fn pastel_color_to_dim_rgb(pastel_color: &PastelColor) -> Rgb {
    let pastel_color = pastel_color.darken(0.66);
    pastel_color_to_rgb(&pastel_color)
}

fn pastel_color_to_rgb_pair(pastel_color: &PastelColor) -> ColorPair<Rgb> {
    ColorPair::new(pastel_color_to_rgb(pastel_color), pastel_color_to_dim_rgb(pastel_color))
}

pub fn palette24(num_players: PlayerNum) -> Result<Palette<Rgb>,String> {
    let player_colors: Vec<ColorPair<Rgb>> = if num_players == 0 {
        Vec::new()
    } else if num_players == 1 {
        vec![
            pastel_color_to_rgb_pair(&Vivid.generate())
        ]
    } else {
        let pastel_colors: Vec<PastelColor> = distinct_colors(usize::from(num_players), DistanceMetric::CIE76, false, true).unwrap().0;
        
        let rgb_pairs: Vec<ColorPair<Rgb>> = pastel_colors.iter().map(pastel_color_to_rgb_pair).collect();
        rgb_pairs
    };

    Ok(Palette {
        background: Rgb(0,0,0),
        land: ColorPair::new_rgb(24,216,67, 141,185,138),
        ocean: ColorPair::new_rgb(60,27,225, 78,50,171),
        players: player_colors,
        neutral: ColorPair::new_rgb(202,202,202, 102,102,102),
        text: Rgb(255,255,255),
        notice: Rgb(232,128,56)
    })
}


// trait Palette<C:Color> {
//     fn background()-> ColorPair<C>;
//     fn foreground() -> ColorPair<C>;
//     fn notice() -> ColorPair<C>;
//     fn player_color(player: PlayerNum) -> ColorPair<C>;
// }


// struct Palette16 {

// }

// impl Palette<AnsiValue> for Palette16 {
//     fn background()-> ColorPair<AnsiValue> {

//     }
//     fn foreground() -> ColorPair<AnsiValue>;
//     fn notice() -> ColorPair<AnsiValue>;
//     fn player_color(player: PlayerNum) -> ColorPair<AnsiValue>;
// }


// struct Palette256 {

// }

// impl Palette<AnsiValue> for Palette256 {
//     fn background()-> ColorPair<AnsiValue>;
//     fn foreground() -> ColorPair<AnsiValue>;
//     fn notice() -> ColorPair<AnsiValue>;
//     fn player_color(player: PlayerNum) -> ColorPair<AnsiValue>;
// }

// struct Palette24bit {

// }

// impl Palette<Rgb> for Palette24bit {

// }




// struct Color {
//     ansi_normal: u8,
//     ansi_bright: u8,
//     rgb_normal: Rgb,
//     rgb_bright: Rgb,
// }

// impl Color {
//     fn new(ansi_normal: u8, ansi_bright: u8, rgb_normal: Rgb, rgb_bright: Rgb) -> Self {
//         Self {
//             ansi_normal,
//             ansi_bright,
//             rgb_normal,
//             rgb_bright,
//         }
//     }


// }

// const BLACK: Color = Color::new(0, 8, Rgb(0, 0, 0), Rgb(50, 50, 50));

// /// A hybrid color declared in 8-bit color (256 color), and optionally in 16 color and 24-bit color.
// /// 
// /// The standard color system for this application is 8-bit color with its 256 values. This is widely
// /// supported across terminals.
// #[derive(Debug)]
// struct CompatibleColor {
//     ansi_16_color: Option<AnsiValue>,
//     ansi_256_color: AnsiValue,
//     ansi_rgb_color: Option<Rgb>
// }

// impl Color for CompatibleColor {
//     /// Write the foreground version of this color.
//     fn write_fg(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         let mut result = self.ansi_16_color.write_fg(f);
//         if let Some(ansi_256_color) = self.ansi_256_color {
//             result = result.and(ansi_256_color.write_fg(f));
//         }

//         if let Some(ansi_rgb_color) = self.ansi_rgb_color {
//             result = result.and(ansi_rgb_color.write_fg(f));
//         }

//         result
//     }

//     /// Write the background version of this color.
//     fn write_bg(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         self.ansi_16_color.write_bg(f).and(
//             self.ansi_256_color
//         )
//     }
// }

#[derive(Copy,Clone)]
pub struct ColorPair<C:Color> {
    active: C,
    foggy: C
}

impl <C:Color+Copy> ColorPair<C> {
    pub fn new(active: C, foggy: C) -> Self {
        Self { active, foggy }
    }

    pub fn get(&self, currently_observed: bool) -> C {
        if currently_observed {
            self.active
        } else {
            self.foggy
        }
    }
}

impl ColorPair<AnsiValue> {
    fn new_ansi(active: u8, foggy: u8) -> Self {
        Self::new( AnsiValue(active), AnsiValue(foggy) )
    }
}

impl ColorPair<Rgb> {
    fn new_rgb(
        active_r: u8, active_g: u8, active_b: u8,
        foggy_r: u8, foggy_g: u8, foggy_b: u8
    ) -> Self {
        Self::new( Rgb(active_r, active_g, active_b), Rgb(foggy_r, foggy_g, foggy_b) )
    }
}

