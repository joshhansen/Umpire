//!
//! Umpire: a game of world conquest
//!

//Wishlist:
// Copy is implemented for Rgb, Bg, Fg

extern crate rand;
extern crate terminal_size;
extern crate termion;

use std::io::{Read, Write, stdout, stdin};

use rand::Rng;
use terminal_size::{Width, Height, terminal_size};
use termion::color::{Fg, Bg, Blue, Green, Rgb, Color, AnsiValue};
use termion::event::Key;
use termion::raw::IntoRawMode;
use termion::input::TermRead;


const APP_NAME: &'static str = "umpire";

const MAP_DIMS: Dims = Dims { width: 180, height: 90 };
const HEADER_HEIGHT: u16 = 1;
const FOOTER_HEIGHT: u16 = 5;

const NUM_CONTINENTS:u16 = 100;

const NUM_TEAMS: u16 = 4;
const PLAYER_TEAM: u16 = 0;



// Terminal utility functions

/// 0-indexed variant of Goto
fn goto(x: u16, y: u16) -> termion::cursor::Goto {
    termion::cursor::Goto(x + 1, y + 1)
}

fn draw_scroll_mark(x: u16, y: u16, sym: char) {
    let stdout = stdout();
    let mut stdout = stdout.lock().into_raw_mode().unwrap();
    write!(stdout, "{}{}{}{}", termion::style::Reset, goto(x,y), Fg(AnsiValue(11)), sym);
}

fn erase(x: u16, y: u16) {
    let stdout = stdout();
    let mut stdout = stdout.lock().into_raw_mode().unwrap();
    write!(stdout, "{}{} ", termion::style::Reset, goto(x,y));
}




enum TerrainType {
    WATER,
    LAND,
    // CITY
    //ice, lava, river, deep sea vs shallow, etc.
}

struct Terrain {
    type_: TerrainType,
    x: u16,
    y: u16
}

impl Terrain {
    fn water(x: u16, y: u16) -> Terrain {
        Terrain{ type_: TerrainType::WATER, x: x, y: y }
    }

    fn land(x: u16, y: u16) -> Terrain {
        Terrain{ type_: TerrainType::LAND, x: x, y: y }
    }

    fn color(&self) -> AnsiValue {
        match self.type_ {
            TerrainType::WATER => AnsiValue(12),
            TerrainType::LAND => AnsiValue(10),
            // TerrainType::CITY => AnsiValue(245)
        }
    }
}

enum Alignment {
    NEUTRAL,
    BELLIGERENT { team: u16 }
    // active neutral, chaotic, etc.
}

enum UnitType {
    CITY,

    INFANTRY,
    ARMOR,
    FIGHTER,
    BOMBER,
    TRANSPORT,
    DESTROYER,
    SUBMARINE,
    CRUISER,
    BATTLESHIP,
    CARRIER
}

struct Unit {
    type_: UnitType,
    alignment: Alignment,
    hp: u32,
    max_hp: u32,
    x: u16,
    y: u16
}

impl Unit {
    fn infantry(alignment: Alignment, x: u16, y: u16) -> Unit {
        Unit {
            type_: UnitType::INFANTRY,
            alignment: alignment,
            hp: 1,
            max_hp: 1,
            x: x,
            y: y
        }
    }

    fn city(alignment: Alignment, x: u16, y:u16) -> Unit {
        Unit {
            type_: UnitType::CITY,
            alignment: alignment,
            hp: 1,
            max_hp: 1,
            x: x,
            y: y
        }
    }

    fn symbol(&self) -> char {
        match self.type_ {
            UnitType::CITY => '#',
            UnitType::INFANTRY => '⤲',
            UnitType::ARMOR => 'A',
            UnitType::FIGHTER => '✈',
            UnitType::BOMBER => 'b',
            UnitType::TRANSPORT => 't',
            UnitType::DESTROYER => 'd',
            UnitType::SUBMARINE => '—',
            UnitType::CRUISER => 'c',
            UnitType::BATTLESHIP => 'B',
            UnitType::CARRIER => 'C'
        }
    }

