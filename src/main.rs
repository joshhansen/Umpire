//!
//! Umpire: a game of world conquest
//!

//Wishlist:
// Copy is implemented for Rgb, Bg, Fg

mod conf;

extern crate rand;
extern crate terminal_size;
extern crate termion;

use std::cmp::max;
use std::io::{Write, stdout, stdin};

use rand::Rng;
use terminal_size::{Width, Height, terminal_size};
use termion::color::{Fg, Bg, AnsiValue};
use termion::event::Key;
use termion::raw::IntoRawMode;
use termion::input::TermRead;

// Derived configuration
const MAP_DIMS: Dims = Dims { width: conf::MAP_WIDTH, height: conf::MAP_HEIGHT };


// Utility functions

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

fn safe_minus_one(x:u16) -> u16 {
    if x > 0 { x - 1}
    else { 0 }
}

fn safe_plus_one(x:u16, max:u16) -> u16 {
    if x < max { x + 1 }
    else { max }
}

#[derive(PartialEq)]
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

#[derive(Copy,Clone)]
enum Alignment {
    NEUTRAL,
    BELLIGERENT { team: u8 }
    // active neutral, chaotic, etc.
}

fn team_color(alignment: Alignment) -> AnsiValue {
    match alignment {
        Alignment::NEUTRAL => AnsiValue(8),
        Alignment::BELLIGERENT{team} => AnsiValue(team+9)
    }
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
    h_scrollbar_height: u16,
    v_scrollbar_width: u16,
    viewport_dims: Dims,
    viewport_offset: Vec2d<u16>,
    tiles: Vec<Vec<Tile>>, // tiles[col][row]

    old_h_scroll_x: Option<u16>,
    old_v_scroll_y: Option<u16>
}

impl Game {
    fn new(term_dims: Dims, map_dims: Dims, header_height: u16, footer_height: u16) -> Game {
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
            h_scrollbar_height: h_scrollbar_height,
            v_scrollbar_width: v_scrollbar_width,
            viewport_dims: Dims{
                width: term_dims.width - v_scrollbar_width,
                height: term_dims.height - header_height - footer_height - h_scrollbar_height
            },
            viewport_offset: Vec2d{ x: map_dims.width/2, y: map_dims.height/2 },
            tiles: map_tiles,

            old_h_scroll_x: Option::None,
            old_v_scroll_y: Option::None,
        };

        game.generate_map();

