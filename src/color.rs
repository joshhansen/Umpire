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

use pastel::Color as PastelColor;
use pastel::RGBA;
use pastel::distinct::{DistanceMetric,distinct_colors};
use pastel::random::RandomizationStrategy;
use pastel::random::strategies::Vivid;

use termion::color::AnsiValue;
use termion::color::Color;
use termion::color::Rgb;

use game::PlayerNum;

#[derive(Copy,Clone,Debug)]
pub enum Colors {
    /// The background behind everything else
    Background,

    /// Dry land
    Land,

    /// The ocean
    Ocean,

    /// The neutral "player"'s color
    Neutral,

    /// A player's color
    Player(u8),

    /// The default text color
    Text,

    /// A message that needs to be extra noticeable
    Notice,

    /// The cursor
    Cursor,

    /// Messages or effects about combat
    Combat,

    /// Scroll percentage indicators
    ScrollMarks,
}

pub trait Colorized {
    fn color(&self) -> Option<Colors>;
}

pub trait PairColorized<C:Color+Copy> {
    fn color_pair(&self, palette: &Palette<C>) -> Option<ColorPair<C>>;
}

pub trait PaletteT<C:Color> {
    fn background(&self) -> C;
}

pub struct Palette<C> {
    // general
    background: C,
    

    // map
    land: ColorPair<C>,
    ocean: ColorPair<C>,
    players: Vec<ColorPair<C>>,
    neutral: ColorPair<C>,

    // text
    text: C,

    notice: C,

    cursor: C,

    combat: C,

    // other
    scroll_marks: C
}

impl <C:Color+Copy> Palette<C> {
    pub fn get_single(&self, color: Colors) -> C {
        match color {
            Colors::Background => self.background,
            Colors::Notice => self.notice,
            Colors::Text => self.text,
            Colors::Combat => self.combat,
            Colors::Cursor => self.cursor,
            Colors::ScrollMarks => self.scroll_marks,
            _ => panic!("Color {:?} is not a single color", color)
        }
    }

    pub fn get(&self, color: Colors, currently_observed: bool) -> C {
        match color {
            Colors::Background => self.background,
            Colors::Land => self.land.get(currently_observed),
            Colors::Ocean => self.ocean.get(currently_observed),
            Colors::Neutral => self.neutral.get(currently_observed),
            Colors::Player(player_num) => self.players[usize::from(player_num)].get(currently_observed),
            Colors::Notice => self.notice,
            Colors::Text => self.text,
            Colors::Cursor => self.cursor,
            Colors::Combat => self.combat,
            Colors::ScrollMarks => self.scroll_marks
        }
    }

    pub fn get_pair(&self, color: Colors) -> ColorPair<C> {
        match color {
            Colors::Land => self.land,
            Colors::Ocean => self.ocean,
            Colors::Neutral => self.neutral,
            Colors::Player(player_num) => self.players[usize::from(player_num)],
            _ => panic!("Color {:?} is not a paired color", color)
        }
    }
}

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
        notice: AnsiValue(14),
        cursor: AnsiValue(15),
        combat: AnsiValue(9),
        scroll_marks: AnsiValue(11),
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
        notice: AnsiValue(14),
        cursor: AnsiValue(15),
        combat: AnsiValue(9),
        scroll_marks: AnsiValue(11),
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
        notice: Rgb(232,128,56),
        combat: Rgb(237,89,66),
        cursor: Rgb(255,154,71),
        scroll_marks: Rgb(248,202,0),
    })
}

#[derive(Copy,Clone)]
pub struct ColorPair<C> {
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