    fn name(&self) -> &'static str {
        match self.type_ {
            UnitType::CITY => "City",
            UnitType::INFANTRY => "Infantry",
            UnitType::ARMOR => "Armor",
            UnitType::FIGHTER => "Fighter",
            UnitType::BOMBER => "Bomber",
            UnitType::TRANSPORT => "Transport",
            UnitType::DESTROYER => "Destroyer",
            UnitType::SUBMARINE => "Submarine",
            UnitType::CRUISER => "Cruiser",
            UnitType::BATTLESHIP => "Battleship",
            UnitType::CARRIER => "Carrier"
        }
    }
}

struct Tile {
    terrain: Terrain,
    units: Vec<Unit>
}

impl Tile {
    fn new(terrain: Terrain) -> Tile {
        Tile{ terrain: terrain, units: Vec::new() }
    }
}

#[derive(Copy,Clone)]
struct Dims {
    width: u16,
    height: u16
}

#[derive(Copy,Clone)]
struct Vec2d<T> {
    x: T,
    y: T
}

struct Game {
    term_dims: Dims,
    map_dims: Dims,
    header_height: u16,
    viewport_dims: Dims,
    viewport_offset: Vec2d<u16>,
    tiles: Vec<Vec<Tile>>, // tiles[col][row]

    old_h_scroll_x: Option<u16>,
    old_v_scroll_y: Option<u16>
}

impl Game {
    fn new(term_dims: Dims, map_dims: Dims, header_height: u16, footer_height: u16, num_continents: u16) -> Game {
        let mut map_tiles = Vec::new();

        for x in 0..map_dims.width {
            let mut col = Vec::new();
            for y in 0..map_dims.height {
                col.push(Tile::new(Terrain::water(x, y)));
            }

            map_tiles.push(col);
        }

        let h_scrollbar_height = 1;
        let v_scrollbar_width = 1;

        let mut game = Game {
            term_dims: term_dims,
            map_dims: map_dims,
            header_height: header_height,
            viewport_dims: Dims{
                width: term_dims.width - v_scrollbar_width,
                height: term_dims.height - header_height - footer_height - h_scrollbar_height
            },
            viewport_offset: Vec2d{ x: map_dims.width/2, y: map_dims.height/2 },
            tiles: map_tiles,

            old_h_scroll_x: Option::None,
            old_v_scroll_y: Option::None,
        };

        game.generate_map(num_continents);


        game
    }

    fn draw(&mut self) {
        let stdout = stdout();
        let mut stdout = stdout.lock().into_raw_mode().unwrap();

        write!(stdout, "{}{}{}{}{}",
            termion::clear::All,
            goto(0,0),
            termion::style::Underline,
            APP_NAME,
            termion::style::Reset
        ).unwrap();


        self.draw_map();
        self.draw_scroll_bars();

        write!(stdout, "{}{}", termion::style::Reset, termion::cursor::Hide).unwrap();
        stdout.flush().unwrap();
    }

    fn draw_map(&self) {
        let stdout = stdout();
        let mut stdout = stdout.lock().into_raw_mode().unwrap();

        for viewport_x in 0..self.viewport_dims.width {
            for viewport_y in 0..self.viewport_dims.height {
                let abs_x = (viewport_x + self.viewport_offset.x) % self.map_dims.width;// mod implements wrapping
                let abs_y = (viewport_y + self.viewport_offset.y) % self.map_dims.height;// mod implements wrapping

                let tile = &self.tiles[abs_x as usize][abs_y as usize];
                let terrain = &tile.terrain;

                let sym = match tile.units.last() {
                    Option::None => ' ',
                    Option::Some(unit) => unit.symbol()
                };

                if abs_y == self.map_dims.height - 1 {
                    write!(stdout, "{}", termion::style::Underline).unwrap();
                }

                write!(stdout, "{}{}{}{}",
                    goto(viewport_x, viewport_y + self.header_height),
                    Bg(terrain.color()),
                    sym,
                    termion::style::NoUnderline
                ).unwrap();


            }
        }
    }

