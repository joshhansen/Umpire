//!
//! Umpire: a game of world conquest
//!

//Wishlist:
// Copy is implemented for Rgb, Bg, Fg

mod conf;
mod map;
mod unit;
mod util;

use map::{Tile};
use util::{goto,Dims,Vec2d,Rect};

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

struct Game<'a> {
    stdout: termion::raw::RawTerminal<std::io::StdoutLock<'a>>,
    term_dims: Dims,
    map_dims: Dims,
    header_height: u16,
    h_scrollbar_height: u16,
    v_scrollbar_width: u16,
    viewport_size: ViewportSize,
    viewport_rect: Rect,
    viewport_offset: Vec2d<u16>,
    tiles: Vec<Vec<Tile>>, // tiles[col][row]

    old_h_scroll_x: Option<u16>,
    old_v_scroll_y: Option<u16>
}

enum ViewportSize {
    REGULAR,
    THEATER,
    FULLSCREEN
}

impl<'b> Game<'b> {
    fn new(
        stdout: termion::raw::RawTerminal<std::io::StdoutLock<'b>>,
        term_dims: Dims, map_dims: Dims, header_height: u16, footer_height: u16
    ) -> Game<'b> {

        let h_scrollbar_height = 1;
        let v_scrollbar_width = 1;

        let mut game = Game {
            stdout: stdout,
            term_dims: term_dims,
            map_dims: map_dims,
            header_height: header_height,
            h_scrollbar_height: h_scrollbar_height,
            v_scrollbar_width: v_scrollbar_width,
            viewport_size: ViewportSize::REGULAR,
            viewport_rect: Game::viewport_rect(&ViewportSize::REGULAR, header_height, h_scrollbar_height, v_scrollbar_width, &term_dims),
            viewport_offset: Vec2d{ x: map_dims.width/2, y: map_dims.height/2 },
            tiles: map::gen::generate_map(map_dims),
            old_h_scroll_x: Option::None,
            old_v_scroll_y: Option::None,
        };

        // game.set_viewport_rect(ViewportSize::REGULAR);