        game
    }

    fn draw(&mut self) {
        let stdout = stdout();
        let mut stdout = stdout.lock().into_raw_mode().unwrap();

        write!(stdout, "{}{}{}{}{}",
            termion::clear::All,
            goto(0,0),
            termion::style::Underline,
            conf::APP_NAME,
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
            for viewport_y in 0..(self.viewport_dims.height+1) {
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


                let fg_color = if tile.units.is_empty() { AnsiValue(0) } else {
                    let last_unit = &tile.units.last().unwrap();
                    team_color(last_unit.alignment)
                };

                write!(stdout, "{}{}{}{}{}",
                    goto(viewport_x, viewport_y + self.header_height),
                    Fg(fg_color),
                    Bg(terrain.color()),
                    sym,
                    termion::style::NoUnderline
                ).unwrap();
            }
        }
    }

    fn draw_scroll_bars(&mut self) {
        let stdout = stdout();
        let stdout = stdout.lock().into_raw_mode().unwrap();

        let h_scroll_x: u16 = (self.viewport_dims.width as f32 * (self.viewport_offset.x as f32 / self.map_dims.width as f32)) as u16;
        let h_scroll_y = self.header_height + self.viewport_dims.height + self.h_scrollbar_height;


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

        let v_scroll_x = self.viewport_dims.width + self.v_scrollbar_width - 1;
        let v_scroll_y: u16 = self.header_height + (self.viewport_dims.height as f32 * (self.viewport_offset.y as f32 / self.map_dims.height as f32)) as u16;

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

    fn _is_land(&self, x:u16, y:u16) -> bool {
        return self.tiles[x as usize][y as usize].terrain.type_ == TerrainType::LAND;
    }

    fn _land_cardinal_neighbors(&self, x:u16, y:u16) -> u16 {
        let mut land_cardinal_neighbors = 0;

        // left
        if x > 0 && self._is_land(x-1, y) {
            land_cardinal_neighbors += 1;
        }
        // right
        if x < self.map_dims.width - 1 && self._is_land(x+1, y) {
            land_cardinal_neighbors += 1;
        }
        // up
        if y > 0 && self._is_land(x, y-1) {
            land_cardinal_neighbors += 1;
        }
        // down
        if y < self.map_dims.height - 1 && self._is_land(x, y+1) {
            land_cardinal_neighbors += 1;
        }

        land_cardinal_neighbors
    }

    fn _land_diagonal_neighbors(&self, x:u16, y:u16) -> u16 {
        let x_low_room = x > 0;
        let y_low_room = y > 0;
        let x_high_room = x < self.map_dims.width - 1;
        let y_high_room = y < self.map_dims.height - 1;

        let mut land_neighbors = 0;

        if x_low_room && y_low_room && self._is_land(x-1, y-1) {
            land_neighbors += 1;
        }
        if x_low_room && y_high_room && self._is_land(x-1, y+1) {
            land_neighbors += 1;
        }
        if x_high_room && y_low_room && self._is_land(x+1, y-1) {
            land_neighbors += 1;
        }
        if x_high_room && y_high_room && self._is_land(x+1, y+1) {
            land_neighbors += 1;
        }
        land_neighbors
    }

    // fn _land_neighbors(&self, x:u16, y:u16) -> u16 {
    //     let mut land_nearby = 0;
    //     for x2 in safe_minus_one(x)..(safe_plus_one(x, self.map_dims.width)+1) {
    //         for y2 in safe_minus_one(y)..(safe_plus_one(y, self.map_dims.height)+1) {
    //             if x2 != x && y2 != y {
    //                 if self.tiles[x2 as usize][y2 as usize].terrain.type_ == TerrainType::LAND {
    //                     land_nearby += 1;
    //                 }
    //             }
    //         }
    //     }
    //     land_nearby
    // }

    fn generate_map(&mut self) {
        let mut rng = rand::thread_rng();

        // Seed the continents/islands
        for _ in 0..conf::LANDMASSES {
            let x = rng.gen_range(0, self.map_dims.width);
            let y = rng.gen_range(0, self.map_dims.height);

            let terrain = &mut self.tiles[x as usize][y as usize].terrain;
            // let terrain = &mut tile.terrain;
            terrain.type_ = TerrainType::LAND;
        }

        // Grow landmasses
        for _iteration in 0..conf::GROWTH_ITERATIONS {
            for x in 0..self.map_dims.width {
                for y in 0..self.map_dims.height {

                    match self.tiles[x as usize][y as usize].terrain.type_ {
                        // TerrainType::LAND => {
                        //
                        //     for x2 in safe_minus_one(x)..(safe_plus_one(x, self.map_dims.width)+1) {
                        //         for y2 in safe_minus_one(y)..(safe_plus_one(y, self.map_dims.height)+1) {
                        //             if x2 != x && y2 != y {
                        //                 if rng.next_f32() <= GROWTH_PROB {
                        //                     self.tiles[x2 as usize][y2 as usize].terrain.type_ = TerrainType::LAND;
                        //                 }
                        //             }
                        //         }
                        //     }
                        // },
                        TerrainType::WATER => {
                            let cardinal_growth_prob = self._land_cardinal_neighbors(x, y) as f32 / (4_f32 + conf::GROWTH_CARDINAL_LAMBDA);
                            let diagonal_growth_prob = self._land_diagonal_neighbors(x, y) as f32 / (4_f32 + conf::GROWTH_DIAGONAL_LAMBDA);

                            if rng.next_f32() <= cardinal_growth_prob || rng.next_f32() <= diagonal_growth_prob {
                                self.tiles[x as usize][y as usize].terrain.type_ = TerrainType::LAND;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Populate neutral cities
        for x in 0..self.map_dims.width {
            for y in 0..self.map_dims.height {
                let tile = &mut self.tiles[x as usize][y as usize];
                if tile.terrain.type_ == TerrainType::LAND {
                    if rng.next_f32() <= conf::NEUTRAL_CITY_DENSITY {
                        tile.units.push( Unit::city(Alignment::NEUTRAL, x, y));
                    }
                }
            }
        }

        // Populate player cities
        let mut team_idx = 0_u8;
        while team_idx < conf::NUM_TEAMS {
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
            MAP_DIMS, conf::HEADER_HEIGHT, conf::FOOTER_HEIGHT
        );

        let stdin = stdin();
        let stdout = stdout();
        let mut stdout = stdout.lock().into_raw_mode().unwrap();

        game.draw();

        for c in stdin.keys() {
            match c.unwrap() {
                Key::Char(conf::KEY_VIEWPORT_SHIFT_LEFT)       => game.shift_viewport(Vec2d{x:-1, y: 0}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_RIGHT)      => game.shift_viewport(Vec2d{x: 1, y: 0}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_UP)         => game.shift_viewport(Vec2d{x: 0, y:-1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN)       => game.shift_viewport(Vec2d{x: 0, y: 1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_UP_LEFT)    => game.shift_viewport(Vec2d{x:-1, y:-1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_UP_RIGHT)   => game.shift_viewport(Vec2d{x: 1, y:-1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN_LEFT)  => game.shift_viewport(Vec2d{x:-1, y: 1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN_RIGHT) => game.shift_viewport(Vec2d{x: 1, y: 1}),
                Key::Char(conf::KEY_QUIT) => break,
                _ => {}
            }
        }
        write!(stdout, "{}{}\n\n", goto(0, term_height), termion::style::Reset).unwrap();
    } else {
        println!("Unable to get terminal size");
    }
}