    fn draw_scroll_bars(&mut self) {
        let stdout = stdout();
        let mut stdout = stdout.lock().into_raw_mode().unwrap();


        let h_scrollbar_height = 1;
        let v_scrollbar_width = 1;

        let h_scroll_x: u16 = (self.viewport_dims.width as f32 * (self.viewport_offset.x as f32 / self.map_dims.width as f32)) as u16;
        let h_scroll_y = self.header_height + self.viewport_dims.height + h_scrollbar_height - 1;


        //FIXME There must be a cleaner way to do this
        match self.old_h_scroll_x {
            Option::None => {
                draw_scroll_mark(h_scroll_x, h_scroll_y, '^');
            },
            Option::Some(old_h_scroll_x) => {
                if h_scroll_x != old_h_scroll_x {
                    erase(old_h_scroll_x, h_scroll_y);
                    draw_scroll_mark(h_scroll_x, h_scroll_y, '^');
                }
            }
        }
        self.old_h_scroll_x = Option::Some(h_scroll_x);

        let v_scroll_x = self.viewport_dims.width + v_scrollbar_width - 1;
        let v_scroll_y: u16 = (self.viewport_dims.height as f32 * (self.viewport_offset.y as f32 / self.map_dims.height as f32)) as u16;

        //FIXME There must be a cleaner way to do this
        match self.old_v_scroll_y {
            Option::None => {
                draw_scroll_mark(v_scroll_x, v_scroll_y, '<');
            },
            Option::Some(old_v_scroll_y) => {
                if v_scroll_y != old_v_scroll_y {
                    erase(v_scroll_x, old_v_scroll_y);
                    draw_scroll_mark(v_scroll_x, v_scroll_y, '<');
                }
            }
        }
        self.old_v_scroll_y = Option::Some(v_scroll_y);
    }

    fn generate_map(&mut self, continents: u16) {
        let mut rng = rand::thread_rng();
        for _ in 0..continents {
            let x = rng.gen_range(0, self.map_dims.width);
            let y = rng.gen_range(0, self.map_dims.height);

            let terrain = &mut self.tiles[x as usize][y as usize].terrain;
            // let terrain = &mut tile.terrain;
            terrain.type_ = TerrainType::LAND;
        }

        let mut team_idx = 0;
        while team_idx < NUM_TEAMS {
            let x = rng.gen_range(0, self.map_dims.width);
            let y = rng.gen_range(0, self.map_dims.height);

            let tile = &mut self.tiles[x as usize][y as usize];

            match tile.terrain.type_ {
                TerrainType::LAND => {
                    if tile.units.is_empty() {
                        tile.units.push( Unit::city( Alignment::BELLIGERENT{ team: team_idx }, x, y ) );
                        team_idx += 1;
                    }
                },
                _ => {}
            }
        }
    }

    fn shift_viewport(&mut self, shift: Vec2d<i32>) {
        let mut new_x_offset:i32 = ( self.viewport_offset.x as i32 ) + shift.x;
        let mut new_y_offset:i32 = ( self.viewport_offset.y as i32 ) + shift.y;

        while new_x_offset < 0 {
            new_x_offset += self.map_dims.width as i32;
        }
        while new_y_offset < 0 {
            new_y_offset += self.map_dims.height as i32;
        }

        self.viewport_offset.x = (new_x_offset as u16) % self.map_dims.width;
        self.viewport_offset.y = (new_y_offset as u16) % self.map_dims.height;
        self.draw_map();
        self.draw_scroll_bars();
    }
}

fn main() {
    if let Some((Width(term_width), Height(term_height))) = terminal_size() {
        let mut game = Game::new(
            Dims{ width: term_width, height: term_height },
            MAP_DIMS, HEADER_HEIGHT, FOOTER_HEIGHT, NUM_CONTINENTS
        );

        let stdin = stdin();
        let stdout = stdout();
        let mut stdout = stdout.lock().into_raw_mode().unwrap();

        game.draw();

        for c in stdin.keys() {
            match c.unwrap() {
                Key::Char('H') => game.shift_viewport(Vec2d{x:-1, y:  0}),
                Key::Char('L') => game.shift_viewport(Vec2d{x: 1, y:  0}),
                Key::Char('K') => game.shift_viewport(Vec2d{x: 0, y: -1}),
                Key::Char('J') => game.shift_viewport(Vec2d{x: 0, y:  1}),
                Key::Char('q') => break,
                _ => {}
            }
        }
        println!("");
    } else {
        println!("Unable to get terminal size");
    }
}
