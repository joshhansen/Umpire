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

use crossterm::style::Color;

use pastel::{
    Color as PastelColor,
    RGBA,
    distinct::{
        DistanceMetric,
        IterationStatistics,
        distinct_colors,
    },
    random::{
        RandomizationStrategy,
        strategies::Vivid,
    },
};

use crate::game::PlayerNum;

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
    scroll_marks: Color
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
            _ => panic!("Color {:?} is not a single color", color)
        }
    }

    pub fn get(&self, color: Colors, currently_observed: bool) -> Color {
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

    pub fn get_pair(&self, color: Colors) -> ColorPair {
        match color {
            Colors::Land => self.land,
            Colors::Ocean => self.ocean,
            Colors::Neutral => self.neutral,
            Colors::Player(player_num) => self.players[usize::from(player_num)],
            _ => panic!("Color {:?} is not a paired color", color)
        }
    }
}

pub fn palette16() -> Palette {
    Palette {
        background: Color::Black,
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
    }
}

pub fn palette256() -> Palette {
    palette16()//These are the same for now
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
        r: rgba.r, g: rgba.g, b: rgba.b
    }
}

fn pastel_color_to_dim_rgb(pastel_color: &PastelColor, darken_percent: f64) -> Color {
    let pastel_color = pastel_color.darken(darken_percent);
    pastel_color_to_rgb(&pastel_color)
}

fn pastel_color_to_rgb_pair(pastel_color: &PastelColor, darken_percent: f64) -> ColorPair {
    ColorPair::new(pastel_color_to_rgb(pastel_color), pastel_color_to_dim_rgb(pastel_color, darken_percent))
}

fn color_to_rgb_pair(color: Color, darken_percent: f64) -> ColorPair {
    let pastel_color = color_to_pastel_color(color);
    ColorPair::new(pastel_color_to_rgb(&pastel_color), pastel_color_to_dim_rgb(&pastel_color, darken_percent))
}

fn color_to_pastel_color(color: Color) -> PastelColor {
    if let Color::Rgb{r,g,b} = color {
        PastelColor::from_rgb(r,g,b)
    } else {
        panic!("Unsupported color type to convert to pastel color format")
    }
}

pub fn palette24(num_players: PlayerNum, darken_percent: f64) -> Result<Palette,String> {
    let land_color_pair = color_to_rgb_pair(Color::Rgb{r:24, g:216, b:67}, darken_percent);
    let ocean_color_pair = color_to_rgb_pair(Color::Rgb{r:60, g:27, b:225}, darken_percent);


    let player_colors: Vec<ColorPair> = if num_players == 0 {
        Vec::new()
    } else if num_players == 1 {
        vec![
            pastel_color_to_rgb_pair(&Vivid.generate(), darken_percent)
        ]
    } else {
        let land_color: PastelColor = color_to_pastel_color(land_color_pair.active);
        let ocean_color: PastelColor = color_to_pastel_color(ocean_color_pair.active);
        let preexisting_colors = vec![land_color, ocean_color];
        
        let mut callback = |_stats: &IterationStatistics| {};

        let pastel_colors: Vec<PastelColor> = distinct_colors(
            usize::from(num_players) + 2,
            // DistanceMetric::CIE76,
            DistanceMetric::CIEDE2000,
            preexisting_colors,
            &mut callback,
        ).0.iter().skip(2).cloned().collect();
        
        let rgb_pairs: Vec<ColorPair> = pastel_colors.iter().map(|pastel_color| pastel_color_to_rgb_pair(pastel_color, darken_percent)).collect();
        rgb_pairs
    };

    Ok(Palette {
        background: Color::Rgb{r:0,g:0,b:0},
        land: land_color_pair,//ColorPair::new_rgb(24,216,67, 141,185,138),
        ocean: ocean_color_pair,//ColorPair::new_rgb(60,27,225, 78,50,171),
        players: player_colors,
        neutral: color_to_rgb_pair(Color::Rgb{r:202,g:202,b:202}, darken_percent),//ColorPair::new_rgb(202,202,202, 102,102,102),
        text: Color::Rgb{r:255,g:255,b:255},
        notice: Color::Rgb{r:232,g:128,b:56},
        combat: Color::Rgb{r:237,g:89,b:66},
        cursor: Color::Rgb{r:255,g:154,b:71},
        scroll_marks: Color::Rgb{r:248,g:202,b:0},
    })
}


#[derive(Copy,Clone)]
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