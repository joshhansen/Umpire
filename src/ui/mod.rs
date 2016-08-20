//!
//! The user interface.
//!
//! Making use of the abstract game engine, implement a user interface for the game.
extern crate termion;

use std::io::{Write, stdout, stdin, StdoutLock};

use termion::color::{Fg, Bg, AnsiValue};
use termion::event::Key;
use termion::input::TermRead;

use conf;
use game::Game;
use util::{Dims,Rect,Vec2d,Location};

/// 0-indexed variant of Goto
pub fn goto(x: u16, y: u16) -> termion::cursor::Goto {
    termion::cursor::Goto(x + 1, y + 1)
}

pub struct UI<'a> {
    game: Game,
    stdout: termion::raw::RawTerminal<StdoutLock<'a>>,
    term_dims: Dims,
    header_height: u16,
    h_scrollbar_height: u16,
    v_scrollbar_width: u16,
    viewport_size: ViewportSize,
    // viewport_rect: Rect,
    viewport_offset: Vec2d<u16>,
    old_h_scroll_x: Option<u16>,
    old_v_scroll_y: Option<u16>
}

enum ViewportSize {
    REGULAR,
    THEATER,
    FULLSCREEN
}



impl<'b> UI<'b> {
    pub fn new(
        game: Game,
        stdout: termion::raw::RawTerminal<StdoutLock<'b>>,
        term_dims: Dims, header_height: u16
    ) -> Self {

        let h_scrollbar_height = 1;
        let v_scrollbar_width = 1;

        let offset = Vec2d{ x: game.map_dims.width/2, y: game.map_dims.height/2 };

        UI {
            game: game,
            mode: Mode::General,
            stdout: stdout,
            term_dims: term_dims,
            header_height: header_height,
            h_scrollbar_height: h_scrollbar_height,
            v_scrollbar_width: v_scrollbar_width,
            viewport_size: ViewportSize::REGULAR,
            // viewport_rect: viewport_rect(&ViewportSize::REGULAR, header_height, h_scrollbar_height, v_scrollbar_width, &term_dims),
            viewport_offset: offset,
            old_h_scroll_x: None,
            old_v_scroll_y: None,
    }

    pub fn run(&mut self) {
        self.draw();
{
        let (player,turn) = self.game.next_player_turn();

        for production_set_loc in turn.production_set_requests() {

        }
    }

        let stdin = stdin();
        for c in stdin.keys() {
            match c.unwrap() {
                Key::Char(conf::KEY_VIEWPORT_SHIFT_LEFT)       => self.shift_viewport(Vec2d{x:-1, y: 0}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_RIGHT)      => self.shift_viewport(Vec2d{x: 1, y: 0}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_UP)         => self.shift_viewport(Vec2d{x: 0, y:-1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN)       => self.shift_viewport(Vec2d{x: 0, y: 1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_UP_LEFT)    => self.shift_viewport(Vec2d{x:-1, y:-1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_UP_RIGHT)   => self.shift_viewport(Vec2d{x: 1, y:-1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN_LEFT)  => self.shift_viewport(Vec2d{x:-1, y: 1}),
                Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN_RIGHT) => self.shift_viewport(Vec2d{x: 1, y: 1}),
                Key::Char(conf::KEY_QUIT) => break,
                Key::Char(conf::KEY_VIEWPORT_SIZE_ROTATE) => {
                    let new_size = match self.viewport_size {
                        ViewportSize::REGULAR => ViewportSize::THEATER,
                        ViewportSize::THEATER => ViewportSize::FULLSCREEN,
                        ViewportSize::FULLSCREEN => ViewportSize::REGULAR
                    };

                    self.set_viewport_size(new_size);
                }
                _ => {}
            }
        }

        self.quit();
    }

    fn set_viewport_size(&mut self, viewport_size: ViewportSize) {
        // self.viewport_rect = viewport_rect(&viewport_size, self.header_height, self.h_scrollbar_height, self.v_scrollbar_width, &self.term_dims);
        self.viewport_size = viewport_size;
        self.draw();
    }

    pub fn draw(&mut self) {
        write!(self.stdout, "{}{}{}{}{}",
            termion::clear::All,
            goto(0,0),
            termion::style::Underline,
            conf::APP_NAME,
            termion::style::Reset
        ).unwrap();

        self.draw_map();
        self.draw_scroll_bars();

        write!(self.stdout, "{}{}", goto(45, 45), self.game.turn).unwrap();

        write!(self.stdout, "{}{}", termion::style::Reset, termion::cursor::Hide).unwrap();
        self.stdout.flush().unwrap();
    }

    fn viewport_to_map_coords(&self, x: &u16, y: &u16) -> Location {
        let map_x:u16 = (x + self.viewport_offset.x) % self.game.map_dims.width;// mod implements wrapping
        let map_y:u16 = (y + self.viewport_offset.y) % self.game.map_dims.height;// mod implements wrapping
        Location{
            x: map_x,
            y: map_y
        }
    }

    fn draw_map(&mut self) {
        let viewport_rect = self.viewport_rect();
        for viewport_x in 0_u16..viewport_rect.width {
            for viewport_y in 0_u16..(viewport_rect.height+1) {
                let map_location = self.viewport_to_map_coords(&viewport_x, &viewport_y);

                self.draw_tile(map_location, viewport_x, viewport_y);
            }
        }
    }

    fn draw_tile(&mut self, tile_loc: Location, viewport_x: u16, viewport_y: u16) {
        let tile = &self.game.tiles[tile_loc];

        if tile.loc.y == self.game.map_dims.height - 1 {
            write!(self.stdout, "{}", termion::style::Underline).unwrap();
        }

        match tile.fg_color() {
            Some(fg_color) => {
                write!(self.stdout, "{}", Fg(fg_color)).unwrap();
            },
            _ => {}
        }

        let viewport_rect = self.viewport_rect();
        write!(self.stdout, "{}{}{}{}",
            goto(viewport_x + viewport_rect.left, viewport_y + viewport_rect.top),
            Bg(tile.bg_color()),
            tile.sym(),
            termion::style::NoUnderline
        ).unwrap();
    }

    /// Update the map to reflect the current viewport offset
    fn update_map(&mut self) {
        let viewport_rect = self.viewport_rect();
        for viewport_x in 0_u16..viewport_rect.width {
            for viewport_y in 0_u16..(viewport_rect.height+1) {
                let old_map_loc = self.viewport_to_map_coords(&viewport_x, &viewport_y);
                let new_map_loc = self.viewport_to_map_coords(&viewport_x, &viewport_y);

                let should_draw_tile:bool = {
                    let old_tile = &self.game.tiles[old_map_loc];
                    let new_tile = &self.game.tiles[new_map_loc];

                    let alignments_match = match old_tile.alignment() {
                        None => {
                            match new_tile.alignment() {
                                None => true,
                                Some(_new_alignment) => false
                            }
                        },
                        Some(old_alignment) => {
                            match new_tile.alignment() {
                                None => false,
                                Some(new_alignment) => old_alignment==new_alignment
                            }
                        }
                    };

                    old_tile.terrain.type_==new_tile.terrain.type_ &&
                        old_tile.sym() == new_tile.sym() &&
                        alignments_match
                };



                if should_draw_tile {
                    self.draw_tile(new_map_loc, viewport_x, viewport_y);
                }

            }
        }
    }

    fn draw_scroll_bars(&mut self) {
        let viewport_rect = self.viewport_rect();
        let h_scroll_x: u16 = (viewport_rect.width as f32 * (self.viewport_offset.x as f32 / self.game.map_dims.width as f32)) as u16;
        let h_scroll_y = self.header_height + viewport_rect.height + self.h_scrollbar_height;


        //FIXME There must be a cleaner way to do this
        match self.old_h_scroll_x {
            None => {
                self.draw_scroll_mark(h_scroll_x, h_scroll_y, '^');
            },
            Some(old_h_scroll_x) => {
                if h_scroll_x != old_h_scroll_x {
                    self.erase(old_h_scroll_x, h_scroll_y);
                    self.draw_scroll_mark(h_scroll_x, h_scroll_y, '^');
                }
            }
        }
        self.old_h_scroll_x = Some(h_scroll_x);

        let v_scroll_x = viewport_rect.width + self.v_scrollbar_width - 1;
        let v_scroll_y: u16 = self.header_height + (viewport_rect.height as f32 * (self.viewport_offset.y as f32 / self.game.map_dims.height as f32)) as u16;

        //FIXME There must be a cleaner way to do this
        match self.old_v_scroll_y {
            None => {
                self.draw_scroll_mark(v_scroll_x, v_scroll_y, '<');
            },
            Some(old_v_scroll_y) => {
                if v_scroll_y != old_v_scroll_y {
                    self.erase(v_scroll_x, old_v_scroll_y);
                    self.draw_scroll_mark(v_scroll_x, v_scroll_y, '<');
                }
            }
        }
        self.old_v_scroll_y = Some(v_scroll_y);
    }



    pub fn shift_viewport(&mut self, shift: Vec2d<i32>) {
        let mut new_x_offset:i32 = ( self.viewport_offset.x as i32 ) + shift.x;
        let mut new_y_offset:i32 = ( self.viewport_offset.y as i32 ) + shift.y;

        while new_x_offset < 0 {
            new_x_offset += self.game.map_dims.width as i32;
        }
        while new_y_offset < 0 {
            new_y_offset += self.game.map_dims.height as i32;
        }

        self.update_map();

        self.viewport_offset.x = (new_x_offset as u16) % self.game.map_dims.width;
        self.viewport_offset.y = (new_y_offset as u16) % self.game.map_dims.height;
        // self.draw_map();
        self.draw_scroll_bars();
    }

    pub fn center_viewport(&mut self, center: Location) {
        self.viewport_offset = center;
        self.update_map()
    }

    // Utility methods
    fn draw_scroll_mark(&mut self, x: u16, y: u16, sym: char) {
        write!(self.stdout, "{}{}{}{}", termion::style::Reset, goto(x,y), Fg(AnsiValue(11)), sym).unwrap();
    }

    fn erase(&mut self, x: u16, y: u16) {
        write!(self.stdout, "{}{} ", termion::style::Reset, goto(x,y)).unwrap();
    }

    pub fn quit(&mut self) {
        write!(self.stdout, "{}{}\n\n", goto(0, self.term_dims.height), termion::style::Reset).unwrap();
    }

    fn viewport_rect(&self) -> Rect {
        match self.viewport_size {
            ViewportSize::REGULAR => Rect {
                left: 0,
                top: self.header_height,
                width: (self.term_dims.width - self.v_scrollbar_width) / 2,
                height: 25
            },
            ViewportSize::THEATER => Rect {
                left: 0,
                top: self.header_height,
                width: self.term_dims.width - self.v_scrollbar_width,
                height: 25
            },
            ViewportSize::FULLSCREEN => Rect {
                left: 0,
                top: 0,
                width: self.term_dims.width - self.v_scrollbar_width,
                height: self.term_dims.height - self.h_scrollbar_height - 1
            }
        }
    }
}
