extern crate rand;
extern crate termion;

use rand::Rng;

use termion::color::{Fg, Bg, Blue, Green, Rgb, Color, AnsiValue};
use termion::raw::IntoRawMode;
use std::io::{Read, Write, stdout, stdin};

const APP_NAME: &'static str = "umpire";

// impl Copy for Rgb {
//
// }
//
// impl<C:Color+Copy> Copy for Bg<C> {
//
// }

// type RgbColor = Rgb;
// impl Copy for RgbColor {}
//
//
// type BgRgb = Bg<RgbColor>;
// impl Copy for BgRgb {}

// type BgColor<C> = Bg<C>;
//
// impl<C:Color> Bg<C> {
//
// }

//Wishlist:
// Copy is implemented for Rgb, Bg, Fg

enum TerrainType {
    WATER,
    LAND,
    CITY
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
            TerrainType::CITY => AnsiValue(245)
        }
    }
}

enum Alignment {
    NEUTRAL,
    BELLIGERENT { team: u32 }
    // active neutral, chaotic, etc.
}

enum UnitType {
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
    name: &'static str,
    symbol: char,
    hp: u32,
    max_hp: u32,
    x: u32,
    y: u32
}

impl Unit {
    // fn new(name: &'static str, symbol: char) -> Unit {
    //     Unit{ name: name, symbol: symbol }
    // }

    fn infantry(alignment: Alignment, x: u32, y: u32) -> Unit {
        Unit {
            type_: UnitType::INFANTRY,
            alignment: alignment,
            name: "Infantry",
            symbol: 'i',
            hp: 1,
            max_hp: 1,
            x: x,
            y: y
        }
    }
}


// // Ж≈≋♂

struct Tile {
    terrain: Terrain,
    units: Vec<Unit>
}

impl Tile {
    fn new(terrain: Terrain) -> Tile {
        Tile{ terrain: terrain, units: Vec::new() }
    }
}

const MAP_WIDTH:u16 = 180;
const MAP_HEIGHT:u16 = 90;

const VIEWPORT_WIDTH:u16 = 50;
const VIEWPORT_HEIGHT:u16 = 50;

const NUM_CONTINENTS:u16 = 100;

struct Game {
    map_width: u16,
    map_height: u16,
    viewport_x_offset: u16,
    viewport_y_offset: u16,
    tiles: Vec<Vec<Tile>> // tiles[col][row]
}

// 0-indexed variant of Goto
fn goto(x: u16, y: u16) -> termion::cursor::Goto {
    termion::cursor::Goto(x + 1, y + 1)
}

impl Game {
    fn new(map_width: u16, map_height: u16, num_continents: u16) -> Game {
        let mut map_tiles = Vec::new();

        for x in 0..map_width {
            let mut col = Vec::new();
            for y in 0..map_height {
                col.push(Tile::new(Terrain::water(x, y)));
            }

            map_tiles.push(col);
        }

        let mut game = Game {
            map_width: map_width,
            map_height: map_height,
            viewport_x_offset: map_width/2,
            viewport_y_offset: map_height/2,
            tiles: map_tiles
        };

        game.generate_map(num_continents);


        game
    }

    fn draw(&self) {
        let stdout = stdout();
        let mut stdout = stdout.lock().into_raw_mode().unwrap();

        write!(stdout, "{}", termion::clear::All).unwrap();

        for viewport_x in 0..VIEWPORT_WIDTH {
            for viewport_y in 0..VIEWPORT_HEIGHT {
                let abs_x = (viewport_x + self.viewport_x_offset) % MAP_WIDTH;// mod implements wrapping
                let abs_y = (viewport_y + self.viewport_y_offset) % MAP_HEIGHT;// mod implements wrapping

                let tile = &self.tiles[abs_x as usize][abs_y as usize];
                let terrain = &tile.terrain;

                write!(stdout, "{}{} ", goto(viewport_x, viewport_y), Bg(terrain.color())).unwrap();
            }
        }

        write!(stdout, "{}", termion::style::Reset).unwrap();
    }

    fn generate_map(&mut self, continents: u16) {
        let mut rng = rand::thread_rng();
        for _ in 0..continents {
            let x = rng.gen_range(0, self.map_width);
            let y = rng.gen_range(0, self.map_height);

            let tile = &mut self.tiles[x as usize][y as usize];
            let terrain = &mut tile.terrain;
            terrain.type_ = TerrainType::LAND;
            // let mut terrain = &tile.terrain;
            // (*terrain).type_ = TerrainType::LAND;
            // let mut type_ = &terrain.type_;
            // type_ = TerrainType::LAND;
            // let mut tiles = self.tiles;
            // tiles[x as usize][y as usize].terrain.type_ = TerrainType::LAND;
        }
    }
}

struct Test {
    x: i32
}

fn main() {

    let mut a = Test { x: 10 };
    let b = &mut a;
    // a.x = 20;
    b.x = 20;

    let game = Game::new(MAP_WIDTH, MAP_HEIGHT, NUM_CONTINENTS);
    game.draw();



    return;

    // Initialize 'em all.
    let stdout = stdout();
    let mut stdout = stdout.lock().into_raw_mode().unwrap();
    let stdin = stdin();
    let stdin = stdin.lock();

    write!(stdout, "{}{}{}yo, 'q' will exit.{}{}", termion::clear::All, termion::cursor::Goto(5, 5),
           termion::style::Bold, termion::style::Reset, termion::cursor::Goto(20, 10)).unwrap();
    stdout.flush().unwrap();

    let mut bytes = stdin.bytes();
    loop {
        let b = bytes.next().unwrap().unwrap();

        match b {
            // Quit
            b'q' => return,
            // Clear the screen
            b'c' => write!(stdout, "{}", termion::clear::All),
            // // Set red color
            // b'r' => write!(stdout, "{}", color::Fg(color::Rgb(5, 0, 0))),
            // Write it to stdout.
            a => write!(stdout, "{}", a),
        }.unwrap();

        stdout.flush().unwrap();
    }
}














// extern crate termion;
//
// use termion::event::Key;
// use termion::input::TermRead;
// use termion::raw::IntoRawMode;
// use std::io::{Write, stdout, stdin};
//
// fn rainbow<W: Write>(stdout: &mut W, blue: u8) {
//     write!(stdout, "{}{}", termion::cursor::Goto(1, 1), termion::clear::All).unwrap();
//
//     for red in 0..255 {
//         for green in 0..255 {
//             write!(stdout, "{} ", termion::color::Bg(termion::color::Rgb(red, green, blue))).unwrap();
//         }
//         write!(stdout, "\n\r").unwrap();
//     }
//
//     writeln!(stdout, "{}b = {}", termion::style::Reset, blue).unwrap();
// }
//
// fn main() {
//     let stdin = stdin();
//     let mut stdout = stdout().into_raw_mode().unwrap();
//
//     writeln!(stdout, "{}{}{}Use the arrow keys to change the blue in the rainbow.",
//            termion::clear::All,
//            termion::cursor::Goto(1, 1),
//            termion::cursor::Hide).unwrap();
//
//     let mut blue = 172u8;
//
//     for c in stdin.keys() {
//         match c.unwrap() {
//             Key::Up => {
//                 blue = blue.saturating_add(4);
//                 rainbow(&mut stdout, blue);
//             },
//             Key::Down => {
//                 blue = blue.saturating_sub(4);
//                 rainbow(&mut stdout, blue);
//             },
//             Key::Char('q') => break,
//             _ => {},
//         }
//         stdout.flush().unwrap();
//     }
//
//     write!(stdout, "{}", termion::cursor::Show).unwrap();
// }