        game
    }

    fn viewport_rect(
            viewport_size: &ViewportSize,
            header_height: u16,
            h_scrollbar_height: u16,
            v_scrollbar_width: u16,
            term_dims: &Dims
    ) -> Rect {
        match *viewport_size {
            ViewportSize::REGULAR => Rect {
                left: 0,
                top: header_height,
                width: (term_dims.width - v_scrollbar_width) / 2,
                height: 25
            },
            ViewportSize::THEATER => Rect {
                left: 0,
                top: header_height,
                width: term_dims.width - v_scrollbar_width,
                height: 25
            },
            ViewportSize::FULLSCREEN => Rect {
                left: 0,
                top: 0,
                width: term_dims.width - v_scrollbar_width,
                height: term_dims.height - h_scrollbar_height - 1
            }
        }
    }

    fn set_viewport_size(&mut self, viewport_size: ViewportSize) {
        self.viewport_rect = Game::viewport_rect(&viewport_size, self.header_height, self.h_scrollbar_height, self.v_scrollbar_width, &self.term_dims);
        self.viewport_size = viewport_size;
        self.draw();
    }

    fn draw(&mut self) {
        write!(self.stdout, "{}{}{}{}{}",
            termion::clear::All,
            goto(0,0),
            termion::style::Underline,
            conf::APP_NAME,
            termion::style::Reset
        ).unwrap();

        self.draw_map();
        self.draw_scroll_bars();

        write!(self.stdout, "{}{}", termion::style::Reset, termion::cursor::Hide).unwrap();
        self.stdout.flush().unwrap();
    }

    fn viewport_to_map_coords(&self, x: &u16, y: &u16, viewport_offset: &Vec2d<u16>) -> (u16,u16) {
        let map_x:u16 = (x + viewport_offset.x) % self.map_dims.width;// mod implements wrapping
        let map_y:u16 = (y + viewport_offset.y) % self.map_dims.height;// mod implements wrapping
        (map_x,map_y)
    }

    fn draw_map(&mut self) {
        for viewport_x in 0_u16..self.viewport_rect.width {
            for viewport_y in 0_u16..(self.viewport_rect.height+1) {
                let (map_x,map_y) = self.viewport_to_map_coords(&viewport_x, &viewport_y, &self.viewport_offset);

                self.draw_tile(map_x, map_y, viewport_x, viewport_y);
            }
        }
    }

    fn draw_tile(&mut self, tile_x: u16, tile_y: u16, viewport_x: u16, viewport_y: u16) {
        let tile = &self.tiles[tile_x as usize][tile_y as usize];

        if tile.y == self.map_dims.height - 1 {
            write!(self.stdout, "{}", termion::style::Underline).unwrap();
        }

        match tile.fg_color() {
            Option::Some(fg_color) => {
                write!(self.stdout, "{}", Fg(fg_color));
            },
            _ => {}
        }

        write!(self.stdout, "{}{}{}{}",
            goto(viewport_x + self.viewport_rect.left, viewport_y + self.viewport_rect.top),
            Bg(tile.bg_color()),
            tile.sym(),
            termion::style::NoUnderline
        ).unwrap();
    }

    fn update_map(&mut self, new_viewport_offset: Vec2d<u16>) {
        for viewport_x in 0_u16..self.viewport_rect.width {
            for viewport_y in 0_u16..(self.viewport_rect.height+1) {
                let (old_map_x,old_map_y) = self.viewport_to_map_coords(&viewport_x, &viewport_y, &new_viewport_offset);
                let (new_map_x,new_map_y) = self.viewport_to_map_coords(&viewport_x, &viewport_y, &new_viewport_offset);

                let should_draw_tile:bool = {
                    let old_tile = &self.tiles[old_map_x as usize][old_map_y as usize];
                    let new_tile = &self.tiles[new_map_x as usize][new_map_y as usize];

                    let alignments_match = match old_tile.alignment() {
                        Option::None => {
                            match new_tile.alignment() {
                                Option::None => true,
                                Option::Some(_new_alignment) => false
                            }
                        },
                        Option::Some(old_alignment) => {
                            match new_tile.alignment() {
                                Option::None => false,
                                Option::Some(new_alignment) => old_alignment==new_alignment
                            }
                        }
                    };

                    old_tile.terrain.type_==new_tile.terrain.type_ &&
                        old_tile.sym() == new_tile.sym() &&
                        alignments_match
                };



                if should_draw_tile {
                    self.draw_tile(new_map_x, new_map_y, viewport_x, viewport_y);
                }

            }
        }
    }

    fn draw_scroll_bars(&mut self) {
        let h_scroll_x: u16 = (self.viewport_rect.width as f32 * (self.viewport_offset.x as f32 / self.map_dims.width as f32)) as u16;
        let h_scroll_y = self.header_height + self.viewport_rect.height + self.h_scrollbar_height;


        //FIXME There must be a cleaner way to do this
        match self.old_h_scroll_x {
            Option::None => {
                self.draw_scroll_mark(h_scroll_x, h_scroll_y, '^');
            },
            Option::Some(old_h_scroll_x) => {
                if h_scroll_x != old_h_scroll_x {
                    self.erase(old_h_scroll_x, h_scroll_y);
                    self.draw_scroll_mark(h_scroll_x, h_scroll_y, '^');
                }
            }
        }
        self.old_h_scroll_x = Option::Some(h_scroll_x);

        let v_scroll_x = self.viewport_rect.width + self.v_scrollbar_width - 1;
        let v_scroll_y: u16 = self.header_height + (self.viewport_rect.height as f32 * (self.viewport_offset.y as f32 / self.map_dims.height as f32)) as u16;

        //FIXME There must be a cleaner way to do this
        match self.old_v_scroll_y {
            Option::None => {
                self.draw_scroll_mark(v_scroll_x, v_scroll_y, '<');
            },
            Option::Some(old_v_scroll_y) => {
                if v_scroll_y != old_v_scroll_y {
                    self.erase(v_scroll_x, old_v_scroll_y);
                    self.draw_scroll_mark(v_scroll_x, v_scroll_y, '<');
                }
            }
        }
        self.old_v_scroll_y = Option::Some(v_scroll_y);
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




        let new_viewport_offset: Vec2d<u16> = Vec2d{ x: new_x_offset as u16, y: new_y_offset as u16 };

        self.update_map(new_viewport_offset);






        self.viewport_offset.x = (new_x_offset as u16) % self.map_dims.width;
        self.viewport_offset.y = (new_y_offset as u16) % self.map_dims.height;
        // self.draw_map();
        self.draw_scroll_bars();
    }



    // Utility methods
    fn draw_scroll_mark(&mut self, x: u16, y: u16, sym: char) {
        write!(self.stdout, "{}{}{}{}", termion::style::Reset, goto(x,y), Fg(AnsiValue(11)), sym);
    }

    fn erase(&mut self, x: u16, y: u16) {
        write!(self.stdout, "{}{} ", termion::style::Reset, goto(x,y));
    }
}



fn main() {
    let stdout_0 : std::io::Stdout = stdout();
    let stdout_1 = stdout_0.lock().into_raw_mode().unwrap();
    if let Some((Width(term_width), Height(term_height))) = terminal_size() {
        let stdin = stdin();



        let mut game = Game::new(
            stdout_1,
            Dims{ width: term_width, height: term_height },
            MAP_DIMS, conf::HEADER_HEIGHT, conf::FOOTER_HEIGHT
        );



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
                Key::Char(conf::KEY_VIEWPORT_SIZE_ROTATE) => {
                    let new_size = match game.viewport_size {
                        ViewportSize::REGULAR => ViewportSize::THEATER,
                        ViewportSize::THEATER => ViewportSize::FULLSCREEN,
                        ViewportSize::FULLSCREEN => ViewportSize::REGULAR
                    };

                    game.set_viewport_size(new_size);
                }
                _ => {}
            }
        }

        let stdout_2 : std::io::Stdout = stdout();
        let mut stdout_3 = stdout_0.lock().into_raw_mode().unwrap();
        write!(stdout_3, "{}{}\n\n", goto(0, term_height), termion::style::Reset).unwrap();
    } else {
        println!("Unable to get terminal size");
    }
}
