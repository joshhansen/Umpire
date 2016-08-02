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
use util::{draw_scroll_mark,erase,goto,Dims,Vec2d};

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
            tiles: map::gen::generate_map(map_dims),

            old_h_scroll_x: Option::None,
            old_v_scroll_y: Option::None,
        };

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
                    unit::alignment_color(last_unit.alignment)
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
