extern crate termion;

use std::io::{StdoutLock,Write};

use termion::color::{Fg, Bg};
use termion::event::Key;

use conf;
use game::Game;
use ui::{Component,Draw,Keypress,Redraw,ScrollableComponent};
use util::{Dims,Location,Rect,Vec2d};

fn nonnegative_mod(x: i32, max: u16) -> u16 {
    let mut result = x;

    while result < 0 {
        result += max as i32;
    }

    return (result % max as i32) as u16;
}

pub struct Map {
    rect: Rect,
    map_dims: Dims,
    old_viewport_offset: Vec2d<u16>,
    viewport_offset: Vec2d<u16>
}

impl Map {
    pub fn new(rect: &Rect, map_dims: &Dims) -> Self {
        Map{
            rect: *rect,
            map_dims: *map_dims,
            old_viewport_offset: Vec2d::new(0, 0),
            viewport_offset: Vec2d::new(rect.width / 2, rect.height / 2)
        }
    }

    pub fn shift_viewport(&mut self, shift: Vec2d<i32>) {
        let mut new_x_offset:i32 = ( self.viewport_offset.x as i32 ) + shift.x;
        let mut new_y_offset:i32 = ( self.viewport_offset.y as i32 ) + shift.y;

        while new_x_offset < 0 {
            new_x_offset += self.map_dims.width as i32;
        }
        while new_y_offset < 0 {
            new_y_offset += self.map_dims.height as i32;
        }

        let new_viewport_offset = Vec2d{
            x: (new_x_offset as u16) % self.map_dims.width,
            y: (new_y_offset as u16) % self.map_dims.height
        };

        self.set_viewport_offset(new_viewport_offset);

        // self.update_map(self.viewport_offset, new_viewport_offset);
        //
        // // self.viewport_offset.x = (new_x_offset as u16) % self.game.map_dims.width;
        // // self.viewport_offset.y = (new_y_offset as u16) % self.game.map_dims.height;
        //
        // self.viewport_offset = new_viewport_offset;
        //
        // // self.draw_map();
        // self.draw_scroll_bars();
    }

    fn set_viewport_offset(&mut self, new_viewport_offset: Vec2d<u16>) {
        // let old_viewport_offset = self.viewport_offset;
        self.old_viewport_offset = self.viewport_offset;
        self.viewport_offset = new_viewport_offset;
        // self.redraw(game, stdout);
        // // self.update_map(old_viewport_offset, new_viewport_offset);
        // // self.viewport_offset = new_viewport_offset;
        // self.draw_scroll_bars();
    }


    pub fn center_viewport(&mut self, map_location: &Location) {
        let new_viewport_offset = Vec2d {
            x: nonnegative_mod(
                map_location.x as i32 - (self.rect.width as i32 / 2),
                self.map_dims.width
            ),
            y: nonnegative_mod(
                map_location.y as i32 - (self.rect.height as i32 / 2),
                self.map_dims.height
            )
        };

        self.set_viewport_offset(new_viewport_offset);
    }

    fn draw_tile(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>,
            tile_loc: Location, viewport_x: u16, viewport_y: u16) {

        let tile = &game.tiles[tile_loc];

        if tile.loc.y == game.map_dims.height - 1 {
            write!(stdout, "{}", termion::style::Underline).unwrap();
        }

        if let Some(fg_color) = tile.fg_color() {
            write!(stdout, "{}", Fg(fg_color)).unwrap();
        }

        write!(stdout, "{}{}{}{}",
            self.goto(viewport_x, viewport_y),
            Bg(tile.bg_color()),
            tile.sym(),
            termion::style::Reset
        ).unwrap();
    }

    fn viewport_to_map_coords(&self, viewport_loc: &Location, viewport_offset: &Vec2d<u16>) -> Location {
        Location {
            x: (viewport_loc.x + viewport_offset.x) % self.map_dims.width, // mod implements wrapping,
            y: (viewport_loc.y + viewport_offset.y) % self.map_dims.height // mod implements wrapping
        }
    }
}

impl ScrollableComponent for Map {
    fn scroll_relative(&mut self, offset: Vec2d<i32>) {
        self.shift_viewport(offset);
    }

    fn offset(&self) -> Vec2d<u16> { self.viewport_offset }
}

impl Redraw for Map {
    /// Update the map to reflect the current viewport offset
    // fn update_map(&mut self, old_viewport_offset: Vec2d<u16>, new_viewport_offset: Vec2d<u16>) {
    fn redraw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        let mut viewport_loc = Location{x: 0, y: 0};
        for viewport_x in 0_u16..self.rect.width {
            viewport_loc.x = viewport_x;
            for viewport_y in 0_u16..(self.rect.height+1) {
                viewport_loc.y = viewport_y;

                let old_map_loc = self.viewport_to_map_coords(&viewport_loc, &self.old_viewport_offset);
                let new_map_loc = self.viewport_to_map_coords(&viewport_loc, &self.viewport_offset);

                let should_draw_tile = {
                    let old_tile = &game.tiles[old_map_loc];
                    let new_tile = &game.tiles[new_map_loc];

                    let redraw_for_border =
                    old_map_loc.y != new_map_loc.y && (
                        old_map_loc.y == game.map_dims.height - 1 ||
                        new_map_loc.y == game.map_dims.height - 1
                    );

                    let redraw_for_mismatch = !(
                        old_tile.terrain==new_tile.terrain &&
                        old_tile.sym() == new_tile.sym() &&
                        old_tile.alignment() == new_tile.alignment()
                    );

                    redraw_for_border || redraw_for_mismatch
                };

                if should_draw_tile {
                    self.draw_tile(game, stdout, new_map_loc, viewport_x, viewport_y);
                }

            }
        }

        write!(stdout, "{}{}", termion::style::Reset, termion::cursor::Hide).unwrap();
        stdout.flush().unwrap();
    }
}

impl Keypress for Map {
    fn keypress(&mut self, key: &Key, game: &mut Game) {
        match *key {
            Key::Char(conf::KEY_VIEWPORT_SHIFT_LEFT)       => self.shift_viewport(Vec2d{x:-1, y: 0}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_RIGHT)      => self.shift_viewport(Vec2d{x: 1, y: 0}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_UP)         => self.shift_viewport(Vec2d{x: 0, y:-1}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN)       => self.shift_viewport(Vec2d{x: 0, y: 1}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_UP_LEFT)    => self.shift_viewport(Vec2d{x:-1, y:-1}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_UP_RIGHT)   => self.shift_viewport(Vec2d{x: 1, y:-1}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN_LEFT)  => self.shift_viewport(Vec2d{x:-1, y: 1}),
            Key::Char(conf::KEY_VIEWPORT_SHIFT_DOWN_RIGHT) => self.shift_viewport(Vec2d{x: 1, y: 1}),
            _ => {}
        }
    }
}

impl Component for Map {
    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn rect(&self) -> Rect {
        self.rect
    }

    fn is_done(&self) -> bool { false }
}

impl Draw for Map {
    // fn draw_map(&mut self) {
    fn draw(&self, game: &Game, stdout: &mut termion::raw::RawTerminal<StdoutLock>) {
        let mut viewport_loc = Location{x: 0, y: 0};
        for viewport_x in 0_u16..self.rect.width {
            viewport_loc.x = viewport_x;
            for viewport_y in 0_u16..(self.rect.height+1) {
                viewport_loc.y = viewport_y;

                let map_location = self.viewport_to_map_coords(&viewport_loc, &self.viewport_offset);

                self.draw_tile(game, stdout, map_location, viewport_x, viewport_y);
            }
        }
    }
}
